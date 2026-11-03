//! Shared `BidderFanout` conformance suite.
//!
//! Compiled only under `cfg(test)` or the `conformance-suite` feature.
//! An integration test compiles the library as an ordinary dependency
//! WITHOUT `--cfg test`, so a `cfg(test)`-only helper would be
//! invisible to `tests/*.rs`; the feature gate plus `pub` visibility is
//! what makes one set of assertions reachable from every conformance
//! test binary. The feature-off public API is unchanged.
//!
//! Two buckets:
//!
//! * **Bucket A** runs against [`DevFanout`] and against
//!   [`EdgeZeroFanout`] on both of its drives. It covers behaviour
//!   that needs no mid-fanout deadline or per-call-timeout
//!   cancellation, which is exactly what `DevFanout` cannot do.
//! * **Bucket B** needs a real `Timer` to synthesize a
//!   [`TimeoutCause`] at all, so it skips `DevFanout` and runs against
//!   [`EdgeZeroFanout`] on both drives: the per-call strategy bridge
//!   and the wave scheduler. The wave arm is the one a served request
//!   reaches, so a timeout attribution that holds only on the strategy
//!   bridge certifies nothing production does.
//!
//! Every fixture drives a [`FakeClock`], which is BOTH the `Clock` and
//! the `Timer`. A pending `sleep` fires only when something calls
//! `advance`, so any case whose deadline or per-call timeout is still
//! in the future must be polled by hand: poll to `Pending`, advance the
//! shared clock past the wake instant, then repoll. A bare
//! `block_on` there would park forever.

pub use crate::test_support::{bidder_fixture::fake_bidder, boot_parity_fixture};

mod handler_request;

use crate::test_support::outbound_request;

use crate::config::AuctioneerAppConfig;
use crate::config::budgets::resolve_budgets;
use crate::handlers::{TestFanoutHandle, auction as auction_handler};
use crate::host_primitives::edgezero_proxy::{EdgeZeroFanout, EdgeZeroProxyAdapter};
use crate::host_primitives::fanout::DevFanout;
use crate::host_primitives::registry::{TestClockHandle, TestRngHandle};
use crate::host_primitives::strategy::RaceTimersStrategy;
use async_trait::async_trait;
use auctioneer_bidder::{
    Bidder, BidderError, BidderHttpResponse, BidderName, BidderRegistry, BidderRequestInfo,
    BidderResponse, DebugBodySchema, OutboundRequestMeta, OutboundRequestSink,
};
use auctioneer_config_data::types::BidderInfo;
use auctioneer_exchange::fanout::{TransportError, TransportErrorKind};
use auctioneer_exchange::scrub::DisclosureAudit;
use auctioneer_exchange::{
    AccountSource, AuctionContext, AuctionDeadline, AuctionError, AuctionPlan, BidderCall,
    BidderCallResult, BidderFanout, Compression, ExchangeFeatures, FanoutError,
    FanoutInternalError, OversizeOrigin, ResolvedAccount, ResolvedAliases, TimeoutCause,
    TmaxSource, UNRESOLVED_ACCOUNT_ID, run_auction,
};
use auctioneer_openrtb::ext::imp::ImpExt;
use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, Imp};
use auctioneer_openrtb::types::bid_response::BidResponse;
use auctioneer_openrtb::types::field::JsonField;
use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare};
use auctioneer_privacy::gcl::Gcl;
use auctioneer_privacy::gvl::GvlStore;
use auctioneer_runtime::{
    Clock, FakeClock, MAX_GVL_PARSED_BYTES, MIN_INBOUND_PARSE_FACTOR, MIN_RESPONSE_NODE_PEAK_BYTES,
    MIN_RESPONSE_PARSE_FACTOR, NoopDecodeCooperator, ResolvedIsolateLimits, Rng, SeededRng,
    StreamGate, Timer,
};
use edgezero_core::blob_envelope::BlobEnvelope;
use edgezero_core::body::Body;
use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;
use edgezero_core::config_store::{
    BoundedStoreRead, ConfigStore, ConfigStoreError, ConfigStoreHandle, ConfigValue,
};
use edgezero_core::context::RequestContext;
use edgezero_core::error::{BadGatewayReason, EdgeError};
use edgezero_core::http::{HeaderMap, StatusCode};
use edgezero_core::outbound::{
    HttpClient, IndependentOutboundProgress, OutboundBatch, OutboundBatchDriverEvent,
    OutboundDispatchRequest, OutboundExchangeOwner, OutboundFuture, OutboundHead,
    OutboundHeaderFidelity, OutboundHeaderInput, OutboundHttpClient, OutboundPending,
    OutboundPreparedBatch, OutboundProgressRuntime, OutboundRequest, OutboundResponse,
    OutboundResponseContext, OutboundSendMode, OutboundTime,
};
use edgezero_core::params::PathParams;
use edgezero_core::store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry};
use edgezero_core::time::{Deadline, MonotonicClock, MonotonicInstant};
use fake_bidder::FakeBidder;
use futures::executor::block_on;
use futures::future::{Either, LocalBoxFuture, select};
use futures::stream::{self, FuturesUnordered, StreamExt as _};
use http::{Method, Request, Uri};
use std::collections::{BTreeMap, BTreeSet};
use std::future::{Future, pending};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;
use time::OffsetDateTime;

/// Distinct endpoints so the canned-response table can key on URL and
/// a fanout that mixed calls up would fail on the body, not the order.
const BIDDER_A: &str = "https://bidder-a.example/rtb";
const BIDDER_B: &str = "https://bidder-b.example/rtb";
const BIDDER_C: &str = "https://bidder-c.example/rtb";

/// Generous default so a Bucket A row never trips a timeout it did not
/// mean to exercise.
const GENEROUS_MS: u64 = 1000;

/// Auction budget pinned on the fixture request instead of inherited
/// from `[baked.host]` in `auctioneer.toml`.
///
/// The rows that drive `run_auction` do it on a `FakeClock` that never
/// advances, so the whole budget is still unspent when the exchange
/// computes its remaining span. Stating the budget here keeps that span
/// nonzero no matter what the host default becomes: a zero-length
/// budget would short-circuit the auction on the expired-tmax path — or,
/// once the fanout consults the deadline mid-flight, resolve every call
/// as `Timeout(AuctionDeadline)` — and those rows are about transport
/// and capability failures, not about deadlines.
///
/// Must stay at or below `max_tmax_ms` so it survives unclamped.
const AUCTION_TMAX_MS: i64 = 1000;

/// Response cap wide enough that only the oversize row exceeds it.
const WIDE_CAP: usize = 1 << 20;

/// `PrepareLimits::max_impressions` for the single-imp conformance
/// fixture — wide enough that the cap never engages here.
const CONFORMANCE_MAX_IMPRESSIONS: u32 = 100;

// ---------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------

/// What a canned endpoint does once its delay has elapsed.
enum Outcome {
    /// Answer with this status, adapter-visible headers, and encoded body.
    Respond {
        status: u16,
        headers: HeaderMap,
        body: Vec<u8>,
    },
    /// Fail at the transport layer, the way an unreachable or reset
    /// upstream would.
    Transport,
    /// Never answer, so only a timer arm can resolve the call.
    Never,
}

struct Canned {
    delay: Duration,
    outcome: Outcome,
}

/// One recorded outbound dispatch.
pub struct Dispatched {
    /// Absolute URL the fanout asked the proxy for.
    pub url: String,
    /// Body as the fanout prepared it — already gzipped when the call
    /// declared `endpoint_compression`.
    pub body: Vec<u8>,
}

struct ClientState {
    clock: Arc<FakeClock>,
    canned: BTreeMap<String, Canned>,
    dispatched: Mutex<Vec<Dispatched>>,
}

/// Canned `EdgeZero` [`ProxyClient`] keyed by URL. Both fanouts reach
/// it through the same `EdgeZeroProxyAdapter`, so a Bucket A row proves
/// the same wire behaviour on both sides.
///
/// Delays are served off the shared [`FakeClock`], so a delayed
/// endpoint stays pending until a test advances the clock.
#[derive(Clone)]
pub struct TestProxyClient {
    state: Arc<ClientState>,
}

/// Accumulates the per-URL canned table.
pub struct TestProxyClientBuilder {
    clock: Arc<FakeClock>,
    canned: BTreeMap<String, Canned>,
}

impl TestProxyClient {
    /// Start a canned table driven off `clock`.
    #[must_use]
    pub fn builder(clock: &Arc<FakeClock>) -> TestProxyClientBuilder {
        TestProxyClientBuilder {
            clock: Arc::clone(clock),
            canned: BTreeMap::new(),
        }
    }

    /// How many times the fanout actually reached the proxy. `0` proves
    /// a short-circuit returned before dispatch.
    #[must_use]
    pub fn dispatch_count(&self) -> usize {
        self.locked().len()
    }

    /// The recorded dispatches, in the order the proxy saw them.
    #[must_use]
    pub fn dispatched(&self) -> Vec<String> {
        self.locked().iter().map(|row| row.url.clone()).collect()
    }

