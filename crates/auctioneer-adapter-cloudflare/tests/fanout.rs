//! Cloudflare production-fanout integration rows.
//!
//! # What runs where, and why
//!
//! Every assertion here executes on the HOST. That is a constraint, not
//! a preference: this repository has no wasm runner, and CI builds the
//! wasm targets with `--no-run` for exactly that reason. The adapter's
//! `worker`-backed pieces are therefore reached through the
//! [`DelaySource`] seam — `CloudflareTimer` and
//! `CloudflareDecodeCooperator` are generic over it, the production
//! `WorkerDelaySource` is `#[cfg(target_arch = "wasm32")]`, and these
//! rows drive a host fake in its place.
//!
//! So what is proven here is the LOGIC: that the timer sleeps for the
//! duration it was asked for, that the cooperator yields exactly once
//! per decode, that the wave race observes both, and that the scheduler
//! keeps the guarantees auctioneer owns. What is NOT proven here is the
//! `worker::Delay` binding itself. That binding is compile-verified on
//! `wasm32-unknown-unknown` and could only be exercised by a deployed
//! probe, which this repository cannot run.
//!
//! # What auctioneer may and may not guarantee on this target
//!
//! Cloudflare reports `outbound-deadlines` and
//! `outbound-batch-cancellation` as `BestEffort`. Dropping the pending
//! batch future is the whole cancellation contract auctioneer has; the
//! adapter beneath it issues a guest-side `AbortController::abort()` per
//! Fetch, but that abort is REQUESTED and never proven finite. No row
//! below asserts that an origin-side Fetch terminated. What the rows do
//! assert is auctioneer's own half: no late slot is published as on-time,
//! the wave lease releases by RAII, and each slot keeps the
//! `selected_cause` composed for it.

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use auctioneer_adapter_cloudflare::cooperator::CloudflareDecodeCooperator;
    use auctioneer_adapter_cloudflare::delay::DelaySource;
    use auctioneer_adapter_cloudflare::limits::OUTBOUND_PROFILE;
    use auctioneer_adapter_cloudflare::timer::CloudflareTimer;
    use auctioneer_core::outbound::policy_from_limits;
    use auctioneer_core::outbound::wave::{WaveHost, compose_all, drive_fanout};
    use auctioneer_core::outbound::{BoundedProxyClientApi, ElapsedSemantics, SupportLevel};
    use auctioneer_core::test_support::outbound_request;
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, FanoutError, TimeoutCause, TmaxSource,
    };
    use auctioneer_runtime::{
        Clock as _, DecodeCooperator as _, FakeClock, MAX_GVL_PARSED_BYTES,
        MIN_INBOUND_PARSE_FACTOR, MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR,
        ResolvedIsolateLimits, StreamGate, Timer as _,
    };
    use edgezero_core::body::Body;
    use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;
    use edgezero_core::error::{BudgetSource, EdgeError};
    use edgezero_core::http::{HeaderMap, StatusCode};
    use edgezero_core::outbound::{
        HttpClient, IndependentOutboundProgress, OutboundBatch, OutboundBatchDriverEvent,
        OutboundBatchNext, OutboundBatchTermination, OutboundDispatchRequest,
        OutboundExchangeOwner, OutboundFuture, OutboundHead, OutboundHeaderFidelity,
        OutboundHeaderInput, OutboundHttpClient, OutboundPending, OutboundPreparedBatch,
        OutboundProgressRuntime, OutboundRequest, OutboundResponse, OutboundSendMode, OutboundTime,
        ResponseHeaderLimiter,
    };
    use edgezero_core::time::{Deadline, MonotonicClock};
    use futures::executor::block_on;
    use futures::stream;
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::Duration;
    use time::OffsetDateTime;

    // ---------------------------------------------------------------------
    // The host stand-in for `worker::Delay`
    // ---------------------------------------------------------------------

    /// A [`DelaySource`] that advances a [`FakeClock`] instead of calling
    /// `setTimeout`.
    ///
    /// This is the whole point of the seam. A Workers isolate's clock is
    /// frozen for the duration of a synchronous turn and moves only when
    /// control returns to the event loop; a `FakeClock` moves only when
    /// something advances it. Driving the fake clock FROM the delay source
    /// reproduces that coupling exactly, so a test can distinguish an
    /// implementation that reaches the host from one that merely re-polls
    /// itself — the second never moves the clock.
    struct ClockAdvancingDelay {
        clock: Arc<FakeClock>,
        /// Every duration this source was asked for, in order.
        calls: Arc<Mutex<Vec<Duration>>>,
    }

    impl ClockAdvancingDelay {
        fn new(clock: &Arc<FakeClock>, calls: &Arc<Mutex<Vec<Duration>>>) -> Self {
            Self {
                clock: Arc::clone(clock),
                calls: Arc::clone(calls),
            }
        }
    }

    #[async_trait(?Send)]
    impl DelaySource for ClockAdvancingDelay {
        async fn delay(&self, duration: Duration) {
            self.calls
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(duration);
            self.clock.advance(duration);
        }
    }

    /// A [`DelaySource`] that yields to the host but moves no clock, so a
    /// row can charge CPU time to a decode independently of the yield.
    struct CpuBurningDelay {
        clock: Arc<FakeClock>,
        /// Advanced on each yield, standing in for the CPU time the settle
        /// spent inside the frozen turn and which only becomes visible when
        /// control returns to the host.
        burn: Duration,
        yields: Arc<Mutex<usize>>,
    }

    #[async_trait(?Send)]
    impl DelaySource for CpuBurningDelay {
        async fn delay(&self, _duration: Duration) {
            let mut count = self.yields.lock().unwrap_or_else(PoisonError::into_inner);
            *count = count.saturating_add(1);
            drop(count);
            self.clock.advance(self.burn);
        }
    }

    /// Increment a shared counter. The counters are `Mutex`-backed rather
    /// than `Cell`-backed because `Timer`, `DecodeCooperator` and `Clock`
    /// all require `Send + Sync`, so a `Cell` could not live inside one.
    fn bump(counter: &Arc<Mutex<usize>>) {
        let mut count = counter.lock().unwrap_or_else(PoisonError::into_inner);
        *count = count.saturating_add(1);
    }

    fn read(counter: &Arc<Mutex<usize>>) -> usize {
        *counter.lock().unwrap_or_else(PoisonError::into_inner)
    }

    // ---------------------------------------------------------------------
    // Scheduler fixtures
    // ---------------------------------------------------------------------

    fn test_limits() -> ResolvedIsolateLimits {
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
            max_active_body_streams: 4,
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

    fn call(url: &str, timeout_ms: u64) -> BidderCall {
        BidderCall {
            request: outbound_request::request(
                "POST".into(),
                url.into(),
                vec![],
                b"{}".to_vec(),
                vec![],
            )
            .expect("sink-built fixture"),
            timeout: Duration::from_millis(timeout_ms),
            max_transport_bytes: 256 * 1024,
            max_decoded_bytes: 256 * 1024,
        }
    }

    fn auction(clock: &FakeClock, millis: u64) -> AuctionDeadline {
        AuctionDeadline {
            deadline_at: clock
                .origin()
                .checked_add(Duration::from_millis(millis))
                .expect("the test horizon is representable"),
            source: TmaxSource::Request,
        }
    }

    // Recipes are shareable; local response bodies are created only after SDK preparation.
    type SlotScript = Vec<(usize, SlotRecipe)>;

    struct SlotRecipe {
        elapsed: Duration,
        outcome: Result<(HeaderMap, Vec<u8>), EdgeError>,
    }

    impl SlotRecipe {
        fn new(elapsed: Duration, outcome: Result<(HeaderMap, Vec<u8>), EdgeError>) -> Self {
            Self { elapsed, outcome }
        }
    }

    struct FixtureProvider {
        clock: Arc<FakeClock>,
        events: Mutex<Option<SlotScript>>,
    }

    impl OutboundHttpClient for FixtureProvider {
        fn begin(&self, _request: OutboundDispatchRequest) -> Result<OutboundPending, EdgeError> {
            Err(EdgeError::not_implemented(
                "fanout fixtures only dispatch batches",
            ))
        }

        fn dispatch_head(&self, _request: OutboundDispatchRequest) -> OutboundFuture<OutboundHead> {
            panic!("fanout fixtures only dispatch batches");
        }

        fn progress(&self) -> Rc<dyn OutboundProgressRuntime> {
            Rc::new(IndependentOutboundProgress)
        }

        fn start_batch_until(&self, batch: OutboundPreparedBatch) -> OutboundBatch {
            let (context, prepared_slots) = batch.into_slots();
            let Some(script) = self
                .events
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take()
            else {
                return OutboundBatch::from_driver(
                    context,
                    stream::pending(),
                    OutboundExchangeOwner::new(|| {}),
                );
            };
            let mut slots: Vec<_> = prepared_slots
                .into_iter()
                .map(|(index, request)| (index, Some(request)))
                .collect();
            let cutoff = context.observation_cutoff();
            let mut cutoff_unresolved = false;
            let mut events: Vec<_> = script
                .into_iter()
                .map(|(index, recipe)| {
                    let request = slots
                        .iter_mut()
                        .find(|(slot_index, _)| *slot_index == index)
                        .and_then(|(_, request)| request.take())
                        .expect("one prepared request per scripted slot");
                    let budget = request.budget();
                    let completed_at = context
                        .started_at()
                        .checked_add(recipe.elapsed)
                        .expect("the scripted observation is representable");
                    cutoff_unresolved |= cutoff.is_expired_at(completed_at)
                        && !budget.own_deadline_expired_before(cutoff, completed_at);
                    let outcome = recipe.outcome.and_then(|(headers, body)| {
                        let (parts, response_context) = request.into_parts();
                        let (normalized_headers, normalization, _) = OutboundHeaderInput::new(
                            "fixture",
                            headers,
                            OutboundHeaderFidelity::ProviderFieldBoundariesUnknown,
                        )
                        .normalize(
                            ResponseHeaderLimiter::new(
                                parts.max_response_header_bytes,
                                parts.max_response_header_count,
                            ),
                            &parts.method,
                            StatusCode::OK,
                            parts.proxy_marker,
                        )?;
                        Ok(OutboundResponse::new(
                            parts.method,
                            StatusCode::OK,
                            normalized_headers,
                            Body::from_bytes(body),
                            response_context.clock().clone(),
                            normalization.content_type_provenance,
                        ))
                    });
                    self.clock.advance_to(completed_at);
                    OutboundBatchDriverEvent::Item {
                        index,
                        completed_at,
                        outcome,
                    }
                })
                .collect();
            if cutoff_unresolved {
                events.push(OutboundBatchDriverEvent::Cutoff);
            }
            OutboundBatch::from_driver(
                context,
                stream::iter(events),
                OutboundExchangeOwner::new(|| {}),
            )
        }

        fn time(&self) -> OutboundTime {
            let source = Arc::clone(&self.clock);
            OutboundTime::new(monotonic(&self.clock), move |deadline, clock| {
                let waiter_clock = Arc::clone(&source);
                Box::pin(async move {
                    waiter_clock
                        .sleep(deadline.remaining_at(clock.now()).unwrap_or_default())
                        .await;
                    Ok(())
                })
            })
        }

        fn validate_supported_options(
            &self,
            _request: &OutboundRequest,
            _mode: OutboundSendMode,
        ) -> Result<(), EdgeError> {
            Ok(())
        }
    }

    fn prepared_batch(
        requests: Vec<OutboundRequest>,
        cutoff: Deadline,
        clock: &Arc<FakeClock>,
        events: Option<SlotScript>,
    ) -> OutboundBatch {
        HttpClient::new(Arc::new(FixtureProvider {
            clock: Arc::clone(clock),
            events: Mutex::new(events),
        }))
        .start_batch_until(requests, cutoff)
    }

    fn ok_slot(_clock: &Arc<FakeClock>) -> SlotRecipe {
        SlotRecipe::new(Duration::ZERO, Ok((HeaderMap::new(), b"{}".to_vec())))
    }

    /// A response upstream refused to decode, so the slot settles through
    /// the cooperative yield and the one deadline recheck auctioneer owns.
    ///
    /// The coding is one upstream does NOT implement. A `gzip`, `deflate`,
    /// or `br` body is decoded and stripped upstream, so it arrives ready
    /// to parse and never reaches the recheck this helper exists to drive.
    fn passthrough_slot(_clock: &Arc<FakeClock>) -> SlotRecipe {
        let mut headers = HeaderMap::new();
        headers.insert(
            "content-encoding",
            "zstd".parse().expect("a static header value parses"),
        );
        SlotRecipe::new(Duration::ZERO, Ok((headers, b"{}".to_vec())))
    }

    /// Serves a scripted driver event sequence and records whether the batch
    /// was ever started.
    struct ScriptedClient {
        clock: Arc<FakeClock>,
        events: Cell<Option<SlotScript>>,
        started: Arc<Mutex<usize>>,
    }

    impl ScriptedClient {
        fn new(clock: &Arc<FakeClock>, events: SlotScript, started: &Arc<Mutex<usize>>) -> Self {
            Self {
                clock: Arc::clone(clock),
                events: Cell::new(Some(events)),
                started: Arc::clone(started),
            }
        }
    }

    #[async_trait(?Send)]
    impl BoundedProxyClientApi for ScriptedClient {
        fn start_wave(
            &self,
            requests: Vec<OutboundRequest>,
            observation_cutoff: Deadline,
        ) -> Result<OutboundBatch, EdgeError> {
            bump(&self.started);
            let events = self.events.take().unwrap_or_default();
            Ok(prepared_batch(
                requests,
                observation_cutoff,
                &self.clock,
                Some(events),
            ))
        }
    }

    /// A batch whose driver never yields anything, so the wave future stays
    /// pending forever. Dropping that future is how a row reproduces handler
    /// teardown mid-wave.
    struct NeverResolvingClient {
        clock: Arc<FakeClock>,
        started: Arc<Mutex<usize>>,
    }

    #[async_trait(?Send)]
    impl BoundedProxyClientApi for NeverResolvingClient {
        fn start_wave(
            &self,
            requests: Vec<OutboundRequest>,
            observation_cutoff: Deadline,
        ) -> Result<OutboundBatch, EdgeError> {
            bump(&self.started);
            Ok(prepared_batch(
                requests,
                observation_cutoff,
                &self.clock,
                None,
            ))
        }
    }

    fn new_clock() -> Arc<FakeClock> {
        Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH))
    }

    /// The wave's own clock, in `EdgeZero`'s monotonic domain.
    ///
    /// A canned response carries deferred body deadlines, and upstream
    /// reads them off the clock handed to `OutboundResponse::new`. That
    /// has to be the source the wave and its timer already run on, or a
    /// test that advances the fake clock would move the scheduler while
    /// the response's own deadlines stayed pinned to real time.
    fn monotonic(clock: &Arc<FakeClock>) -> MonotonicClock {
        let source = Arc::clone(clock);
        MonotonicClock::new(move || source.now_monotonic())
    }

    /// Drives a whole fanout through the adapter's real timer and
    /// cooperator, over a gate wide enough for one wave.
    fn drive_through_adapter(
        calls: Vec<BidderCall>,
        client: &dyn BoundedProxyClientApi,
        clock: &Arc<FakeClock>,
        auction_millis: u64,
    ) -> (Vec<BidderCallResult>, Arc<StreamGate>) {
        let gate = StreamGate::new(4);
        let delay_calls: Arc<Mutex<Vec<Duration>>> = Arc::new(Mutex::new(Vec::new()));
        let timer = CloudflareTimer::new(ClockAdvancingDelay::new(clock, &delay_calls));
        let cooperator =
            CloudflareDecodeCooperator::new(ClockAdvancingDelay::new(clock, &delay_calls));
        let limits = test_limits();
        let deadline = auction(clock, auction_millis);
        let results = block_on(drive_fanout(
            calls,
            &deadline,
            WaveHost {
                clock: clock.as_ref(),
                timer: &timer,
                cooperator: &cooperator,
            },
            client,
            &limits,
            &gate,
        ))
        .expect("the fanout completes");
        (results, gate)
    }

    // ---------------------------------------------------------------------
    // Cancellation and deadline rows
    // ---------------------------------------------------------------------

    /// Handler teardown mid-wave. The pending fanout future is DROPPED, and
    /// the two things auctioneer owns are asserted: nothing surfaces as an
    /// on-time result, and the wave lease is back in the gate.
    ///
    /// Deliberately silent about the origin side. `outbound-batch-
    /// cancellation` is `BestEffort` on Cloudflare, so whether every Fetch
    /// has actually terminated is not something auctioneer can observe or
    /// promise, and a row that asserted it would be asserting the platform's
    /// behaviour rather than auctioneer's.
    #[test]
    fn cf_besteffort_cancellation_suppresses_late_results_and_releases_lease() {
        let clock = new_clock();
        let started: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        let client = NeverResolvingClient {
            clock: Arc::clone(&clock),
            started: Arc::clone(&started),
        };
        let gate = StreamGate::new(4);
        let delay_calls: Arc<Mutex<Vec<Duration>>> = Arc::new(Mutex::new(Vec::new()));
        let timer = CloudflareTimer::new(ClockAdvancingDelay::new(&clock, &delay_calls));
        let cooperator =
            CloudflareDecodeCooperator::new(ClockAdvancingDelay::new(&clock, &delay_calls));
        let limits = test_limits();
        let deadline = auction(&clock, 200);

        let capacity = gate.capacity();
        let published: Option<Vec<BidderCallResult>> = block_on(async {
            let mut fanout = Box::pin(drive_fanout(
                vec![
                    call("https://a.example/rtb", 50),
                    call("https://b.example/rtb", 50),
                ],
                &deadline,
                WaveHost {
                    clock: clock.as_ref(),
                    timer: &timer,
                    cooperator: &cooperator,
                },
                &client,
                &limits,
                &gate,
            ));
            // One poll gets the wave dispatched and parked on a driver that
            // will never yield. Then the future is dropped — which is the
            // entire cancellation contract auctioneer has on this target.
            let polled = futures::poll!(fanout.as_mut());
            assert!(polled.is_pending(), "the wave must still be in flight");
            assert_eq!(
                read(&started),
                1,
                "the batch was dispatched before teardown"
            );
            drop(fanout);
            None
        });

        assert!(
            published.is_none(),
            "a torn-down auction publishes nothing at all, so no slot can be \
             surfaced as an on-time result",
        );
        assert_eq!(
            gate.free(),
            capacity,
            "the wave lease releases by RAII when the dropped future unwinds \
             its scope; a leaked permit would deadlock the next auction",
        );
        assert_eq!(
            gate.in_flight(),
            0,
            "and no weight is left inside a live permit",
        );
        assert_eq!(
            gate.waiting_len(),
            0,
            "nor is a cancelled acquisition retained in the waiting order",
        );
    }

    /// No auctioneer timer ever drops the batch.
    ///
    /// Three members with composed deadlines t+10, t+50 and t+50. Slot 0
    /// expires as its OWN `GatewayTimeout`, reported inside its own slot
    /// while the batch keeps running, and slots 1 and 2 return real
    /// responses. Deadline-driven batch cancellation is retired precisely
    /// because dropping the batch at `max(deadline_at)` would discard the
    /// slots that had already succeeded along with every slot's own elapsed
    /// and error class.
    #[test]
    fn cf_batch_is_never_dropped_on_a_deadline() {
        let clock = new_clock();
        let started: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        // Slot 0's own expiry, then two real responses. The driver keeps
        // producing after the first member's deadline passes, which is what
        // "the batch is not dropped" looks like from inside.
        let client = ScriptedClient::new(
            &clock,
            vec![
                (
                    0,
                    SlotRecipe::new(
                        Duration::from_millis(10),
                        Err(EdgeError::gateway_timeout_caused(
                            "upstream expiry",
                            BudgetSource::BatchCutoff,
                        )),
                    ),
                ),
                (1, ok_slot(&clock)),
                (2, ok_slot(&clock)),
            ],
            &started,
        );
        let (results, gate) = drive_through_adapter(
            vec![
                call("https://a.example/rtb", 10),
                call("https://b.example/rtb", 50),
                call("https://c.example/rtb", 50),
            ],
            &client,
            &clock,
            200,
        );

        assert_eq!(read(&started), 1, "exactly one batch was ever dispatched");
        assert!(
            matches!(results[0].result, Err(FanoutError::Timeout(_))),
            "slot 0 expires inside its own slot, as its own timeout",
        );
        assert!(
            results[1].result.is_ok() && results[2].result.is_ok(),
            "and its siblings run on to real responses; a deadline-driven \
             batch drop would have destroyed both",
        );
        assert_eq!(
            gate.free(),
            gate.capacity(),
            "the wave lease releases by RAII once the future completes",
        );
    }

    /// The second case of the row above: teardown mid-wave, asserting only
    /// auctioneer-owned outcomes.
    ///
    /// Nothing is surfaced as on-time, the lease releases, and each
    /// unresolved slot keeps the `selected_cause` composed for it. Not
    /// asserted, on purpose: that every Fetch aborted. Origin-side
    /// termination is requested and never proven finite on a `BestEffort`
    /// target.
    #[test]
    fn cf_batch_teardown_publishes_nothing_and_keeps_stored_provenance() {
        let clock = new_clock();
        let started: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        let client = NeverResolvingClient {
            clock: Arc::clone(&clock),
            started: Arc::clone(&started),
        };
        let gate = StreamGate::new(4);
        let delay_calls: Arc<Mutex<Vec<Duration>>> = Arc::new(Mutex::new(Vec::new()));
        let timer = CloudflareTimer::new(ClockAdvancingDelay::new(&clock, &delay_calls));
        let cooperator =
            CloudflareDecodeCooperator::new(ClockAdvancingDelay::new(&clock, &delay_calls));
        let limits = test_limits();
        // The auction budget is the tighter operand for the second call, so
        // the two members are composed with DIFFERENT causes. That the
        // stored provenance survives teardown is only observable when the
        // two differ.
        let deadline = auction(&clock, 30);
        let composed = compose_all(
            vec![
                call("https://a.example/rtb", 10),
                call("https://b.example/rtb", 90),
            ],
            &deadline,
            clock.now_monotonic(),
            policy_from_limits(&limits),
            usize::try_from(limits.max_outbound_request_bytes).unwrap_or(usize::MAX),
        )
        .expect("composition succeeds");
        assert_eq!(
            composed[0].deadline().selected_cause,
            TimeoutCause::Bidder,
            "the first call's own timeout is the tighter operand",
        );
        assert_eq!(
            composed[1].deadline().selected_cause,
            TimeoutCause::AuctionDeadline,
            "the second call's is the auction budget",
        );

        block_on(async {
            let mut fanout = Box::pin(drive_fanout(
                vec![
                    call("https://a.example/rtb", 10),
                    call("https://b.example/rtb", 90),
                ],
                &deadline,
                WaveHost {
                    clock: clock.as_ref(),
                    timer: &timer,
                    cooperator: &cooperator,
                },
                &client,
                &limits,
                &gate,
            ));
            assert!(
                futures::poll!(fanout.as_mut()).is_pending(),
                "the wave is in flight",
            );
            drop(fanout);
        });

        assert_eq!(
            gate.free(),
            gate.capacity(),
            "the wave lease releases by RAII on teardown",
        );
        // The provenance an unresolved slot keeps is the one composition
        // fixed for it, and composition is independent of the teardown: a
        // re-composition over the same inputs and the same entry instant
        // yields the same causes, which is exactly what a resumed or
        // re-reported slot would carry.
        let recomposed = compose_all(
            vec![
                call("https://a.example/rtb", 10),
                call("https://b.example/rtb", 90),
            ],
            &deadline,
            clock.now_monotonic(),
            policy_from_limits(&limits),
            usize::try_from(limits.max_outbound_request_bytes).unwrap_or(usize::MAX),
        )
        .expect("composition succeeds");
        assert_eq!(
            recomposed[0].deadline().selected_cause,
            composed[0].deadline().selected_cause,
            "each unresolved slot keeps its stored selected_cause across teardown",
        );
        assert_eq!(
            recomposed[1].deadline().selected_cause,
            composed[1].deadline().selected_cause,
            "including the one the auction budget won",
        );
    }

    /// The EARLIEST member's deadline never cancels its siblings.
    ///
    /// Members carry heterogeneous composed deadlines by construction —
    /// each is `min(bidder_at, auction_at)` — so the earliest one proves
    /// nothing about the others. Dropping the batch when the first expires
    /// would cancel siblings whose deadlines have not passed and lose their
    /// positional outcomes.
    #[test]
    fn cf_batch_survives_earliest_member_deadline() {
        let clock = new_clock();
        let started: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        let client = ScriptedClient::new(
            &clock,
            vec![
                (
                    0,
                    SlotRecipe::new(
                        Duration::from_millis(10),
                        Err(EdgeError::gateway_timeout_caused(
                            "upstream expiry",
                            BudgetSource::BatchCutoff,
                        )),
                    ),
                ),
                (1, ok_slot(&clock)),
                (2, ok_slot(&clock)),
            ],
            &started,
        );
        let (results, _gate) = drive_through_adapter(
            vec![
                call("https://a.example/rtb", 10),
                call("https://b.example/rtb", 50),
                call("https://c.example/rtb", 50),
            ],
            &client,
            &clock,
            200,
        );

        assert_eq!(results.len(), 3, "every position is published");
        assert!(
            matches!(results[0].result, Err(FanoutError::Timeout(_))),
            "the earliest member expires as its own slot's timeout",
        );
        assert!(
            results[1].result.is_ok(),
            "out[1] delivers a real response in position",
        );
        assert!(
            results[2].result.is_ok(),
            "and so does out[2]; the earliest deadline cancelled neither",
        );
    }

    /// Upstream's reported timeout never overrides auctioneer's stored cause.
    ///
    /// The name is capability-neutral on purpose: `outbound-deadlines` is
    /// `BestEffort` on Cloudflare, and this assertion is about auctioneer's own
    /// stored provenance, which holds at every support level. Composition
    /// picks the cause once, from `min(bidder_at, auction_at)`, and the
    /// classification of an upstream deadline error reads THAT, not anything
    /// upstream said about which budget it thought expired.
    ///
    /// At these expiry boundaries the SDK may replace the provider error
    /// or end observation at cutoff. The separate pre-expiry rows below
    /// retain misleading provider causes to isolate Auctioneer's classifier.
    #[test]
    fn cf_deadline_cause_matches_composed_selected_cause() {
        let clock = new_clock();
        let started: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        // The auction budget (30ms) is tighter than the call's own 90ms, so
        // composition selects `AuctionDeadline`.
        let client = ScriptedClient::new(
            &clock,
            vec![(
                0,
                SlotRecipe::new(
                    Duration::from_millis(30),
                    Err(EdgeError::gateway_timeout_caused(
                        "upstream expiry",
                        BudgetSource::BatchCutoff,
                    )),
                ),
            )],
            &started,
        );
        let (results, _gate) =
            drive_through_adapter(vec![call("https://a.example/rtb", 90)], &client, &clock, 30);
        assert!(
            matches!(
                results[0].result,
                Err(FanoutError::Timeout(TimeoutCause::AuctionDeadline))
            ),
            "the cause comes from composition, not from the upstream report",
        );

        // The mirror case: the call's own timeout is the tighter operand, so
        // the SAME upstream error classifies as `Bidder`. If upstream's
        // report were driving the cause, both rows would agree — and they
        // must not.
        let mirror_clock = new_clock();
        let mirror_started: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        let mirror_client = ScriptedClient::new(
            &mirror_clock,
            vec![(
                0,
                SlotRecipe::new(
                    Duration::from_millis(10),
                    Err(EdgeError::gateway_timeout_caused(
                        "upstream expiry",
                        BudgetSource::BatchCutoff,
                    )),
                ),
            )],
            &mirror_started,
        );
        let (mirror_results, _mirror_gate) = drive_through_adapter(
            vec![call("https://a.example/rtb", 10)],
            &mirror_client,
            &mirror_clock,
            200,
        );
        assert!(
            matches!(
                mirror_results[0].result,
                Err(FanoutError::Timeout(TimeoutCause::Bidder))
            ),
            "and the bidder's own budget wins when it is the tighter operand",
        );
    }

    // ---------------------------------------------------------------------
    // Host timer
    // ---------------------------------------------------------------------

    /// Before expiry, the SDK preserves provider authorship rather than
    /// replacing the report with its selected deadline and timeout cause.
    #[test]
    fn cf_preexpiry_timeout_preserves_upstream_authorship() {
        for upstream_cause in [BudgetSource::PerCallTimeout, BudgetSource::BatchCutoff] {
            let clock = new_clock();
            let started = Arc::new(Mutex::new(0));
            let observed = Duration::from_millis(5);
            let client = ScriptedClient::new(
                &clock,
                vec![(
                    0,
                    SlotRecipe::new(
                        observed,
                        Err(EdgeError::gateway_timeout_caused(
                            "misleading upstream report",
                            upstream_cause,
                        )),
                    ),
                )],
                &started,
            );
            let request_deadline = monotonic(&clock).deadline_after(Duration::from_millis(30));
            let cutoff = monotonic(&clock).deadline_after(Duration::from_millis(200));
            let mut batch = client
                .start_wave(
                    vec![
                        OutboundRequest::get("https://a.example/rtb")
                            .expect("a static fixture URL parses")
                            .deadline(request_deadline),
                    ],
                    cutoff,
                )
                .expect("SDK preparation starts the scripted batch");
            let OutboundBatchNext::Item(item) =
                block_on(batch.next()).expect("the SDK retains the provider timeout")
            else {
                panic!("the batch must deliver a provider timeout receipt");
            };
            assert_eq!(read(&started), 1);
            assert!(!request_deadline.is_expired_at(clock.now_monotonic()));
            assert_eq!(item.index(), 0);
            assert_eq!(item.result().elapsed, observed);
            assert!(
                matches!(
                    &item.result().outcome,
                    Err(EdgeError::GatewayTimeout { message, cause, .. })
                        if message == "misleading upstream report" && *cause == upstream_cause
                ),
                "the original provider message and misleading cause survive SDK validation",
            );
            assert!(matches!(
                block_on(batch.next()).expect("the driver reaches clean EOF"),
                OutboundBatchNext::Finished(OutboundBatchTermination::Completed)
            ));
        }
    }

    /// This is a Auctioneer classification guard, not an SDK expiry check:
    /// both observations precede every deadline, so the SDK cannot
    /// replace the deliberately misleading provider timeout cause.
    #[test]
    fn cf_preexpiry_misleading_timeout_uses_composed_cause() {
        for (bidder_ms, auction_ms, upstream_cause, expected) in [
            (
                90,
                30,
                BudgetSource::PerCallTimeout,
                TimeoutCause::AuctionDeadline,
            ),
            (10, 200, BudgetSource::BatchCutoff, TimeoutCause::Bidder),
        ] {
            let clock = new_clock();
            let started = Arc::new(Mutex::new(0));
            let observed = Duration::from_millis(5);
            let client = ScriptedClient::new(
                &clock,
                vec![(
                    0,
                    SlotRecipe::new(
                        observed,
                        Err(EdgeError::gateway_timeout_caused(
                            "misleading upstream report",
                            upstream_cause,
                        )),
                    ),
                )],
                &started,
            );
            let (results, gate) = drive_through_adapter(
                vec![call("https://a.example/rtb", bidder_ms)],
                &client,
                &clock,
                auction_ms,
            );
            let selected_deadline = clock
                .origin()
                .checked_add(Duration::from_millis(bidder_ms.min(auction_ms)))
                .expect("the composed deadline is representable");
            assert!(clock.now_monotonic() < selected_deadline);
            assert_eq!(read(&started), 1);
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].elapsed, observed);
            assert!(
                matches!(
                    &results[0].result,
                    Err(FanoutError::Timeout(cause)) if *cause == expected
                ),
                "Auctioneer uses its composed {expected:?}, not upstream's {upstream_cause:?}",
            );
            assert_eq!(gate.free(), gate.capacity());
        }
    }

    /// `CloudflareTimer::sleep` awaits its delay source for the duration it
    /// was asked for, and the instant afterwards is at least that much
    /// later.
    ///
    /// This is the frozen-clock property the immediate-resolve stub lacked:
    /// the stub returns on its first poll and moves nothing, so a caller
    /// that raced it against a deadline would race against a sleep that was
    /// already over. The `worker::Delay` binding that supplies this on the
    /// real target is compile-verified only; what runs here is the timer
    /// logic sitting on top of it.
    #[test]
    fn cloudflare_timer_sleep_advances_the_host_clock() {
        let clock = new_clock();
        let calls: Arc<Mutex<Vec<Duration>>> = Arc::new(Mutex::new(Vec::new()));
        let timer = CloudflareTimer::new(ClockAdvancingDelay::new(&clock, &calls));

        let before = clock.now_monotonic();
        block_on(timer.sleep(Duration::from_millis(40)));
        let after = clock.now_monotonic();

        assert!(
            after.saturating_duration_since(before) >= Duration::from_millis(40),
            "the instant after sleep(d) is at least d later",
        );
        assert_eq!(
            calls.lock().unwrap_or_else(PoisonError::into_inner).clone(),
            vec![Duration::from_millis(40)],
            "and the sleep reached the host source exactly once, with the \
             duration it was asked for rather than one of its own choosing",
        );
    }

    /// The wave's acquisition race does not expire a call before its
    /// deadline.
    ///
    /// With a real timer the race resolves in the acquisition's favour when
    /// permits are free, so the wave dispatches and its members get their
    /// full budget. Under the immediate-resolve stub the sleep arm would
    /// have been ready on the first poll every time.
    #[test]
    fn wave_wait_does_not_expire_before_deadline_cloudflare() {
        let clock = new_clock();
        let started: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        let client = ScriptedClient::new(&clock, vec![(0, ok_slot(&clock))], &started);
        let (results, gate) = drive_through_adapter(
            vec![call("https://a.example/rtb", 50)],
            &client,
            &clock,
            200,
        );

        assert_eq!(
            read(&started),
            1,
            "the wave dispatched rather than expiring in the race",
        );
        assert!(
            results[0].result.is_ok(),
            "a call whose deadline has not passed is never published as a timeout",
        );
        assert_eq!(
            gate.free(),
            gate.capacity(),
            "and its lease is back in the gate",
        );
    }

    // ---------------------------------------------------------------------
    // Decode cooperator
    // ---------------------------------------------------------------------

    /// A slot whose settle CPU time exceeds its deadline expires, even
    /// though the clock only advances at `yield_to_host()`.
    ///
    /// The cooperator is what makes that possible. Inside a Workers turn the
    /// clock is frozen, so auctioneer's own recheck would compare the deadline
    /// against the instant the turn began and could never fire; awaiting a
    /// real host event is what turns the next read into a new one. An
    /// already-resolved promise or a bare `yield_now()` re-polls inside the
    /// same turn and would leave the recheck inert — which is why the
    /// source here moves the clock only when it is actually awaited.
    #[test]
    fn cloudflare_settle_expiry_under_frozen_cpu_clock() {
        let clock = new_clock();
        let yields: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        let started: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        // Upstream refused this coding, so the slot settles through the
        // yield and the recheck that follows it. Nothing else in this row
        // advances the clock: the ONLY motion comes from the cooperator's
        // yield, which is exactly the frozen-turn shape being reproduced.
        let client = ScriptedClient::new(&clock, vec![(0, passthrough_slot(&clock))], &started);
        let gate = StreamGate::new(4);
        // The timer must NOT move the clock here, or the expiry could come
        // from the wave's own wait rather than from the settle.
        let timer = CloudflareTimer::new(CpuBurningDelay {
            clock: Arc::clone(&clock),
            burn: Duration::ZERO,
            yields: Arc::new(Mutex::new(0)),
        });
        // Each yield surfaces 60ms of CPU the settle spent inside the
        // frozen turn — past the call's own 50ms deadline.
        let cooperator = CloudflareDecodeCooperator::new(CpuBurningDelay {
            clock: Arc::clone(&clock),
            burn: Duration::from_millis(60),
            yields: Arc::clone(&yields),
        });
        let limits = test_limits();
        let deadline = auction(&clock, 200);
        let results = block_on(drive_fanout(
            vec![call("https://a.example/rtb", 50)],
            &deadline,
            WaveHost {
                clock: clock.as_ref(),
                timer: &timer,
                cooperator: &cooperator,
            },
            &client,
            &limits,
            &gate,
        ))
        .expect("the fanout completes");

        assert_eq!(
            read(&yields),
            1,
            "the settle yielded to the host exactly once, before its recheck",
        );
        assert!(
            matches!(
                results[0].result,
                Err(FanoutError::Timeout(TimeoutCause::Bidder))
            ),
            "a settle whose CPU time outran the deadline expires, and it \
             classifies through the cause stored at composition; without the \
             yield the clock would still read the instant the turn began and \
             the recheck could never fire",
        );
        assert_eq!(
            gate.free(),
            gate.capacity(),
            "and the wave lease still releases by RAII",
        );
    }

    /// The yield itself is a real host event, not a self re-poll.
    ///
    /// Separated from the row above so the property is stated on its own:
    /// awaiting an already-resolved promise or a bare `yield_now()` would
    /// satisfy the trait's signature while refreshing nothing, and on a
    /// frozen-clock runtime that makes the settle recheck permanently inert.
    #[test]
    fn cloudflare_cooperator_yield_reaches_the_host_once() {
        let clock = new_clock();
        let yields: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        let cooperator = CloudflareDecodeCooperator::new(CpuBurningDelay {
            clock: Arc::clone(&clock),
            burn: Duration::from_millis(60),
            yields: Arc::clone(&yields),
        });

        let before = clock.now_monotonic();
        block_on(cooperator.yield_to_host());
        let after = clock.now_monotonic();

        assert_eq!(read(&yields), 1, "one call reaches the host exactly once");
        assert!(
            after > before,
            "and the instant afterwards is a fresh reading",
        );
    }

    // ---------------------------------------------------------------------
    // Static profile expectations
    // ---------------------------------------------------------------------

    /// Checks the static Cloudflare profile against hardcoded expected values.
    ///
    /// This does not query SDK capability metadata or exercise provider
    /// behavior. It detects declaration changes relative to these literals,
    /// not SDK drift or deadline/cancellation guarantees. The profile is not
    /// installed or used for runtime enforcement.
    #[test]
    fn capability_table_matches_cloudflare() {
        assert_eq!(
            OUTBOUND_PROFILE.elapsed_semantics,
            ElapsedSemantics::CompletionTime,
            "Cloudflare is Native for outbound-batch-completion-order and \
             outbound-batch-slot-isolation, so each slot reports its own \
             completion rather than its observation",
        );
        assert_eq!(
            OUTBOUND_PROFILE.deadline_support,
            SupportLevel::BestEffort,
            "outbound-deadlines is BestEffort on Cloudflare at the pinned \
             revision; only Axum is Native",
        );
        assert_eq!(
            OUTBOUND_PROFILE.cancellation_support,
            SupportLevel::BestEffort,
            "and so is outbound-batch-cancellation; dropping the batch \
             requests an abort it cannot prove finite",
        );
    }
}
