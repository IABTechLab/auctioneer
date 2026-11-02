//! Spin production-fanout integration rows.
//!
//! # What runs where, and why
//!
//! Every assertion here executes on the HOST. That is a constraint, not
//! a preference: this repository has no wasm runner, and CI builds the
//! wasm targets with `--no-run` for exactly that reason. The adapter's
//! `spin-sdk`-backed piece is therefore reached through the
//! [`DelaySource`] seam — [`SpinTimer`] is generic over it, the
//! production `SpinDelaySource` is `#[cfg(target_arch = "wasm32")]`, and
//! these rows drive a host fake in its place. The cooperator needs no
//! such seam: it yields to the executor, not to the host, so the real
//! implementation runs here unmodified.
//!
//! So what is proven here is the LOGIC: that the timer sleeps for the
//! duration it was asked for, that the wave race observes it, and that
//! the scheduler keeps the guarantees auctioneer owns. What is NOT proven
//! here is the `spin_sdk::time::sleep` binding itself. That binding is
//! compile-verified on `wasm32-wasip2` and could only be exercised by a
//! live Spin component test, which this repository cannot run.
//!
//! # What auctioneer may and may not guarantee on this target
//!
//! Spin reports `outbound-deadlines` and `outbound-batch-cancellation`
//! as `BestEffort`: cancellation stays cooperative until host teardown
//! is observable within a documented bound, and no such bound exists.
//! Dropping the pending batch future is the whole cancellation contract
//! auctioneer has. No row below asserts that the host tore a request down —
//! that is `EdgeZero`'s concern and is `BestEffort` by design. What the
//! rows do assert is auctioneer's own half: no late slot is published as
//! on-time, the wave lease releases by RAII, and each slot keeps the
//! `selected_cause` composed for it.

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use auctioneer_adapter_spin::cooperator::SpinDecodeCooperator;
    use auctioneer_adapter_spin::delay::DelaySource;
    use auctioneer_adapter_spin::limits::OUTBOUND_PROFILE;
    use auctioneer_adapter_spin::timer::SpinTimer;
    use auctioneer_core::outbound::wave::{WaveHost, drive_fanout};
    use auctioneer_core::outbound::{BoundedProxyClientApi, ElapsedSemantics, SupportLevel};
    use auctioneer_core::test_support::outbound_request;
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, FanoutError, TimeoutCause, TmaxSource,
    };
    use auctioneer_runtime::{
        Clock as _, DecodeCooperator, FakeClock, MAX_GVL_PARSED_BYTES, MIN_INBOUND_PARSE_FACTOR,
        MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR, ResolvedIsolateLimits, StreamGate,
        Timer as _,
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
    // The host stand-in for `spin_sdk::time::sleep`
    // ---------------------------------------------------------------------

    /// A [`DelaySource`] that advances a [`FakeClock`] instead of
    /// awaiting the WASI monotonic-clock pollable.
    ///
    /// This is the whole point of the seam. A `FakeClock` moves only
    /// when something advances it, so driving it FROM the delay source
    /// lets a row distinguish a sleep that actually awaited the source
    /// from one that resolved on its first poll — the second never moves
    /// the clock, which is exactly what the immediate-resolve stub did.
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

    /// Increment a shared counter. The counters are `Mutex`-backed rather
    /// than `Cell`-backed because `Timer` and `Clock` both require
    /// `Send + Sync`, so a `Cell` could not live inside one.
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

    /// A response upstream refused to decode, so the slot settles
    /// through the cooperative yield and the one deadline recheck auctioneer
    /// owns.
    ///
    /// The coding is one upstream does NOT implement. A `gzip`,
    /// `deflate`, or `br` body is decoded and stripped upstream, so it
    /// arrives ready to parse and never reaches the recheck this helper
    /// exists to drive.
    fn passthrough_slot(_clock: &Arc<FakeClock>) -> SlotRecipe {
        let mut headers = HeaderMap::new();
        headers.insert(
            "content-encoding",
            "zstd".parse().expect("a static header value parses"),
        );
        SlotRecipe::new(Duration::ZERO, Ok((headers, b"{}".to_vec())))
    }

    /// The shape a `BestEffort` host produces when it ran a slot past the
    /// deadline auctioneer set: upstream observed the expiry itself and
    /// reports it as its own `GatewayTimeout`.
    fn upstream_expiry(elapsed_ms: u64) -> SlotRecipe {
        SlotRecipe::new(
            Duration::from_millis(elapsed_ms),
            Err(EdgeError::gateway_timeout_caused(
                "upstream expiry",
                BudgetSource::BatchCutoff,
            )),
        )
    }

    /// Serves a scripted driver event sequence and records whether the
    /// batch was ever started.
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

    /// A batch whose driver never yields anything, so the wave future
    /// stays pending forever. Dropping that future is how a row
    /// reproduces handler teardown mid-wave.
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
        let timer = SpinTimer::new(ClockAdvancingDelay::new(clock, &delay_calls));
        let limits = test_limits();
        let deadline = auction(clock, auction_millis);
        let results = block_on(drive_fanout(
            calls,
            &deadline,
            WaveHost {
                clock: clock.as_ref(),
                timer: &timer,
                cooperator: &SpinDecodeCooperator,
            },
            client,
            &limits,
            &gate,
        ))
        .expect("the fanout completes");
        (results, gate)
    }

    // ---------------------------------------------------------------------
    // BestEffort host teardown
    // ---------------------------------------------------------------------

    /// A slot whose `BestEffort` host ran past the deadline is REPORTED,
    /// not reinterpreted — and a sibling that answered inside its own
    /// deadline in the SAME batch still publishes `Ok`.
    ///
    /// Both halves matter together. The host overran slot 0's budget and
    /// said so, and auctioneer classifies that through the cause it stored at
    /// composition rather than through anything upstream said about which
    /// budget it thought expired. Slot 1 answered in time; a
    /// post-publication clock compare — one that asked whether the
    /// deadline had passed by the time the BATCH finished — would have
    /// discarded it because slot 0 delayed the batch. auctioneer re-checks
    /// only its OWN work, so it cannot.
    #[test]
    fn spin_upstream_expiry_is_reported_not_reinterpreted() {
        let clock = new_clock();
        let started: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        // Slot 0's host overran its 10ms budget and took 120ms; slot 1
        // answered inside its own 200ms budget. The batch therefore ran
        // well past slot 0's deadline while slot 1 was never late.
        let client = ScriptedClient::new(
            &clock,
            vec![(0, upstream_expiry(120)), (1, ok_slot(&clock))],
            &started,
        );
        let (results, gate) = drive_through_adapter(
            vec![
                call("https://a.example/rtb", 10),
                call("https://b.example/rtb", 200),
            ],
            &client,
            &clock,
            500,
        );

        assert_eq!(read(&started), 1, "exactly one batch was ever dispatched");
        assert!(
            matches!(
                results[0].result,
                Err(FanoutError::Timeout(TimeoutCause::Bidder))
            ),
            "the overrun slot arrives as a GatewayTimeout and classifies as \
             Timeout through the cause composition selected for it: its own \
             10ms budget was the tighter operand",
        );
        assert!(
            results[1].result.is_ok(),
            "and the sibling that answered inside its own deadline publishes \
             Ok; a post-return clock compare would have discarded it because \
             slot 0 delayed the batch",
        );
        assert_eq!(
            gate.free(),
            gate.capacity(),
            "the wave lease releases by RAII once the future completes",
        );
    }

    /// The same upstream error classifies as `AuctionDeadline` when the
    /// auction budget is the tighter operand.
    ///
    /// Stated separately so the property is not read off a single row: if
    /// upstream's own report were driving the cause, both rows would
    /// agree — and they must not.
    ///
    /// This expiry-boundary row also exercises SDK cutoff handling.
    /// The separate pre-expiry rows retain misleading provider causes
    /// without allowing SDK deadline normalization to mask classification.
    #[test]
    fn spin_upstream_expiry_classifies_through_the_stored_cause() {
        let clock = new_clock();
        let started: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        // The auction's 30ms budget is tighter than the call's own 90ms.
        let client = ScriptedClient::new(&clock, vec![(0, upstream_expiry(30))], &started);
        let (results, _gate) =
            drive_through_adapter(vec![call("https://a.example/rtb", 90)], &client, &clock, 30);
        assert!(
            matches!(
                results[0].result,
                Err(FanoutError::Timeout(TimeoutCause::AuctionDeadline))
            ),
            "the cause comes from composition, not from the upstream report",
        );
    }

    /// Before expiry, the SDK preserves provider authorship rather than
    /// replacing the report with its selected deadline and timeout cause.
    #[test]
    fn spin_preexpiry_timeout_preserves_upstream_authorship() {
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
    fn spin_preexpiry_misleading_timeout_uses_composed_cause() {
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

    /// The wave lease releases whether or not the host honored the
    /// cancellation.
    ///
    /// The pending fanout future is DROPPED mid-wave, which is the whole
    /// cancellation contract auctioneer has on a `BestEffort` target.
    /// Deliberately silent about the host side: whether Spin actually
    /// tore the request down is `EdgeZero`'s concern and is not something
    /// auctioneer can observe or promise. What auctioneer owns is that the permit
    /// comes back — a leaked one would deadlock the next auction — and
    /// that nothing surfaces as an on-time result.
    #[test]
    fn spin_lease_released_regardless_of_host_teardown() {
        let clock = new_clock();
        let started: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        let client = NeverResolvingClient {
            clock: Arc::clone(&clock),
            started: Arc::clone(&started),
        };
        let gate = StreamGate::new(4);
        let delay_calls: Arc<Mutex<Vec<Duration>>> = Arc::new(Mutex::new(Vec::new()));
        let timer = SpinTimer::new(ClockAdvancingDelay::new(&clock, &delay_calls));
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
                    cooperator: &SpinDecodeCooperator,
                },
                &client,
                &limits,
                &gate,
            ));
            // One poll gets the wave dispatched and parked on a driver
            // that will never yield. Then the future is dropped.
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
             its scope, whether or not the host honored the cancellation",
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

    // ---------------------------------------------------------------------
    // Host timer
    // ---------------------------------------------------------------------

    /// The wave's acquisition race does not expire a call before its
    /// deadline.
    ///
    /// With a real timer the race resolves in the acquisition's favour
    /// when permits are free, so the wave dispatches and its members get
    /// their full budget. Under the immediate-resolve stub this adapter
    /// installed before, the sleep arm would have been ready on the first
    /// poll every time.
    #[test]
    fn wave_wait_does_not_expire_before_deadline_spin() {
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

    /// `SpinTimer::sleep` awaits its delay source once, for the duration
    /// it was asked for.
    ///
    /// This is the property the immediate-resolve stub lacked: the stub
    /// returns on its first poll and moves nothing, so a caller that
    /// raced it against a deadline would race against a sleep that was
    /// already over. The `spin_sdk::time::sleep` binding that supplies
    /// this on the real target is compile-verified only; what runs here
    /// is the timer logic sitting on top of it.
    #[test]
    fn spin_timer_sleep_awaits_the_host_source() {
        let clock = new_clock();
        let calls: Arc<Mutex<Vec<Duration>>> = Arc::new(Mutex::new(Vec::new()));
        let timer = SpinTimer::new(ClockAdvancingDelay::new(&clock, &calls));

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

    // ---------------------------------------------------------------------
    // Decode cooperator
    // ---------------------------------------------------------------------

    /// A passthrough settle whose CPU time outruns its deadline expires,
    /// and the real cooperator is what the wave yields through on the way
    /// to that recheck.
    ///
    /// The clock advance is attached to the yield only because the yield
    /// is the one point in the settle a test can hook. The property being
    /// asserted is NOT that the yield refreshed the reading — on Spin it
    /// does not have to, because WASI's monotonic clock keeps advancing
    /// across guest CPU work, so the recheck would see the elapsed time
    /// with or without it. That is the difference from the Cloudflare
    /// row, where the yield IS the only thing that can refresh a frozen
    /// `Date.now()` and a no-op cooperator would leave the recheck
    /// permanently inert. Here the recheck works either way, and the
    /// yield earns its place by bounding poll monopolization instead.
    #[test]
    fn spin_settle_recheck_sees_cpu_time_and_expires() {
        /// Advances a clock when awaited, standing in for the CPU time
        /// the settle spent before the recheck reads the clock.
        struct CpuBurningCooperator {
            clock: Arc<FakeClock>,
            burn: Duration,
            yields: Arc<Mutex<usize>>,
        }

        #[async_trait(?Send)]
        impl DecodeCooperator for CpuBurningCooperator {
            async fn yield_to_host(&self) {
                // The real cooperator's yield, then the elapsed CPU.
                SpinDecodeCooperator.yield_to_host().await;
                bump(&self.yields);
                self.clock.advance(self.burn);
            }
        }

        /// A delay source that moves nothing, so the expiry can only come
        /// from the settle rather than from the wave's own wait.
        struct InertDelay;

        #[async_trait(?Send)]
        impl DelaySource for InertDelay {
            async fn delay(&self, _duration: Duration) {}
        }

        let clock = new_clock();
        let started: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        let yields: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        let client = ScriptedClient::new(&clock, vec![(0, passthrough_slot(&clock))], &started);
        let gate = StreamGate::new(4);
        let timer = SpinTimer::new(InertDelay);
        // Each yield surfaces 60ms of CPU the settle spent, past the
        // call's own 50ms budget.
        let cooperator = CpuBurningCooperator {
            clock: Arc::clone(&clock),
            burn: Duration::from_millis(60),
            yields: Arc::clone(&yields),
        };
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
            "the settle yielded exactly once, before its recheck",
        );
        assert!(
            matches!(
                results[0].result,
                Err(FanoutError::Timeout(TimeoutCause::Bidder))
            ),
            "a settle whose CPU time outran the deadline expires, and it \
             classifies through the cause stored at composition",
        );
        assert_eq!(
            gate.free(),
            gate.capacity(),
            "and the wave lease still releases by RAII",
        );
    }

    // ---------------------------------------------------------------------
    // Static profile expectations
    // ---------------------------------------------------------------------

    /// Checks the static Spin profile against hardcoded expected values.
    ///
    /// This does not query SDK capability metadata or exercise provider
    /// behavior. It detects declaration changes relative to these literals,
    /// not SDK drift or deadline/cancellation guarantees. The profile is not
    /// installed or used for runtime enforcement.
    #[test]
    fn capability_table_matches_spin() {
        assert_eq!(
            OUTBOUND_PROFILE.elapsed_semantics,
            ElapsedSemantics::CompletionTime,
            "Spin is Native for outbound-batch-completion-order and \
             outbound-batch-slot-isolation, so each slot reports its own \
             completion rather than its observation",
        );
        assert_eq!(
            OUTBOUND_PROFILE.deadline_support,
            SupportLevel::BestEffort,
            "outbound-deadlines is BestEffort on Spin at the pinned \
             revision; only Axum is Native",
        );
        assert_eq!(
            OUTBOUND_PROFILE.cancellation_support,
            SupportLevel::BestEffort,
            "and so is outbound-batch-cancellation; Spin cancellation stays \
             cooperative until host teardown is observable within a bound",
        );
    }
}