    /// Bodies exactly as the proxy received them.
    #[must_use]
    pub fn dispatched_bodies(&self) -> Vec<Vec<u8>> {
        self.locked().iter().map(|row| row.body.clone()).collect()
    }

    /// Records one dispatch exactly as the proxy saw it.
    fn record(&self, url: &str, body: Body) {
        let sent_bytes = body
            .into_bytes()
            .map(|buffered| buffered.to_vec())
            .unwrap_or_default();
        self.locked().push(Dispatched {
            url: url.to_owned(),
            body: sent_bytes,
        });
    }

    /// Resolves one canned endpoint: the canned delay on the shared
    /// clock, then its outcome. Recording happened at dispatch, so
    /// both single dispatch and the batch driver reach the wire through here
    /// without either double-recording.
    async fn resolve(
        &self,
        url: &str,
        context: OutboundResponseContext,
    ) -> Result<OutboundHead, EdgeError> {
        let Some(canned) = self.state.canned.get(url) else {
            return Err(EdgeError::not_found(url.to_owned()));
        };
        if !canned.delay.is_zero() {
            self.state.clock.sleep(canned.delay).await;
        }
        match &canned.outcome {
            Outcome::Respond {
                status,
                headers,
                body,
            } => OutboundHead::new(
                StatusCode::from_u16(*status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                OutboundHeaderInput::new(
                    "conformance",
                    headers.clone(),
                    OutboundHeaderFidelity::ProviderFieldBoundariesUnknown,
                ),
                stream::iter([Ok(body.clone().into())]).boxed_local(),
                context,
                OutboundExchangeOwner::new(|| {}),
            ),
            // What an adapter actually raises for a transport fault:
            // every one of the four classifies a non-connect, non-timeout
            // send failure as `BadGateway` carrying the TYPED
            // `Transport` reason. `service_unavailable` would be a
            // stand-in no adapter emits, and the wave classifier reads
            // the typed reason, so the stand-in would send the two arms
            // down different arms of the match for fixture reasons.
            Outcome::Transport => Err(EdgeError::bad_gateway_with_reason(
                "canned transport failure",
                BadGatewayReason::Transport,
            )),
            Outcome::Never => pending::<Result<OutboundHead, EdgeError>>().await,
        }
    }

    /// `resolve`, bounded by the facade's immutable effective deadline.
    ///
    /// Production sets that deadline in `to_outbound` and never
    /// rechecks it in flight: a wave's batch cutoff is its LATEST
    /// member's deadline, so it cannot cut a sibling whose own budget
    /// is tighter. The host is what enforces the per-request value, and
    /// a fixture that ignored it would leave the wave arm's per-call
    /// cancellation row asserting nothing.
    ///
    /// The expiry is reported as the typed `GatewayTimeout` upstream
    /// raises, so the wave classifier takes the arm it would in
    /// production rather than a fixture-only one.
    async fn resolve_until(
        &self,
        url: &str,
        context: OutboundResponseContext,
    ) -> Result<OutboundHead, EdgeError> {
        let budget = context.budget();
        let time = context.time().clone();
        let resolving = self.resolve(url, context);
        match select(Box::pin(resolving), time.wait_until(budget.deadline)).await {
            Either::Left((resolved, _expiry)) => resolved,
            Either::Right((expired, _resolving)) => {
                expired?;
                Err(EdgeError::gateway_timeout_caused(
                    "canned request deadline",
                    budget.cause,
                ))
            }
        }
    }

    fn completion(
        &self,
        request: OutboundDispatchRequest,
    ) -> LocalBoxFuture<'static, (MonotonicInstant, Result<OutboundResponse, EdgeError>)> {
        let clock = request.clock().clone();
        let acquiring = self.dispatch_head(request);
        Box::pin(async move {
            let outcome = match acquiring.await {
                Ok(head) => head.finish().await,
                Err(error) => Err(error),
            };
            (clock.now(), outcome)
        })
    }

    fn locked(&self) -> MutexGuard<'_, Vec<Dispatched>> {
        self.state
            .dispatched
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

impl TestProxyClientBuilder {
    /// Answer `url` immediately with `status` and `body`.
    #[must_use]
    pub fn respond(self, url: &str, status: u16, body: Vec<u8>) -> Self {
        self.respond_with_headers(url, status, HeaderMap::new(), body)
    }

    /// Answer with adapter-visible headers and an encoded provider body.
    ///
    /// The SDK head finalizer applies the prepared response policy, including
    /// decoding, framing and caps; these are not reconstructed by the fixture.
    #[must_use]
    pub fn respond_with_headers(
        self,
        url: &str,
        status: u16,
        headers: HeaderMap,
        body: Vec<u8>,
    ) -> Self {
        self.canned_entry(
            url,
            Duration::ZERO,
            Outcome::Respond {
                status,
                headers,
                body,
            },
        )
    }

    /// Answer `url` once `delay` has elapsed on the shared clock.
    #[must_use]
    pub fn respond_after(self, url: &str, delay: Duration, status: u16, body: Vec<u8>) -> Self {
        self.canned_entry(
            url,
            delay,
            Outcome::Respond {
                status,
                headers: HeaderMap::new(),
                body,
            },
        )
    }

    /// Fail `url` at the transport layer.
    #[must_use]
    pub fn transport_error(self, url: &str) -> Self {
        self.canned_entry(url, Duration::ZERO, Outcome::Transport)
    }

    /// Leave `url` hanging forever, so only a timer arm can win.
    #[must_use]
    pub fn never_responds(self, url: &str) -> Self {
        self.canned_entry(url, Duration::ZERO, Outcome::Never)
    }

    /// Freeze the table into a client.
    #[must_use]
    pub fn build(self) -> TestProxyClient {
        TestProxyClient {
            state: Arc::new(ClientState {
                clock: self.clock,
                canned: self.canned,
                dispatched: Mutex::new(Vec::new()),
            }),
        }
    }

    fn canned_entry(mut self, url: &str, delay: Duration, outcome: Outcome) -> Self {
        self.canned
            .insert(url.to_owned(), Canned { delay, outcome });
        self
    }
}

impl OutboundHttpClient for TestProxyClient {
    fn begin(&self, request: OutboundDispatchRequest) -> Result<OutboundPending, EdgeError> {
        let mut completion = self.completion(request);
        // Start synthetic work before publishing a receiver. A finished recipe
        // retains its original completion rather than being polled twice.
        if let Poll::Ready(result) = completion
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            completion = Box::pin(async move { result });
        }
        Ok(OutboundPending::new(
            Box::pin(async move { completion.await.1 }),
            OutboundExchangeOwner::new(|| {}),
        ))
    }

    fn dispatch_head(&self, request: OutboundDispatchRequest) -> OutboundFuture<OutboundHead> {
        let (parts, context) = request.into_parts();
        let url = parts.uri.to_string();
        self.record(&url, parts.body);
        let client = self.clone();
        Box::pin(async move { client.resolve_until(&url, context).await })
    }

    fn progress(&self) -> Rc<dyn OutboundProgressRuntime> {
        Rc::new(IndependentOutboundProgress)
    }

    /// Every request is recorded at DISPATCH, before any slot resolves,
    /// so the recording assertions see the same requests in the same
    /// order the caller supplied them.
    ///
    /// Completion order: slots are raced against each other, so a slot
    /// whose canned delay is shorter becomes terminal first regardless
    /// of its input index — a delayed endpoint stays pending until a
    /// test advances the shared [`FakeClock`].
    ///
    /// Cutoff: the driver races slot completion against `cutoff` and
    /// emits [`OutboundBatchDriverEvent::Cutoff`] when the cutoff wins
    /// with slots still pending, which is what an `Outcome::Never`
    /// endpoint forces. The wait is served by the shared clock, so a
    /// test that does not advance it holds the batch pending.
    ///
    /// Delay modelling: each slot awaits its own canned delay on the
    /// shared clock, exactly as single dispatch does.
    ///
    /// Cancellation / drop: dropping the batch drops the driver stream
    /// and with it every pending slot future; no further recording
    /// happens, because recording already completed at dispatch.
    fn start_batch_until(&self, batch: OutboundPreparedBatch) -> OutboundBatch {
        let (context, slots) = batch.into_slots();
        let cutoff = context.observation_cutoff();
        let time = context.time().clone();
        // Record at dispatch, ahead of any resolution, so the recorded
        // order is the caller's input order.
        let mut pending = FuturesUnordered::new();
        for (index, request) in slots {
            let budget = request.budget();
            let completion = self.completion(request);
            let slot: LocalBoxFuture<'static, _> = Box::pin(async move {
                let (completed_at, outcome) = completion.await;
                let after_cutoff = cutoff.is_expired_at(completed_at)
                    && !budget.own_deadline_expired_before(cutoff, completed_at);
                (index, completed_at, outcome, after_cutoff)
            });
            pending.push(slot);
        }
        // Every dispatch is recorded before any recipe is polled. Arm all
        // delayed work now, retaining immediate outcomes and their timestamps.
        for slot in &mut pending {
            if let Poll::Ready(result) = slot.as_mut().poll(&mut Context::from_waker(Waker::noop()))
            {
                *slot = Box::pin(async move { result });
            }
        }

        // The SDK owns the shared entry time, slot ledger and preflight
        // outcomes. The driver reports only original indices and timestamps.
        OutboundBatch::from_driver(
            context,
            stream::unfold(Some((pending, time, cutoff, false)), |state| async move {
                let (mut state_slots, state_time, state_cutoff, needs_cutoff) = state?;
                if state_slots.is_empty() {
                    // SDK receipts exclude cutoff-time observations. End with
                    // cutoff only when one was emitted, never as complete EOF.
                    return needs_cutoff.then_some((OutboundBatchDriverEvent::Cutoff, None));
                }
                let next_slot =
                    select(state_slots.next(), state_time.wait_until(state_cutoff)).await;
                match next_slot {
                    Either::Left((Some((index, completed_at, outcome, after_cutoff)), _)) => {
                        Some((
                            OutboundBatchDriverEvent::Item {
                                index,
                                completed_at,
                                outcome,
                            },
                            Some((
                                state_slots,
                                state_time,
                                state_cutoff,
                                needs_cutoff || after_cutoff,
                            )),
                        ))
                    }
                    Either::Left((None, _)) => {
                        needs_cutoff.then_some((OutboundBatchDriverEvent::Cutoff, None))
                    }
                    Either::Right((Ok(()), _)) => Some((OutboundBatchDriverEvent::Cutoff, None)),
                    Either::Right((Err(error), _)) => {
                        Some((OutboundBatchDriverEvent::Failed(error), None))
                    }
                }
            }),
            OutboundExchangeOwner::new(|| {}),
        )
    }

    fn time(&self) -> OutboundTime {
        let source = Arc::clone(&self.state.clock);
        let waits = Arc::clone(&self.state.clock);
        OutboundTime::new(
            MonotonicClock::new(move || source.now_monotonic()),
            move |deadline, clock| {
                let timer = Arc::clone(&waits);
                Box::pin(async move {
                    wait_for_cutoff(timer, clock, deadline).await;
                    Ok(())
                })
            },
        )
    }

    fn validate_supported_options(
        &self,
        _request: &OutboundRequest,
        _mode: OutboundSendMode,
    ) -> Result<(), EdgeError> {
        Ok(())
    }
}

/// Parks until `cutoff` is reached on the SHARED clock.
///
/// `Deadline` is denominated in `EdgeZero`'s monotonic domain, and
/// `Deadline::remaining` reads `MonotonicInstant::now()` — the real
/// clock, which no row advances. Asking it for the remaining span
/// would therefore measure wall time against a budget the test moves
/// only on the `FakeClock`, so this arm could fire early, late, or
/// never, independent of what the row did.
///
/// So the remaining span is computed against the shared clock's own
/// instant instead, and the wait is served by that same clock. A row
/// that advances past the cutoff fires this arm; a row that does not
/// keeps the slot pending, which is what a hanging bidder means.
async fn wait_for_cutoff(timer: Arc<FakeClock>, clock: MonotonicClock, cutoff: Deadline) {
    loop {
        let Some(remaining) = cutoff.remaining_at(clock.now()) else {
            return;
        };
        timer.sleep(remaining).await;
    }
}

/// Counts how many timer arms were built while delegating every sleep
/// to the real `FakeClock`. Arming a timer is the one side effect a
/// short-circuit leaves behind that the dispatch counter cannot see.
struct CountingTimer {
    inner: Arc<FakeClock>,
    created: AtomicUsize,
    armed: AtomicUsize,
    cancelled: AtomicUsize,
}

impl CountingTimer {
    fn over(clock: &Arc<FakeClock>) -> Self {
        Self {
            inner: Arc::clone(clock),
            created: AtomicUsize::new(0),
            armed: AtomicUsize::new(0),
            cancelled: AtomicUsize::new(0),
        }
    }

    fn created(&self) -> usize {
        self.created.load(Ordering::Relaxed)
    }

    fn armed(&self) -> usize {
        self.armed.load(Ordering::Relaxed)
    }

    fn cancelled(&self) -> usize {
        self.cancelled.load(Ordering::Relaxed)
    }
}

struct CountedSleep<'a> {
    inner: Pin<Box<dyn Future<Output = ()> + 'a>>,
    timer: &'a CountingTimer,
    polled: bool,
    completed: bool,
}

impl Future for CountedSleep<'_> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if !self.polled {
            self.timer.armed.fetch_add(1, Ordering::Relaxed);
            self.polled = true;
        }
        let result = self.inner.as_mut().poll(cx);
        self.completed = result.is_ready();
        result
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for CountedSleep<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.timer.cancelled.fetch_add(1, Ordering::Relaxed);
        }
    }
}

// Count creation outside the async body, including never-polled race arms.
impl Timer for CountingTimer {
    fn sleep<'life0, 'async_trait>(
        &'life0 self,
        duration: Duration,
    ) -> Pin<Box<dyn Future<Output = ()> + 'async_trait>>
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        self.created.fetch_add(1, Ordering::Relaxed);
        Box::pin(CountedSleep {
            inner: self.inner.sleep(duration),
            timer: self,
            polled: false,
            completed: false,
        })
    }
}

/// A fanout with no outbound capability at all — the batch-level
/// failure `run_auction` maps to `AuctionError::FanoutUnavailable`.
struct NoOutboundCapabilityFanout;

#[async_trait(?Send)]
impl BidderFanout for NoOutboundCapabilityFanout {
    async fn send_all(
        &self,
        _calls: Vec<BidderCall>,
        _deadline: AuctionDeadline,
    ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
        Err(FanoutInternalError::NoOutboundCapability)
    }
}

/// Which dispatch a Bucket A case drives.
///
/// The arms name the orchestration, not an environment: all three are
/// reachable code, and which one a served request actually takes is
/// stated per arm below rather than implied by its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FanoutUnderTest {
    /// The interim `DevFanout`.
    InterimBridge,
    /// `EdgeZeroFanout` wired to `RaceTimersStrategy`.
    ///
    /// Its `wave` is `None`, so `send_all` takes the strategy branch and
    /// the per-call bridge classifies each answer.
    PerCallBridge,
    /// `EdgeZeroFanout` wired to the wave scheduler — the drive a
    /// served request takes once the boot envelope is installed.
    ///
    /// This is the only arm that reaches the bounded classifier. A row
    /// registered against `InterimBridge` or `PerCallBridge` alone says
    /// nothing about the path production runs.
    BoundedWave,
}

/// Build the fanout under test over `client` and hand it to `body`.
///
/// Every arm wraps the SAME `TestProxyClient`, so a divergence between
/// them is a difference in orchestration and nothing else.
fn with_fanout<R>(
    target: FanoutUnderTest,
    client: &TestProxyClient,
    clock: &Arc<FakeClock>,
    body: impl FnOnce(&dyn BidderFanout) -> R,
) -> R {
    with_fanout_timer(
        target,
        client,
        clock,
        Arc::<FakeClock>::clone(clock),
        wave_limits(),
        body,
    )
}

fn with_fanout_timer<R>(
    target: FanoutUnderTest,
    client: &TestProxyClient,
    clock: &Arc<FakeClock>,
    timer: Arc<dyn Timer + Send + Sync>,
    limits: ResolvedIsolateLimits,
    body: impl FnOnce(&dyn BidderFanout) -> R,
) -> R {
    match target {
        FanoutUnderTest::InterimBridge => {
            let adapter = EdgeZeroProxyAdapter::new(HttpClient::with_client(client.clone()));
            body(&DevFanout {
                client: &adapter,
                clock: clock.as_ref(),
            })
        }
        FanoutUnderTest::PerCallBridge => {
            let as_clock: Arc<dyn Clock + Send + Sync> = Arc::<FakeClock>::clone(clock);
            body(&EdgeZeroFanout::new(
                HttpClient::with_client(client.clone()),
                as_clock,
                timer,
                Arc::new(RaceTimersStrategy),
            ))
        }
        FanoutUnderTest::BoundedWave => {
            let as_clock: Arc<dyn Clock + Send + Sync> = Arc::<FakeClock>::clone(clock);
            // The same constructor `handlers::auction` reaches on a
            // served request, with the limits and gate boot would have
            // installed.
            body(&EdgeZeroFanout::with_wave_scheduler(
                HttpClient::with_client(client.clone()),
                as_clock,
                timer,
                Arc::new(NoopDecodeCooperator),
                Arc::new(RaceTimersStrategy),
                limits,
                StreamGate::new(WAVE_GATE_CAPACITY),
            ))
        }
    }
}

/// How many concurrent body streams the wave arm's gate admits.
///
/// Larger than any conformance row's call count, so the gate never
/// splits a row's wave into several — a split would change what the
/// row observes without changing what it claims to test.
const WAVE_GATE_CAPACITY: u32 = 8;

/// The isolate limits the wave arm charges against.
///
/// Every cap is wide enough that no conformance row trips one: a row
/// here is testing orchestration and classification, so a auctioneer-side
/// size or count refusal firing first would prove something else.
fn wave_limits() -> ResolvedIsolateLimits {
    ResolvedIsolateLimits {
        isolate_total_bytes: 512 * 1024 * 1024,
        baseline_bytes: 0,
        inbound_request_bytes: 256 * 1024,
        inbound_parse_factor: MIN_INBOUND_PARSE_FACTOR,
        max_outbound_request_bytes: 256 * 1024,
        max_outbound_raw_bytes: 256 * 1024,
        max_transport_bytes: 256 * 1024,
        max_decoded_bytes: 256 * 1024,
        response_parse_factor: MIN_RESPONSE_PARSE_FACTOR,
        response_node_peak_bytes: MIN_RESPONSE_NODE_PEAK_BYTES,
        final_response_bytes: 256 * 1024,
        egress_permits: 4,
        response_write_deadline_ms: 10_000,
        guest_decoder_bound_bytes: 0,
        aux_route_permits: 2,
        aux_route_peak_bytes: 1024,
        max_brotli_wbits: 22,
        max_brotli_layers: 1,
        gvl_cache_entries: 1,
        gvl_parsed_bytes: MAX_GVL_PARSED_BYTES,
        max_bidders_per_auction: 8,
        max_outbound_requests: 8,
        max_concurrent_auctions: 4,
        max_active_body_streams: WAVE_GATE_CAPACITY,
        max_runtime_config_bytes: 1024,
        max_runtime_config_blob_bytes: 1024,
        max_parsed_nodes: 1024,
        max_copied_header_bytes: 1024,
        max_admitted_header_bytes: 8192,
        max_admitted_header_count: 64,
        max_impressions_per_auction: 100,
        max_config_json_nodes: DEFAULT_CONFIG_JSON_NODES,
        terminal_response_permits: 2,
        refusal_body_bytes: 1024,
        transmit_state_bytes: 1024,
    }
}

fn fake_clock() -> Arc<FakeClock> {
    Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH))
}

/// An absolute deadline `millis` past the clock's axis origin — the
/// same anchor `advance(millis)` moves the clock to, so a conformance
/// row reads as before even though the axis is now a real instant.
fn deadline_at(clock: &FakeClock, millis: u64) -> AuctionDeadline {
    let origin = clock.origin();
    AuctionDeadline {
        // A conformance budget is milliseconds, so the addition cannot
        // overflow a live clock; falling back to the origin would make
        // the deadline already expired, which every row would notice.
        deadline_at: origin
            .checked_add(Duration::from_millis(millis))
            .unwrap_or(origin),
        source: TmaxSource::Request,
    }
}

#[expect(
    clippy::expect_used,
    reason = "checked conformance fixture must be accepted by the real sink"
)]
fn call_to(url: &str) -> BidderCall {
    BidderCall {
        request: outbound_request::request(
            "POST".to_owned(),
            url.to_owned(),
            Vec::new(),
            br#"{"id":"conformance"}"#.to_vec(),
            vec!["imp-1".to_owned()],
        )
        .expect("sink-built fixture"),
        timeout: Duration::from_millis(GENEROUS_MS),
        max_transport_bytes: u64::try_from(WIDE_CAP).unwrap_or(u64::MAX),
        max_decoded_bytes: u64::try_from(WIDE_CAP).unwrap_or(u64::MAX),
    }
}

fn call_capped(url: &str, cap: u64) -> BidderCall {
    BidderCall {
        max_decoded_bytes: cap,
        ..call_to(url)
    }
}

fn call_with_timeout(url: &str, timeout: Duration) -> BidderCall {
    BidderCall {
        timeout,
        ..call_to(url)
    }
}

struct DriveWake(AtomicBool);

impl Wake for DriveWake {
    fn wake(self: Arc<Self>) {
        self.0.store(true, Ordering::Relaxed);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Drain self-woken SDK/body/decode work before advancing fake time.
/// Return Pending only after a poll parks without scheduling another wake.
///
/// Bounded so a genuinely stuck drive still fails the row instead of
/// hanging it. The bound is far above the stage count any conformance
/// row can produce: its calls are single-digit and each resolves in a
/// fixed number of stages.
fn poll_to_quiescence<F>(driving: &mut Pin<Box<F>>) -> Poll<F::Output>
where
    F: Future + ?Sized,
{
    const MAX_POLLS: usize = 64;
    let wake = Arc::new(DriveWake(AtomicBool::new(false)));
    let waker = Waker::from(Arc::clone(&wake));
    let mut context = Context::from_waker(&waker);
    for polled in 0..MAX_POLLS {
        wake.0.store(false, Ordering::Relaxed);
        if let Poll::Ready(resolved) = driving.as_mut().poll(&mut context) {
            return Poll::Ready(resolved);
        }
        if !wake.0.load(Ordering::Relaxed) {
            return Poll::Pending;
        }
        assert!(
            polled < MAX_POLLS.saturating_sub(1),
            "conformance drive did not park within its fixed poll bound"
        );
    }
    Poll::Pending
}

/// Unwrap the batch-level `Result`. A per-call failure belongs INSIDE
/// a `BidderCallResult`; `FanoutInternalError` is an orchestration
/// fault and no conformance row expects one.
fn rows(outcome: Result<Vec<BidderCallResult>, FanoutInternalError>) -> Vec<BidderCallResult> {
    assert!(
        outcome.is_ok(),
        "per-call failures belong inside the rows, not at the batch level: {outcome:?}",
    );
    outcome.unwrap_or_default()
}

/// Same, for a hand-driven future that must have resolved by now.
fn ready_rows(
    outcome: Poll<Result<Vec<BidderCallResult>, FanoutInternalError>>,
) -> Vec<BidderCallResult> {
    assert!(
        outcome.is_ready(),
        "the fanout must resolve once the clock passes the wake instant",
    );
    match outcome {
        Poll::Ready(resolved) => rows(resolved),
        Poll::Pending => Vec::new(),
    }
}

fn result_at(
    results: &[BidderCallResult],
    index: usize,
) -> Option<&Result<BidderHttpResponse, FanoutError>> {
    results.get(index).map(|row| &row.result)
}

fn elapsed_at(results: &[BidderCallResult], index: usize) -> Option<Duration> {
    results.get(index).map(|row| row.elapsed)
}

fn body_equals_at(results: &[BidderCallResult], index: usize, expected: &[u8]) -> bool {
    matches!(
        result_at(results, index),
        Some(Ok(response)) if response.body_equals(expected)
    )
}

// ---------------------------------------------------------------------
// `run_auction` / handler fixtures
// ---------------------------------------------------------------------

/// A single synthetic bidder pointed at `BIDDER_A`, so the canned
/// proxy table answers whatever `run_auction` dispatches.
fn conformance_bidder_info() -> &'static BidderInfo {
    static INFO: OnceLock<BidderInfo> = OnceLock::new();
    INFO.get_or_init(|| BidderInfo {
        name: "fake_pp".to_owned(),
        endpoint: BIDDER_A.to_owned(),
        uses_https: true,
        ..BidderInfo::default()
    })
}

#[expect(
    clippy::expect_used,
    reason = "unique synthetic conformance registration"
)]
fn conformance_registry() -> BidderRegistry {
    let mut registry = BidderRegistry::new();
    registry
        .register(conformance_bidder_info(), Box::<FakeBidder>::default())
        .expect("unique test bidder");
    registry
}

/// Counts every [`Bidder::make_bids`] entry, then delegates.
///
/// A verdict assertion alone cannot tell "the parser never ran" from
/// "the parser ran and its bids were discarded downstream": both leave
/// an empty `seatbid`. This counter is the only thing that separates
/// them, so a row proving a body never reaches a parser asserts on the
/// count and not just on the response.
struct CountingBidder {
    inner: FakeBidder<'static>,
    parses: Arc<AtomicUsize>,
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
        self.inner.make_requests(request, info, sink)
    }

    fn make_bids(
        &self,
        request: &BidRequest,
        sent: &OutboundRequestMeta,
        response: &BidderHttpResponse,
    ) -> Result<BidderResponse, Vec<BidderError>> {
        self.parses.fetch_add(1, Ordering::Relaxed);
        self.inner.make_bids(request, sent, response)
    }
}

/// A registry whose one adapter tallies its parser entries.
#[expect(
    clippy::expect_used,
    reason = "unique synthetic conformance registration"
)]
fn counting_registry(parses: &Arc<AtomicUsize>) -> BidderRegistry {
    let mut registry = BidderRegistry::new();
    registry
        .register(
            conformance_bidder_info(),
            Box::new(CountingBidder {
                inner: FakeBidder::default(),
                parses: Arc::clone(parses),
            }),
        )
        .expect("unique test bidder");
    registry
}

/// Single-imp banner request naming the one registered bidder, with an
/// explicit `AUCTION_TMAX_MS` budget so neither consumer — the direct
/// `drive_run_auction` call nor the serialized POST through the real
/// handler — depends on the host default to keep the auction deadline
/// in the future.
fn conformance_request() -> BidRequest {
    conformance_request_for("fake_pp")
}

fn conformance_request_for(bidder: &str) -> BidRequest {
    let mut bidders = serde_json::Map::new();
    bidders.insert(bidder.to_owned(), serde_json::json!({}));
    BidRequest {
        id: "auc-conformance".to_owned(),
        tmax: Some(AUCTION_TMAX_MS),
        imp: vec![Imp {
            id: "imp-1".to_owned(),
            banner: Some(Banner {
                w: Some(300_i32),
                h: Some(250_i32),
                ..Banner::default()
            }),
            ext: Some(ImpExt {
                bidder: JsonField::Value(serde_json::Value::Object(bidders)),
                ..ImpExt::default()
            }),
            ..Imp::default()
        }],
        cur: vec!["USD".to_owned()],
        ..BidRequest::default()
    }
}

/// Drive `run_auction` over `fanout` with `seatnonbid` reporting on, so
/// both halves of a fanout-error code pair are observable on the wire.
fn drive_run_auction(
    fanout: &dyn BidderFanout,
    clock: &FakeClock,
) -> Result<BidResponse, AuctionError> {
    drive_run_auction_with(fanout, clock, &conformance_registry())
}

/// Same, over a caller-supplied registry so a row can substitute an
/// instrumented adapter for the plain one.
fn drive_run_auction_with(
    fanout: &dyn BidderFanout,
    clock: &FakeClock,
    registry: &BidderRegistry,
) -> Result<BidResponse, AuctionError> {
    let host = auctioneer_config_data::host_default().clone();
    let account = auctioneer_config_data::account_default();
    let resolved = ResolvedAccount {
        id: UNRESOLVED_ACCOUNT_ID.to_owned(),
        source: AccountSource::DefaultFallback,
        config: account,
    };
    let rng = SeededRng::new(42);
    let overrides: BTreeMap<BidderName, u32> = BTreeMap::new();
    let features = ExchangeFeatures {
        returnallbidstatus_default: true,
        ..ExchangeFeatures::default()
    };
    let prepare_limits = PrepareLimits {
        max_impressions: CONFORMANCE_MAX_IMPRESSIONS,
        max_copied_header_bytes: 4_096,
    };
    let Ok(normalized) = prepare(
        conformance_request(),
        &RequestHeaders::default(),
        &prepare_limits,
    ) else {
        return Err(AuctionError::Internal(
            "fixture request failed prepare".to_owned(),
        ));
    };
    let ctx = AuctionContext::with_request_start_ext(
        clock.now_monotonic(),
        &host,
        &resolved,
        registry,
        fanout,
        clock,
        &rng,
        GvlStore::baked(),
        Gcl::baked(),
        clock.now(),
        false,
        features,
        &overrides,
    );
    let plan = AuctionPlan::new(
        ResolvedAliases::empty(),
        BTreeSet::new(),
        resolve_budgets(
            &AuctioneerAppConfig::default(),
            &host,
            "",
            resolved.source,
            registry,
            &ResolvedAliases::empty(),
        ),
        BTreeMap::new(),
    );
    let mut audit = DisclosureAudit::default();
    block_on(run_auction(&normalized, &ctx, &plan, &mut audit))
}

// ---------------------------------------------------------------------
// Bucket A — runs against all three dispatch arms
// ---------------------------------------------------------------------

/// A response body past the call's cap is `OversizeResponse` with the
/// call-cap origin, and the verdict comes from a real dispatch rather
/// than a pre-flight guess.
///
/// `DevFanout` and the per-call strategy bridge both grade size
/// through auctioneer's own `max_decoded_bytes` check on an already-read
/// body, so the origin is [`OversizeOrigin::CallResponseCap`] on those
/// two arms.
///
/// The wave arm is deliberately NOT registered against this row. It
/// does not perform that check: it narrows the host policy's
/// `max_decoded_bytes` by the call's cap and lets the HOST enforce it
/// during the read, then collects the already-bounded body
/// uncapped. So the origin there is the platform's own `DecodedBody`,
/// not `CallResponseCap`. Both map to the same wire contract — error
/// code 4, `seatnonbid` statuscode 100 — so the divergence is
/// diagnostic fidelity rather than a wire difference, and it is
/// reported rather than encoded here as a second expectation.
///
/// # Panics
/// When the conformance assertion fails.
pub fn oversize_response_yields_oversize_error(target: FanoutUnderTest) {
    let clock = fake_clock();
    let client = TestProxyClient::builder(&clock)
        .respond(BIDDER_A, 200_u16, vec![0_u8; 4096])
        .build();
    let results = with_fanout(target, &client, &clock, |fanout| {
        rows(block_on(fanout.send_all(
            vec![call_capped(BIDDER_A, 100_u64)],
            deadline_at(&clock, GENEROUS_MS),
        )))
    });
    assert_eq!(results.len(), 1, "one call in, one result out");
    assert!(
        matches!(
            result_at(&results, 0),
            Some(Err(FanoutError::OversizeResponse(
                OversizeOrigin::CallResponseCap
            ))),
        ),
        "a body past max_decoded_bytes is OversizeResponse carrying the call-cap origin, \
         got {:?}",
        result_at(&results, 0),
    );
    assert_eq!(
        client.dispatch_count(),
        1,
        "the oversize verdict is post-buffer, so the call was really dispatched",
    );
}

/// Unclassified transport failures never assert an unreachable host.
/// Production preserves the neutral Initial category (4/100), while the
/// legacy development proxy retains its Other mapping (999/100).
///
/// # Panics
/// When the conformance assertion fails.
pub fn transport_error_preserves_category_and_wire_codes(target: FanoutUnderTest) {
    let clock = fake_clock();
    let client = TestProxyClient::builder(&clock)
        .transport_error(BIDDER_A)
        .build();
    let results = with_fanout(target, &client, &clock, |fanout| {
        rows(block_on(fanout.send_all(
            vec![call_to(BIDDER_A)],
            deadline_at(&clock, GENEROUS_MS),
        )))
    });
    assert_eq!(results.len(), 1, "one call in, one result out");
    let (code, message) = match target {
        FanoutUnderTest::InterimBridge | FanoutUnderTest::PerCallBridge => {
            (999_u32, "internal fanout error")
        }
        FanoutUnderTest::BoundedWave => (4_u32, "outbound send failed"),
    };
    let category = match (target, result_at(&results, 0)) {
        (
            FanoutUnderTest::InterimBridge | FanoutUnderTest::PerCallBridge,
            Some(Err(error @ FanoutError::Other(_))),
        )
        | (
            FanoutUnderTest::BoundedWave,
            Some(Err(
                error @ FanoutError::Transport(TransportError {
                    kind: TransportErrorKind::Initial,
                }),
            )),
        ) => Some(error.public_message()),
        _ => None,
    };
    assert_eq!(
        category,
        Some(message),
        "a transport failure retains its fixed category, got {:?}",
        result_at(&results, 0),
    );

    // Same fanout, one auction: the taxonomy has to survive all the way
    // onto the response.
    let response = with_fanout(target, &client, &clock, |fanout| {
        drive_run_auction(fanout, clock.as_ref())
    });
    assert!(
        response.is_ok(),
        "the auction itself succeeds: {response:?}"
    );
    let ext = response
        .ok()
        .and_then(|resp| resp.ext)
        .unwrap_or(serde_json::Value::Null);
    assert_eq!(
        ext.pointer("/errors/fake_pp"),
        Some(&serde_json::json!([{ "code": code, "message": message }])),
        "a transport failure preserves its wire code, got ext {ext}",
    );
    assert_eq!(
        ext.pointer("/prebid/seatnonbid"),
        Some(&serde_json::json!([{
            "seat": "fake_pp",
            "nonbid": [{ "impid": "imp-1", "statuscode": 100_u32 }],
        }])),
        "and seatnonbid statuscode 100, got ext {ext}",
    );
}

/// An empty call list is an empty result set, with nothing dispatched.
///
/// # Panics
/// When the conformance assertion fails.
pub fn zero_calls_returns_empty_result_set(target: FanoutUnderTest) {
    let clock = fake_clock();
    let client = TestProxyClient::builder(&clock).build();
    let results = with_fanout(target, &client, &clock, |fanout| {
        rows(block_on(
            fanout.send_all(Vec::new(), deadline_at(&clock, GENEROUS_MS)),
        ))
    });
    assert!(
        results.is_empty(),
        "zero calls produce zero results, got {results:?}",
    );
    assert_eq!(
        client.dispatch_count(),
        0,
        "an empty call list must not touch the proxy",
    );
}

/// The happy path returns the upstream response and an `elapsed`
/// measured on the INJECTED clock — not zero, and not past the call's
/// own budget.
///
/// # Panics
/// When the conformance assertion fails.
pub fn single_call_success_returns_ok_and_elapsed_within_bounds(target: FanoutUnderTest) {
    let clock = fake_clock();
    let delay = Duration::from_millis(7);
    let client = TestProxyClient::builder(&clock)
        .respond_after(BIDDER_A, delay, 200_u16, b"{\"ok\":true}".to_vec())
        .build();
    let results = with_fanout(target, &client, &clock, |fanout| {
        let mut driving =
            Box::pin(fanout.send_all(vec![call_to(BIDDER_A)], deadline_at(&clock, GENEROUS_MS)));
        assert!(
            poll_to_quiescence(&mut driving).is_pending(),
            "the upstream has not answered at t=0",
        );
        clock.advance(delay);
        ready_rows(poll_to_quiescence(&mut driving))
    });
    assert_eq!(results.len(), 1, "one call in, one result out");
    assert!(
        body_equals_at(&results, 0, b"{\"ok\":true}"),
        "the upstream body is handed back verbatim",
    );
    // Exact equality, not a `<=` bound: `Option`'s derived ordering puts
    // `None` below every `Some`, so a bound would also accept a missing
    // row. Pinning the value covers both claims at once — nonzero, and
    // inside the call's `GENEROUS_MS` budget.
    assert_eq!(
        elapsed_at(&results, 0),
        Some(delay),
        "elapsed is the time the call actually took on the injected clock",
    );
}

/// GDPR enforcement has no retry. A transient upstream failure is dispatched exactly
/// once and reported, never re-sent.
///
/// # Panics
/// When the conformance assertion fails.
pub fn transient_transport_errors_are_not_retried(target: FanoutUnderTest) {
    let clock = fake_clock();
    let client = TestProxyClient::builder(&clock)
        .respond(BIDDER_A, 503_u16, b"upstream busy".to_vec())
        .build();
    let results = with_fanout(target, &client, &clock, |fanout| {
        rows(block_on(fanout.send_all(
            vec![call_to(BIDDER_A)],
            deadline_at(&clock, GENEROUS_MS),
        )))
    });
    assert!(
        matches!(
            result_at(&results, 0),
            Some(Err(FanoutError::BadServerResponse)),
        ),
        "a 5xx is BadServerResponse, got {:?}",
        result_at(&results, 0),
    );
    assert_eq!(
        client.dispatch_count(),
        1,
        "a transient failure is reported, not retried",
    );
    assert_eq!(
        client.dispatched(),
        vec![BIDDER_A.to_owned()],
        "and the single dispatch went to the call's own endpoint",
    );
}

/// `results[i]` corresponds to `calls[i]` even when the calls finish in
/// a different order than they were submitted.
///
/// # Panics
/// When the conformance assertion fails.
pub fn results_preserve_input_order_when_calls_complete_out_of_order(target: FanoutUnderTest) {
    let clock = fake_clock();
    let client = TestProxyClient::builder(&clock)
        .respond_after(BIDDER_A, Duration::from_millis(30), 200_u16, b"a".to_vec())
        .respond_after(BIDDER_B, Duration::from_millis(10), 200_u16, b"b".to_vec())
        .respond_after(BIDDER_C, Duration::from_millis(20), 200_u16, b"c".to_vec())
        .build();
    let results = with_fanout(target, &client, &clock, |fanout| {
        let calls = vec![call_to(BIDDER_A), call_to(BIDDER_B), call_to(BIDDER_C)];
        let mut driving = Box::pin(fanout.send_all(calls, deadline_at(&clock, GENEROUS_MS)));
        assert!(
            poll_to_quiescence(&mut driving).is_pending(),
            "nothing has answered at t=0",
        );
        clock.advance(Duration::from_millis(10));
        assert!(
            poll_to_quiescence(&mut driving).is_pending(),
            "the 10ms endpoint answered but two calls are still in flight",
        );
        clock.advance(Duration::from_millis(10));
        assert!(
            poll_to_quiescence(&mut driving).is_pending(),
            "the 20ms endpoint answered; the 30ms one has not",
        );
        clock.advance(Duration::from_millis(10));
        ready_rows(poll_to_quiescence(&mut driving))
    });
    assert_eq!(results.len(), 3, "three calls in, three results out");
    for (index, expected) in [b"a", b"b", b"c"].into_iter().enumerate() {
        assert!(
            body_equals_at(&results, index, expected),
            "position {index} corresponds to the submitted call, not to completion order",
        );
    }
    assert_eq!(
        (
            elapsed_at(&results, 0),
            elapsed_at(&results, 1),
            elapsed_at(&results, 2),
        ),
        (
            Some(Duration::from_millis(30)),
            Some(Duration::from_millis(10)),
            Some(Duration::from_millis(20)),
        ),
        "the completion order really was 1, 2, 0",
    );
}

/// A slow bidder in the same batch does not lengthen a fast bidder's
/// `elapsed`, and the batch resolves as soon as the SLOWEST call is
/// done — not after the sum of the two.
///
/// # Panics
/// When the conformance assertion fails.
pub fn per_bidder_isolation_fast_bidder_elapsed_matches_own_delay(target: FanoutUnderTest) {
    let clock = fake_clock();
    let fast = Duration::from_millis(5);
    let slow = Duration::from_millis(200);
    let client = TestProxyClient::builder(&clock)
        .respond_after(BIDDER_A, fast, 200_u16, b"fast".to_vec())
        .respond_after(BIDDER_B, slow, 200_u16, b"slow".to_vec())
        .build();
    let results = with_fanout(target, &client, &clock, |fanout| {
        let calls = vec![call_to(BIDDER_A), call_to(BIDDER_B)];
        let mut driving = Box::pin(fanout.send_all(calls, deadline_at(&clock, GENEROUS_MS)));
        assert!(
            poll_to_quiescence(&mut driving).is_pending(),
            "neither bidder has answered at t=0",
        );
        clock.advance(fast);
        assert!(
            poll_to_quiescence(&mut driving).is_pending(),
            "the fast bidder is done, the slow one is not",
        );
        clock.advance(slow.saturating_sub(fast));
        // A serialized fanout would only be STARTING the slow call here
        // and would still be pending.
        ready_rows(poll_to_quiescence(&mut driving))
    });
    assert_eq!(results.len(), 2, "two calls in, two results out");
    assert_eq!(
        elapsed_at(&results, 0),
        Some(fast),
        "the fast bidder is charged its own delay, not the batch's",
    );
    assert_eq!(
        elapsed_at(&results, 1),
        Some(slow),
        "and the slow bidder its own",
    );
}

/// A redirect is `BadServerResponse`, and its body never reaches a
/// bidder parser.
///
/// The fixture answers `307` with a body that is VALID, bid-bearing
/// `OpenRTB`: a redirect carrying junk would be refused by the parser
/// anyway, so it could not tell a status rule from a parse failure.
/// This body would parse clean and its bid would be admitted, which is
/// what makes the status the only thing under test.
///
/// Redirects are the transport telling auctioneer where the resource is,
/// not a bidder answering an auction. auctioneer sends no request it wants
/// followed, so a 3xx is always a misconfigured endpoint or an
/// intercepting proxy, and the entity attached to one belongs to
/// whatever issued the redirect rather than to the bidder.
///
/// Classifying it here rather than in each adapter is the point: an
/// adapter guarding only `status >= 400` admits this body, and that is
/// the live shape across the registry. One rule at the classifier
/// covers every adapter, including ones not written yet.
///
/// The parse counter carries the assertion that the verdict cannot. An
/// empty `seatbid` is equally consistent with a parser that never ran
/// and a parser that ran and had its bids dropped later; only a count
/// of zero rules the second one out.
///
/// # Panics
/// When the conformance assertion fails.
pub fn redirect_is_bad_server_response_and_never_parsed(target: FanoutUnderTest) {
    let clock = fake_clock();
    let client = TestProxyClient::builder(&clock)
        .respond(BIDDER_A, 307_u16, bid_bearing_body())
        .build();
    let results = with_fanout(target, &client, &clock, |fanout| {
        rows(block_on(fanout.send_all(
            vec![call_to(BIDDER_A)],
            deadline_at(&clock, GENEROUS_MS),
        )))
    });
    assert_eq!(results.len(), 1, "one call in, one result out");
    assert!(
        matches!(
            result_at(&results, 0),
            Some(Err(FanoutError::BadServerResponse)),
        ),
        "a 3xx is BadServerResponse at the classifier, got {:?}",
        result_at(&results, 0),
    );
    assert_eq!(
        client.dispatch_count(),
        1,
        "the verdict is read off a real dispatch, not guessed pre-flight",
    );

    // Same fixture through a real auction, over an adapter that tallies
    // every parser entry.
    let parses = Arc::new(AtomicUsize::new(0));
    let outcome = with_fanout(target, &client, &clock, |fanout| {
        drive_run_auction_with(fanout, clock.as_ref(), &counting_registry(&parses))
    });
    assert!(
        outcome.is_ok(),
        "a per-bidder refusal is not an auction failure: {outcome:?}",
    );
    let seatbid = match outcome {
        Ok(response) => response.seatbid,
        Err(_) => Vec::new(),
    };
    assert_eq!(
        parses.load(Ordering::Relaxed),
        0,
        "the redirect body reached a bidder parser; the classifier must \
         refuse it before any adapter sees it",
    );
    assert!(
        seatbid.is_empty(),
        "and no bid from a redirect body reaches the response, got {seatbid:?}",
    );
}

/// Valid, bid-bearing `OpenRTB` — the body a status rule has to refuse on
/// its status alone, because nothing about its CONTENT is refusable.
fn bid_bearing_body() -> Vec<u8> {
    // Raw byte-string fixture: this is a wire body, a `Vec<u8>`.
    // `serde_json::json!` yields a `Value`, so converting would mean
    // `.to_string().into_bytes()` here for no gain.
    br#"{"id":"auc-conformance","cur":"USD","seatbid":[{"seat":"fake_pp","bid":[{"id":"b1","impid":"imp-1","price":1.5}]}]}"#
        .to_vec()
}

// ---------------------------------------------------------------------
// Bucket B — needs a real `Timer`
// ---------------------------------------------------------------------

/// The absolute auction deadline elapsing while a call hangs attributes
/// to `AuctionDeadline`, and the call really was dispatched first —
/// a never-dispatched call could report a timeout for the wrong reason.
///
/// # Panics
/// When the conformance assertion fails.
pub fn deadline_expires_mid_fanout_yields_auction_deadline_cause(target: FanoutUnderTest) {
    let clock = fake_clock();
    let client = TestProxyClient::builder(&clock)
        .never_responds(BIDDER_A)
        .build();
    let results = with_fanout(target, &client, &clock, |fanout| {
        let calls = vec![call_with_timeout(BIDDER_A, Duration::from_millis(500))];
        let mut driving = Box::pin(fanout.send_all(calls, deadline_at(&clock, 50)));
        assert!(
            poll_to_quiescence(&mut driving).is_pending(),
            "the deadline is still ahead at t=0",
        );
        assert_eq!(
            client.dispatch_count(),
            1,
            "the race dispatched before any timer arm could win",
        );
        clock.advance(Duration::from_millis(50));
        ready_rows(poll_to_quiescence(&mut driving))
    });
    assert!(
        matches!(
            result_at(&results, 0),
            Some(Err(FanoutError::Timeout(TimeoutCause::AuctionDeadline))),
        ),
        "the auction deadline ran out first, got {:?}",
        result_at(&results, 0),
    );
    assert_eq!(
        elapsed_at(&results, 0),
        Some(Duration::from_millis(50)),
        "elapsed is the time spent waiting on the injected clock",
    );
}

/// A per-bidder timeout burns only that bidder: its row is
/// `Timeout(Bidder)` while the co-batch bidder keeps its response.
///
/// # Panics
/// When the conformance assertion fails.
pub fn per_call_timeout_expires_yields_bidder_cause(target: FanoutUnderTest) {
    let clock = fake_clock();
    let client = TestProxyClient::builder(&clock)
        .respond(BIDDER_A, 200_u16, b"answered".to_vec())
        .never_responds(BIDDER_B)
        .build();
    let results = with_fanout(target, &client, &clock, |fanout| {
        let calls = vec![
            call_with_timeout(BIDDER_A, Duration::from_millis(500)),
            call_with_timeout(BIDDER_B, Duration::from_millis(50)),
        ];
        let mut driving = Box::pin(fanout.send_all(calls, deadline_at(&clock, 500)));
        assert!(
            poll_to_quiescence(&mut driving).is_pending(),
            "the hanging bidder's own budget is still ahead at t=0",
        );
        assert_eq!(
            client.dispatch_count(),
            2,
            "both calls reached the proxy before any timer arm fired",
        );
        clock.advance(Duration::from_millis(50));
        ready_rows(poll_to_quiescence(&mut driving))
    });
    assert_eq!(results.len(), 2, "two calls in, two results out");
    assert!(
        body_equals_at(&results, 0, b"answered"),
        "the bidder that answered keeps its response",
    );
    assert!(
        matches!(
            result_at(&results, 1),
            Some(Err(FanoutError::Timeout(TimeoutCause::Bidder))),
        ),
        "only the hanging bidder times out, and on its OWN budget: {:?}",
        result_at(&results, 1),
    );
    assert_eq!(
        elapsed_at(&results, 1),
        Some(Duration::from_millis(50)),
        "the per-bidder budget is what elapsed, not the auction deadline",
    );
}

/// A deadline already at or behind `now` returns before any work: no
/// request preparation, no dispatch, no measurable elapsed.
///
/// # Panics
/// When the conformance assertion fails.
pub fn pre_expired_deadline_at_send_all_entry_short_circuits(target: FanoutUnderTest) {
    assert_entry_deadline(target, 51);
}

/// Equality at the entry cutoff is expired on both timed fanout paths.
///
/// # Panics
/// When the conformance assertion fails.
pub fn deadline_equal_to_send_all_entry_short_circuits(target: FanoutUnderTest) {
    assert_entry_deadline(target, 50);
}

#[expect(
    clippy::expect_used,
    reason = "conformance fixture finalization must succeed before fanout"
)]
fn assert_entry_deadline(target: FanoutUnderTest, now_ms: u64) {
    let clock = fake_clock();
    clock.advance(Duration::from_millis(now_ms));
    let client = TestProxyClient::builder(&clock)
        .respond(BIDDER_A, 200, b"never reached a".to_vec())
        .respond(BIDDER_B, 200, b"never reached b".to_vec())
        .build();
    // Encoding is finalized before fanout, as on the served path. The entry
    // guard must not create a timer or submit either sealed request.
    let calls = [BIDDER_A, BIDDER_B]
        .into_iter()
        .map(|url| {
            let mut call = call_to(url);
            let (_, finalized) = auctioneer_bidder::finalize_for_transport(
                call.request,
                &auctioneer_bidder::TransportFinalize {
                    gpc: None,
                    compression: Some(Compression::Gzip),
                    max_encoded_bytes: 1 << 18,
                },
            );
            call.request = finalized.expect("valid conformance request");
            call
        })
        .collect();
    let timer = Arc::new(CountingTimer::over(&clock));
    let mut limits = wave_limits();
    // Refusal takes precedence even when a sealed body exceeds this drive's cap.
    limits.max_outbound_request_bytes = 2;
    let results = with_fanout_timer(
        target,
        &client,
        &clock,
        Arc::<CountingTimer>::clone(&timer),
        limits,
        |fanout| {
            let mut driving = Box::pin(fanout.send_all(calls, deadline_at(&clock, 50)));
            ready_rows(poll_to_quiescence(&mut driving))
        },
    );
    assert_eq!(
        client.dispatch_count(),
        0,
        "expired calls never reach the proxy"
    );
    assert_eq!(timer.created(), 0, "expired entry never creates a timer");
    assert_eq!(timer.armed(), 0, "expired entry never polls a timer");
    assert_eq!(timer.cancelled(), 0, "there is no timer to cancel");
    assert_eq!(results.len(), 2, "exactly one result per input position");
    for index in 0..results.len() {
        assert!(
            matches!(
                result_at(&results, index),
                Some(Err(FanoutError::Timeout(TimeoutCause::AuctionDeadline))),
            ),
            "entry expiry is positional before submission at position {index}, got {:?}",
            result_at(&results, index),
        );
        assert_eq!(
            elapsed_at(&results, index),
            Some(Duration::ZERO),
            "entry refusal performs no timed work"
        );
    }
}

/// The race is biased bidder, deadline, send. A per-bidder timeout and
/// an auction deadline that come due on the SAME wake therefore
/// attribute to the bidder.
///
/// Both budgets are in the FUTURE on purpose: setting them at or behind
/// `now` would hit the pre-expired guard and report `AuctionDeadline`
/// without the tie-break ever running.
///
/// # Panics
/// When the conformance assertion fails.
pub fn equal_call_timeout_and_deadline_ties_to_bidder_cause(target: FanoutUnderTest) {
    let clock = fake_clock();
    let client = TestProxyClient::builder(&clock)
        .never_responds(BIDDER_A)
        .build();
    let both_due_at = Duration::from_millis(100);
    let results = with_fanout(target, &client, &clock, |fanout| {
        let calls = vec![call_with_timeout(BIDDER_A, both_due_at)];
        let mut driving = Box::pin(fanout.send_all(calls, deadline_at(&clock, 100)));
        assert!(
            poll_to_quiescence(&mut driving).is_pending(),
            "both budgets are still ahead at t=0",
        );
        assert_eq!(
            client.dispatch_count(),
            1,
            "the race really started: a never-dispatched call cannot tie anything",
        );
        clock.advance(both_due_at);
        ready_rows(poll_to_quiescence(&mut driving))
    });
    assert!(
        matches!(
            result_at(&results, 0),
            Some(Err(FanoutError::Timeout(TimeoutCause::Bidder))),
        ),
        "a simultaneous fire attributes to the earlier-polled bidder arm, got {:?}",
        result_at(&results, 0),
    );
}

// ---------------------------------------------------------------------
// Batch-level failure mapping
// ---------------------------------------------------------------------

/// A fanout with no outbound capability fails the whole batch. Both
/// layers of that mapping are locked here: `run_auction` turns it into
/// `AuctionError::FanoutUnavailable`, and the handler turns THAT into a
/// plain-text 500.
///
/// A bound config store holding the baked default, so a handler-path
/// row exercises the seam it names rather than the loader's fail-closed
/// answer to an unbound store.
struct DefaultConfigStore;

#[async_trait(?Send)]
impl ConfigStore for DefaultConfigStore {
    async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
        let envelope = BlobEnvelope::new(serde_json::json!({}), "1970-01-01T00:00:00Z".to_owned());
        Ok(Some(
            serde_json::to_string(&envelope).unwrap_or_default().into(),
        ))
    }

    /// The trait's fallback semantics, spelled out: deadline checked
    /// against the caller's clock around the unbounded read, oversized
    /// values rejected.
    async fn get_bounded(
        &self,
        key: &str,
        clock: &MonotonicClock,
        deadline: Deadline,
        max_backend_bytes: u64,
        max_value_bytes: u64,
    ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
        if deadline.is_expired_at(clock.now()) {
            return Err(ConfigStoreError::DeadlineExceeded);
        }
        let value = self.get(key).await?;
        if deadline.is_expired_at(clock.now()) {
            return Err(ConfigStoreError::DeadlineExceeded);
        }
        let backend_bytes = value.as_ref().map_or(Ok(0_u64), |stored_value| {
            u64::try_from(stored_value.len())
                .map_err(|_length_error| ConfigStoreError::ValueTooLarge)
        })?;
        if backend_bytes > max_backend_bytes || backend_bytes > max_value_bytes {
            return Err(ConfigStoreError::ValueTooLarge);
        }
        Ok(BoundedStoreRead {
            backend_bytes,
            value,
        })
    }
}

fn default_config_registry() -> ConfigRegistry {
    StoreRegistry::single_id(
        "auctioneer_runtime".to_owned(),
        ConfigStoreBinding {
            default_key: "auctioneer_runtime".to_owned(),
            handle: ConfigStoreHandle::new(Arc::new(DefaultConfigStore)),
        },
    )
}

/// # Panics
/// When the conformance assertion fails.
pub fn no_outbound_capability_maps_to_fanout_unavailable_and_500() {
    let clock = FakeClock::new(OffsetDateTime::UNIX_EPOCH);
    let outcome = drive_run_auction(&NoOutboundCapabilityFanout, &clock);
    assert!(
        matches!(outcome, Err(AuctionError::FanoutUnavailable)),
        "a batch-level NoOutboundCapability is FanoutUnavailable, got {outcome:?}",
    );

    let (status, content_type, body) = block_on(async {
        let fanout: Arc<dyn BidderFanout + Send + Sync> = Arc::new(NoOutboundCapabilityFanout);
        let handler_clock: Arc<dyn Clock + Send + Sync> =
            Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));
        let Some(input) = handler_request::request() else {
            return (
                StatusCode::OK,
                String::new(),
                b"no enabled serving conformance input".to_vec(),
            );
        };
        let serialized = serde_json::to_vec(&input).unwrap_or_default();
        let Ok(mut request) = Request::builder()
            .method(Method::POST)
            .uri(Uri::from_static("/openrtb2/auction"))
            .header("content-type", "application/json")
            .body(Body::from_bytes(serialized))
        else {
            return (StatusCode::OK, String::new(), Vec::new());
        };
        request.extensions_mut().insert(default_config_registry());
        request.extensions_mut().insert(TestFanoutHandle(fanout));
        request
            .extensions_mut()
            .insert(TestClockHandle(handler_clock));
        request.extensions_mut().insert(TestRngHandle(rng));
        let ctx = RequestContext::new(request, PathParams::default());
        let Ok(response) = auction_handler(ctx).await else {
            return (StatusCode::OK, String::new(), Vec::new());
        };
        let status = response.status();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        let (_parts, body) = response.into_parts();
        let bytes = body
            .into_bytes_bounded(WIDE_CAP)
            .await
            .map(|buffered| buffered.to_vec())
            .unwrap_or_default();
        (status, content_type, bytes)
    });
    assert_eq!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "FanoutUnavailable is a 500",
    );
    assert_eq!(
        content_type, "text/plain; charset=utf-8",
        "and a plain-text one",
    );
    assert_eq!(
        String::from_utf8_lossy(&body),
        "outbound http unavailable\n",
        "with the fixed operator-facing message",
    );
}

/// Initial transport failure on the bounded path renders code 4 / statuscode 100.
/// The bridge-specific 999/100 row remains separate.
///
/// # Panics
/// When the bounded transport contract is violated.
pub fn transport_error_yields_initial_code_4_statuscode_100_on_bounded_wave() {
    let clock = fake_clock();
    let client = TestProxyClient::builder(&clock)
        .transport_error(BIDDER_A)
        .build();
    let results = with_fanout(FanoutUnderTest::BoundedWave, &client, &clock, |fanout| {
        rows(block_on(fanout.send_all(
            vec![call_to(BIDDER_A)],
            deadline_at(&clock, GENEROUS_MS),
        )))
    });
    assert_eq!(results.len(), 1, "one call in, one result out");
    assert!(
        matches!(
            result_at(&results, 0),
            Some(Err(FanoutError::Transport(TransportError {
                kind: TransportErrorKind::Initial,
            })))
        ),
        "unclassified transport failure stays Initial on the bounded path, got {:?}",
        result_at(&results, 0),
    );
    assert_eq!(
        client.dispatch_count(),
        1,
        "no retry after a transport failure"
    );

    let response = with_fanout(FanoutUnderTest::BoundedWave, &client, &clock, |fanout| {
        drive_run_auction(fanout, clock.as_ref())
    });
    assert!(
        response.is_ok(),
        "a positional transport failure does not fail the auction: {response:?}"
    );
    let ext = response
        .ok()
        .and_then(|resp| resp.ext)
        .unwrap_or(serde_json::Value::Null);
    assert_eq!(
        ext.pointer("/errors/fake_pp"),
        Some(&serde_json::json!([{ "code": 4_u32, "message": "outbound send failed" }])),
        "the bounded transport verdict renders code 4, not the bridges' 999",
    );
    assert_eq!(
        ext.pointer("/prebid/seatnonbid"),
        Some(&serde_json::json!([{
            "seat": "fake_pp",
            "nonbid": [{ "impid": "imp-1", "statuscode": 100_u32 }],
        }])),
        "bounded transport failure has seatnonbid statuscode 100",
    );
    assert_eq!(
        client.dispatch_count(),
        2,
        "one dispatch per independent fanout"
    );
}

#[cfg(test)]
mod transport_contract {
    #[test]
    fn transport_error_yields_initial_code_4_statuscode_100_on_bounded_wave() {
        super::transport_error_yields_initial_code_4_statuscode_100_on_bounded_wave();
    }
}

#[cfg(test)]
mod entry_deadline {
    #[test]
    fn pre_expired_deadline_at_send_all_entry_short_circuits_on_bounded_wave() {
        super::pre_expired_deadline_at_send_all_entry_short_circuits(
            super::FanoutUnderTest::BoundedWave,
        );
    }

    #[test]
    fn deadline_equal_to_send_all_entry_short_circuits_on_bounded_wave() {
        super::deadline_equal_to_send_all_entry_short_circuits(super::FanoutUnderTest::BoundedWave);
    }

    #[test]
    fn pre_expired_deadline_at_send_all_entry_short_circuits_on_per_call_bridge() {
        super::pre_expired_deadline_at_send_all_entry_short_circuits(
            super::FanoutUnderTest::PerCallBridge,
        );
    }

    #[test]
    fn deadline_equal_to_send_all_entry_short_circuits_on_per_call_bridge() {
        super::deadline_equal_to_send_all_entry_short_circuits(
            super::FanoutUnderTest::PerCallBridge,
        );
    }

    #[test]
    fn counting_timer_observes_creation_and_cancellation_without_polling() {
        use super::{CountingTimer, Timer as _, block_on, fake_clock};
        use std::time::Duration;

        let clock = fake_clock();
        let timer = CountingTimer::over(&clock);
        let sleep = timer.sleep(Duration::from_millis(50));
        assert_eq!(timer.created(), 1);
        assert_eq!(timer.armed(), 0);
        assert_eq!(timer.cancelled(), 0);
        drop(sleep);
        assert_eq!(timer.cancelled(), 1, "dropping an unpolled arm cancels it");

        block_on(timer.sleep(Duration::ZERO));
        assert_eq!(timer.created(), 2);
        assert_eq!(timer.armed(), 1);
        assert_eq!(
            timer.cancelled(),
            1,
            "completed sleeps are not cancellations"
        );
    }
}
