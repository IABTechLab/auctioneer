//! Fastly production-fanout integration rows.
//!
//! # What runs where, and why
//!
//! Every assertion here executes on the HOST. This repository has no
//! wasm runner and CI builds the wasm targets with `--no-run`, so the
//! `fastly` SDK pieces are unreachable from a test by construction. What
//! IS reachable is everything this task owns: the timer, the decode
//! cooperator, the outbound profile, and the wave loop that runs on top
//! of them.
//!
//! # Why these rows drive the PRODUCTION delay source
//!
//! Unlike the other wasm adapters, this crate's `DelaySource` seam is not
//! a stand-in for a host primitive the test cannot reach. Fastly has no
//! guest-awaitable sleep between dispatch and harvest at all, so
//! `ImmediateDelaySource` — which resolves on its first poll — is what
//! SHIPS. The rows below use it deliberately.
//!
//! Substituting a delaying source here would be the mistake worth naming:
//! it would simulate deadline behaviour Fastly cannot deliver, and every
//! row resting on it would be asserting a guarantee that is absent in
//! production. So the fixtures assert the `BestEffort` loop instead — that
//! it re-reads the clock rather than trusting which arm of a race won,
//! that it emits only the timeouts genuinely due, and that its
//! unproductive re-race is bounded rather than a spin.
//!
//! # What auctioneer may and may not claim on this target
//!
//! Fastly reports `outbound-http` `BestEffort` (dynamic backends),
//! `outbound-deadlines` `BestEffort`, and the `outbound-batch-*` rows
//! `BestEffort` (serial harvest). No row below claims a `Native`
//! cancellation, a deadline-bounded gate wait, or a proven origin-side
//! teardown. The rows assert auctioneer's own half: results stay positional,
//! nothing late is published as on-time, the per-slot metric is named as
//! an observation time for non-timeout outcomes, and leases release by
//! RAII. As on every target, SDK-classified own-deadline timeouts use
//! logical expiry elapsed and may reorder serial harvest observations.

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use auctioneer_adapter_fastly::cooperator::FastlyDecodeCooperator;
    use auctioneer_adapter_fastly::delay::{DelaySource, ImmediateDelaySource};
    use auctioneer_adapter_fastly::limits::OUTBOUND_PROFILE;
    use auctioneer_adapter_fastly::timer::FastlyTimer;
    use auctioneer_core::outbound::wave::{WaveHost, drive_fanout};
    use auctioneer_core::outbound::{BoundedProxyClientApi, ElapsedSemantics, SupportLevel};
    use auctioneer_core::test_support::outbound_request;
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, FanoutError, TmaxSource,
    };
    use auctioneer_runtime::{
        Clock as _, DecodeCooperator as _, FakeClock, MAX_GVL_PARSED_BYTES,
        MIN_INBOUND_PARSE_FACTOR, MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR,
        ResolvedIsolateLimits, StreamGate, Timer as _,
    };
    use edgezero_core::body::Body;
    use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;
    use edgezero_core::error::EdgeError;
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
    use futures::task::noop_waker;
    use std::cell::Cell;
    use std::future::Future;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex, PoisonError};
    use std::task::Context;
    use std::time::Duration;
    use time::OffsetDateTime;

    // -----------------------------------------------------------------
    // Counters
    // -----------------------------------------------------------------

    /// `Mutex`-backed rather than `Cell`-backed because `Timer`,
    /// `DecodeCooperator` and `Clock` all require `Send + Sync`, so a
    /// `Cell` could not live inside one.
    fn bump(counter: &Arc<Mutex<usize>>) {
        let mut count = counter.lock().unwrap_or_else(PoisonError::into_inner);
        *count = count.saturating_add(1);
    }

    fn read(counter: &Arc<Mutex<usize>>) -> usize {
        *counter.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A [`DelaySource`] that counts its calls and, like the production
    /// one, waits for nothing.
    ///
    /// It does NOT delay and does NOT move a clock. That is the point: it
    /// is `ImmediateDelaySource` plus a counter, so a row can bound how
    /// many times the wave loop armed its sleep arm without changing what
    /// the sleep does.
    struct CountingImmediateDelay {
        calls: Arc<Mutex<usize>>,
    }

    #[async_trait(?Send)]
    impl DelaySource for CountingImmediateDelay {
        async fn delay(&self, _duration: Duration) {
            bump(&self.calls);
        }
    }

    // -----------------------------------------------------------------
    // Scheduler fixtures
    // -----------------------------------------------------------------

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
            let events: Vec<_> = script
                .into_iter()
                .map(|(index, recipe)| {
                    let request = slots
                        .iter_mut()
                        .find(|(slot_index, _)| *slot_index == index)
                        .and_then(|(_, request)| request.take())
                        .expect("one prepared request per scripted slot");
                    let completed_at = context
                        .started_at()
                        .checked_add(recipe.elapsed)
                        .expect("the scripted observation is representable");
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

    fn ok_slot_with(_clock: &Arc<FakeClock>, elapsed: Duration, body: &[u8]) -> SlotRecipe {
        SlotRecipe::new(elapsed, Ok((HeaderMap::new(), body.to_vec())))
    }

    /// One observable step of the outbound driver.
    ///
    /// A single shared log with two event kinds rather than two counters:
    /// the property under test is an ORDERING between dispatch and
    /// harvest, which two independent counters could not express.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Event {
        /// `start_wave` was called with this many requests.
        Dispatched(usize),
        /// The driver yielded a terminal slot at this index.
        Harvested(usize),
    }

    /// Logs each dispatch and each harvest, in the order they happen.
    ///
    /// `start_wave` records the dispatch before it builds the driver's
    /// events, and each event records its own harvest, so the log
    /// interleaves the two in real order rather than by construction.
    struct LoggingClient {
        clock: Arc<FakeClock>,
        slots: Cell<Option<Vec<SlotRecipe>>>,
        log: Arc<Mutex<Vec<Event>>>,
    }

    impl LoggingClient {
        fn new(
            clock: &Arc<FakeClock>,
            slots: Vec<SlotRecipe>,
            log: &Arc<Mutex<Vec<Event>>>,
        ) -> Self {
            Self {
                clock: Arc::clone(clock),
                slots: Cell::new(Some(slots)),
                log: Arc::clone(log),
            }
        }
    }

    #[async_trait(?Send)]
    impl BoundedProxyClientApi for LoggingClient {
        fn start_wave(
            &self,
            requests: Vec<OutboundRequest>,
            observation_cutoff: Deadline,
        ) -> Result<OutboundBatch, EdgeError> {
            let count = requests.len();
            self.log
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(Event::Dispatched(count));
            let slots = self.slots.take().unwrap_or_default();
            let log = Arc::clone(&self.log);
            let events: SlotScript = slots
                .into_iter()
                .enumerate()
                .map(|(index, slot)| {
                    log.lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .push(Event::Harvested(index));
                    (index, slot)
                })
                .collect();
            Ok(prepared_batch(
                requests,
                observation_cutoff,
                &self.clock,
                Some(events),
            ))
        }
    }

    /// Serves a scripted driver event sequence and counts batch starts.
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

    /// Answers every wave with one successful slot at index 0.
    ///
    /// [`ScriptedClient`] hands its whole event list to the FIRST batch,
    /// which is wrong for a narrow gate: a one-permit gate splits the
    /// calls across several single-slot waves, and later waves would get
    /// an empty driver and fail the batch contract. This client is
    /// per-wave by construction, so a row can vary gate width without
    /// scripting each wave separately.
    struct PerWaveClient {
        started: Arc<Mutex<usize>>,
        /// The fanout's own clock, so each canned slot's deferred body
        /// deadlines read the source the wave and its timer run on.
        clock: Arc<FakeClock>,
    }

    #[async_trait(?Send)]
    impl BoundedProxyClientApi for PerWaveClient {
        fn start_wave(
            &self,
            requests: Vec<OutboundRequest>,
            observation_cutoff: Deadline,
        ) -> Result<OutboundBatch, EdgeError> {
            bump(&self.started);
            let count = requests.len();
            let events: SlotScript = (0..count)
                .map(|index| {
                    (
                        index,
                        ok_slot_with(&self.clock, Duration::from_millis(5), b"{}"),
                    )
                })
                .collect();
            Ok(prepared_batch(
                requests,
                observation_cutoff,
                &self.clock,
                Some(events),
            ))
        }
    }

    /// Drives a whole fanout through the adapter's REAL timer and
    /// cooperator — the production immediate delay source — over a gate
    /// of the given capacity.
    fn drive_through_adapter(
        calls: Vec<BidderCall>,
        client: &dyn BoundedProxyClientApi,
        clock: &Arc<FakeClock>,
        auction_millis: u64,
        gate_capacity: u32,
    ) -> (Vec<BidderCallResult>, Arc<StreamGate>, Arc<Mutex<usize>>) {
        let gate = StreamGate::new(gate_capacity);
        let sleeps: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        let timer = FastlyTimer::new(CountingImmediateDelay {
            calls: Arc::clone(&sleeps),
        });
        let cooperator = FastlyDecodeCooperator;
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
        (results, gate, sleeps)
    }

    // -----------------------------------------------------------------
    // Dispatch before harvest
    // -----------------------------------------------------------------

    /// Every successful dispatch completes before the first pending
    /// result is harvested.
    ///
    /// The asserted property is dispatch ORDER relative to the first
    /// harvest — NOT simultaneous overlap, which a serial dispatch cannot
    /// guarantee and which this row deliberately does not claim. Fastly
    /// hands a whole wave to one `start_wave` and only then begins
    /// harvesting, so no slot's dispatch waits behind a sibling's result.
    /// That is what makes the fanout a fanout rather than a sequence of
    /// round trips.
    #[test]
    fn fastly_dispatches_every_slot_before_first_harvest() {
        let clock = new_clock();
        let log: Arc<Mutex<Vec<Event>>> = Arc::new(Mutex::new(Vec::new()));
        let client = LoggingClient::new(
            &clock,
            vec![
                ok_slot_with(&clock, Duration::from_millis(30), b"{\"a\":1}"),
                ok_slot_with(&clock, Duration::from_millis(10), b"{\"b\":2}"),
                ok_slot_with(&clock, Duration::from_millis(20), b"{\"c\":3}"),
            ],
            &log,
        );
        let (results, gate, _sleeps) = drive_through_adapter(
            vec![
                call("https://a.example/rtb", 100),
                call("https://b.example/rtb", 100),
                call("https://c.example/rtb", 100),
            ],
            &client,
            &clock,
            200,
            4,
        );

        let events = log.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let first_harvest = events
            .iter()
            .position(|event| matches!(*event, Event::Harvested(_)))
            .expect("at least one slot was harvested");
        let dispatched_total: usize = events
            .iter()
            .filter_map(|event| match *event {
                Event::Dispatched(count) => Some(count),
                Event::Harvested(_) => None,
            })
            .sum();
        let dispatched_before: usize = events
            .iter()
            .take(first_harvest)
            .filter_map(|event| match *event {
                Event::Dispatched(count) => Some(count),
                Event::Harvested(_) => None,
            })
            .sum();

        assert_eq!(
            dispatched_total, 3,
            "all three calls reached a dispatch; the log is {events:?}",
        );
        assert_eq!(
            dispatched_before, dispatched_total,
            "every dispatch completed before the first harvest, so no \
             slot's dispatch waited behind a sibling's result; the log is \
             {events:?}",
        );
        assert!(
            results.iter().all(|result| result.result.is_ok()),
            "and every slot came back on time",
        );
        assert_eq!(
            gate.free(),
            gate.capacity(),
            "with the wave lease released by RAII",
        );
    }

    // -----------------------------------------------------------------
    // Positional results under a serial harvest
    // -----------------------------------------------------------------

    /// Harvest order never reorders `out[i]` relative to `calls[i]`.
    ///
    /// Fastly is `BestEffort` for `outbound-batch-completion-order`: the
    /// harvest is serial, so the order slots are OBSERVED in is not the
    /// order they completed in. Positional integrity is auctioneer's own
    /// guarantee and does not rest on the platform's ordering — each slot
    /// is recorded at its ORIGINAL call index. The row drives the slots
    /// back in an order unrelated to their call positions and still
    /// expects each response beside the call that asked for it.
    #[test]
    fn fastly_results_are_positional_under_harvest_order() {
        let clock = new_clock();
        let started: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        // Harvested 2, 0, 1 — deliberately not the call order.
        let client = ScriptedClient::new(
            &clock,
            vec![
                (
                    2,
                    ok_slot_with(&clock, Duration::from_millis(5), b"{\"slot\":2}"),
                ),
                (
                    0,
                    ok_slot_with(&clock, Duration::from_millis(5), b"{\"slot\":0}"),
                ),
                (
                    1,
                    ok_slot_with(&clock, Duration::from_millis(5), b"{\"slot\":1}"),
                ),
            ],
            &started,
        );
        let (results, _gate, _sleeps) = drive_through_adapter(
            vec![
                call("https://a.example/rtb", 100),
                call("https://b.example/rtb", 100),
                call("https://c.example/rtb", 100),
            ],
            &client,
            &clock,
            200,
            4,
        );

        assert_eq!(read(&started), 1, "one wave, one batch");
        assert_eq!(results.len(), 3, "every call has a positional result");
        for (index, result) in results.into_iter().enumerate() {
            let response = result.result.expect("each slot came back successfully");
            let expected = format!("{{\"slot\":{index}}}");
            assert!(
                response.body_equals(expected.as_bytes()),
                "position {index} holds the response for call {index}, \
                 whatever order the harvest observed them in",
            );
        }
    }

    // -----------------------------------------------------------------
    // The elapsed semantic
    // -----------------------------------------------------------------

    /// Non-timeout results carry the `ObservationTime` semantic, and the
    /// profile NAMES it as such. The common SDK exception is an
    /// own-deadline timeout: its elapsed ends at logical expiry, which
    /// can reorder serial observations, as the next row demonstrates.
    ///
    /// Two halves, both needed. The profile declares
    /// `ElapsedSemantics::ObservationTime` — Fastly is the only target
    /// that does — and the figure reaching a successful result is the
    /// observation the batch reported, carried through unaltered. What the row also pins
    /// is what the value is NOT: it is a DURATION, and nothing converts it
    /// into, or presents it as, a completion timestamp. On a serial
    /// harvest an observation includes waiting out predecessors, so
    /// reading it as a completion instant would misreport every slot
    /// behind a slow one.
    #[test]
    fn fastly_elapsed_is_observation_time_and_named_so() {
        assert_eq!(
            OUTBOUND_PROFILE.elapsed_semantics,
            ElapsedSemantics::ObservationTime,
            "Fastly harvests serially, so a successful slot's elapsed measures \
             when auctioneer OBSERVED it, not when the bidder finished",
        );

        let clock = new_clock();
        let started: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        // A fast bidder observed behind a slow one. Under an observation
        // semantic the reported figures are the observations upstream
        // gave, which is exactly why the semantic has to be declared.
        let observed_slow = Duration::from_millis(70);
        let observed_fast = Duration::from_millis(75);
        let client = ScriptedClient::new(
            &clock,
            vec![
                (0, ok_slot_with(&clock, observed_slow, b"{}")),
                (1, ok_slot_with(&clock, observed_fast, b"{}")),
            ],
            &started,
        );
        let (results, _gate, _sleeps) = drive_through_adapter(
            vec![
                call("https://slow.example/rtb", 150),
                call("https://fast.example/rtb", 150),
            ],
            &client,
            &clock,
            300,
            4,
        );

        assert_eq!(
            results[0].elapsed, observed_slow,
            "the observation is carried through unaltered",
        );
        assert_eq!(
            results[1].elapsed, observed_fast,
            "including a fast bidder's observation, which on a serial \
             harvest includes its predecessor and so exceeds its own \
             latency; the reason the semantic is declared, not assumed",
        );
        assert!(
            results[1].elapsed >= results[0].elapsed,
            "a successful slot observed later reports the larger figure; this is an \
             observation window, never a completion timestamp",
        );
    }

    /// Serial harvest does not override the SDK's common timeout rule.
    /// The late second observation becomes a logical t+20ms timeout and
    /// sorts ahead of the first observation's successful t+70ms result.
    #[test]
    fn fastly_sdk_timeouts_use_logical_expiry_and_reorder_serial_observations() {
        let clock = new_clock();
        let started = Arc::new(Mutex::new(0));
        let client = ScriptedClient::new(
            &clock,
            vec![
                (0, ok_slot_with(&clock, Duration::from_millis(70), b"{}")),
                (1, ok_slot_with(&clock, Duration::from_millis(75), b"{}")),
            ],
            &started,
        );
        let cutoff = Deadline::at_instant(
            clock
                .origin()
                .checked_add(Duration::from_millis(200))
                .expect("the observation cutoff is representable"),
        );
        let mut batch = client
            .start_wave(
                vec![
                    OutboundRequest::get("https://slow.example/rtb")
                        .expect("a static fixture URL parses")
                        .timeout(Duration::from_millis(80)),
                    OutboundRequest::get("https://late.example/rtb")
                        .expect("a static fixture URL parses")
                        .timeout(Duration::from_millis(20)),
                ],
                cutoff,
            )
            .expect("SDK preparation starts the scripted batch");

        assert_eq!(read(&started), 1);
        assert_eq!(
            clock
                .now_monotonic()
                .saturating_duration_since(clock.origin()),
            Duration::from_millis(75),
            "both serial observations occurred on the SDK's paired clock",
        );
        let OutboundBatchNext::Item(timeout) =
            block_on(batch.next()).expect("the SDK delivers the logical timeout")
        else {
            panic!("the batch must deliver a timeout receipt first");
        };
        assert_eq!(timeout.index(), 1, "logical expiry reorders serial harvest");
        assert_eq!(timeout.result().elapsed, Duration::from_millis(20));
        assert!(matches!(
            &timeout.result().outcome,
            Err(EdgeError::GatewayTimeout { .. })
        ));
        let OutboundBatchNext::Item(success) =
            block_on(batch.next()).expect("the on-time sibling remains successful")
        else {
            panic!("the batch must deliver the successful sibling");
        };
        assert_eq!(success.index(), 0);
        assert_eq!(success.result().elapsed, Duration::from_millis(70));
        assert_eq!(
            success.result().outcome.as_ref().unwrap().status(),
            StatusCode::OK
        );
        assert!(matches!(
            block_on(batch.next()).expect("the driver reaches clean EOF"),
            OutboundBatchNext::Finished(OutboundBatchTermination::Completed)
        ));
    }

    // -----------------------------------------------------------------
    // The gate wait — BestEffort
    // -----------------------------------------------------------------

    /// The gate wait is `BestEffort`, and the profile says so.
    ///
    /// Fastly has NO host sleep primitive available to the guest between
    /// dispatch and harvest. The consequence, stated plainly rather than
    /// papered over: the wave-gate wait CAN overrun a slot deadline. A
    /// bare `acquire_wave(k).await` cannot re-read the clock until a
    /// permit wakes it, and no timer can wake it sooner, because there is
    /// no timer to arm.
    ///
    /// So this row asserts the DECLARATION plus auctioneer's own half, and
    /// deliberately does NOT assert a deadline-bounded wait:
    ///
    /// 1. `deadline_support` is `BestEffort`, so the gap is declared.
    /// 2. `FastlyTimer::sleep` resolves immediately whatever it is asked
    ///    for, which is the platform fact the declaration rests on.
    /// 3. auctioneer re-checks the clock and emits the due positional
    ///    timeouts, so nothing late is published as on-time — the half
    ///    auctioneer does own.
    #[test]
    fn fastly_gate_wait_is_besteffort_and_documented() {
        assert_eq!(
            OUTBOUND_PROFILE.deadline_support,
            SupportLevel::BestEffort,
            "the gate wait can overrun a slot deadline, and the profile \
             declares that rather than claiming a bound auctioneer cannot keep",
        );

        // The platform fact underneath: the sleep does not wait.
        let clock = new_clock();
        let timer = FastlyTimer::new(ImmediateDelaySource);
        let before = clock.now_monotonic();
        block_on(timer.sleep(Duration::from_millis(500)));
        assert_eq!(
            clock.now_monotonic(),
            before,
            "sleep resolves without waiting; Compute offers the guest no \
             sleep for it to wait on, which is precisely why the wait \
             cannot be deadline-bounded",
        );

        // auctioneer's own half. A call whose deadline has already passed at
        // fanout entry is published as a timeout and never dispatched,
        // even though no timer could have woken anything.
        let started: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        let client = ScriptedClient::new(&clock, vec![], &started);
        let gate = StreamGate::new(4);
        let wave_timer = FastlyTimer::new(ImmediateDelaySource);
        let cooperator = FastlyDecodeCooperator;
        let limits = test_limits();
        let deadline = auction(&clock, 50);
        clock.advance(Duration::from_millis(80));
        let results = block_on(drive_fanout(
            vec![call("https://a.example/rtb", 40)],
            &deadline,
            WaveHost {
                clock: clock.as_ref(),
                timer: &wave_timer,
                cooperator: &cooperator,
            },
            &client,
            &limits,
            &gate,
        ))
        .expect("the fanout completes");

        assert_eq!(
            read(&started),
            0,
            "expired work is never dispatched, however the wait resolved",
        );
        assert!(
            matches!(results[0].result, Err(FanoutError::Timeout(_))),
            "and the clock recheck publishes it as the timeout it is, so \
             nothing late is ever presented as on-time",
        );
        assert_eq!(
            gate.free(),
            gate.capacity(),
            "with permits given back rather than spent",
        );
    }

    /// The `BestEffort` path still must not emit a pre-dispatch timeout
    /// while the deadline is in the future.
    ///
    /// The other side of the row above. The immediate-resolve timer means
    /// the race's sleep arm is ready on its first poll, so a naive loop
    /// could read "the timer fired" as "the deadline passed" and expire a
    /// call with its whole budget left. It must not: the recheck is
    /// against the CLOCK, not against which arm of the race won. An
    /// uncontended wave dispatches normally.
    #[test]
    fn wave_wait_does_not_expire_before_deadline_fastly() {
        let clock = new_clock();
        let started: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        let client = ScriptedClient::new(
            &clock,
            vec![(0, ok_slot_with(&clock, Duration::from_millis(5), b"{}"))],
            &started,
        );
        let (results, gate, _sleeps) = drive_through_adapter(
            vec![call("https://a.example/rtb", 50)],
            &client,
            &clock,
            200,
            4,
        );

        assert_eq!(
            read(&started),
            1,
            "the wave dispatched rather than expiring in a race whose \
             sleep arm is always ready",
        );
        assert!(
            results[0].result.is_ok(),
            "a call whose deadline has not passed is never published as a \
             timeout, however the race resolved",
        );
        assert_eq!(
            gate.free(),
            gate.capacity(),
            "and its lease is back in the gate",
        );
    }

    /// The wave loop does not busy-spin when the timer resolves at once.
    ///
    /// With an immediate sleep and no call yet due, the acquisition race
    /// is UNPRODUCTIVE: it wins no permits and finds nothing expired. A
    /// loop that simply re-raced would burn the isolate. auctioneer bounds it
    /// — after a fixed number of unproductive passes the wait is served by
    /// awaiting the acquisition alone — so the sleep is armed a bounded
    /// number of times per acquisition rather than without limit.
    ///
    /// The row bounds the count rather than pinning an exact figure: the
    /// property that matters is that it is finite and small, and the
    /// scheduler's own constant is its to change. An unbounded re-race
    /// would not terminate at all, so reaching the assertion is itself
    /// part of the evidence.
    #[test]
    fn fastly_wave_loop_does_not_busy_spin() {
        let clock = new_clock();
        let started: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        // Two calls over a single-permit gate, so the wave loop runs
        // twice and each wave is one slot wide. That is the contended
        // shape where an unbounded re-race would show up as an unbounded
        // sleep count.
        let client = PerWaveClient {
            started: Arc::clone(&started),
            clock: Arc::clone(&clock),
        };
        let (results, gate, sleeps) = drive_through_adapter(
            vec![
                call("https://a.example/rtb", 150),
                call("https://b.example/rtb", 150),
            ],
            &client,
            &clock,
            300,
            1,
        );

        let armed = read(&sleeps);
        // The scheduler's bound is per acquisition, so a generous ceiling
        // still separates "bounded" from "spinning".
        assert!(
            armed <= 64,
            "the sleep was armed {armed} times across two waves — bounded, \
             not spun; the loop stops re-racing after a fixed number of \
             unproductive passes instead of looping on a timer that is \
             always ready",
        );
        assert_eq!(
            read(&started),
            2,
            "the one-permit gate split the calls into two waves, which is \
             the contended shape the bound exists for",
        );
        assert_eq!(results.len(), 2, "and both calls still got a result");
        assert!(
            results.iter().all(|result| result.result.is_ok()),
            "each of which came back successfully rather than expiring in \
             a race it could not win",
        );
        assert_eq!(gate.free(), gate.capacity(), "with every permit returned");
    }

    // -----------------------------------------------------------------
    // Decode cooperator
    // -----------------------------------------------------------------

    /// The cooperator returns one poll to the executor, and claims nothing
    /// about refreshing a clock.
    ///
    /// Fastly's impl is a `yield_now()`, as Spin's is, because the Compute
    /// clock is NOT frozen across CPU work: `Instant` on `wasm32-wasip1`
    /// reads a host clock that advances while the guest computes, so
    /// auctioneer's deadline recheck already sees a fresh instant without a
    /// host round trip.
    ///
    /// It is still not `NoopDecodeCooperator`, which is the distinction
    /// this row draws: a no-op never reaches the executor, so a slot
    /// under it yields no poll to the sibling slots behind it. This yield
    /// returns `Pending` exactly once — one wake, one re-poll — which is
    /// what buys that interleaving without costing more than one pass.
    #[test]
    fn fastly_cooperator_yields_one_poll() {
        let cooperator = FastlyDecodeCooperator;
        let mut future = Box::pin(async {
            cooperator.yield_to_host().await;
        });
        let waker = noop_waker();
        let mut cx = Context::from_waker(&waker);
        let mut polls: usize = 0;
        loop {
            polls = polls.saturating_add(1);
            if Future::poll(future.as_mut(), &mut cx).is_ready() {
                break;
            }
        }

        assert_eq!(
            polls, 2,
            "one poll yields to the executor and the next completes: a \
             real yield, not a no-op that never reaches it, and not a \
             loop that yields more than once",
        );
    }

    // -----------------------------------------------------------------
    // Static profile expectations
    // -----------------------------------------------------------------

    /// Checks the static Fastly profile against hardcoded expected values.
    ///
    /// This does not query SDK capability metadata or exercise provider
    /// behavior. It detects declaration changes relative to these literals,
    /// not SDK drift or deadline/cancellation guarantees. The profile is not
    /// installed or used for runtime enforcement.
    #[test]
    fn capability_table_matches_fastly() {
        assert_eq!(
            OUTBOUND_PROFILE.elapsed_semantics,
            ElapsedSemantics::ObservationTime,
            "Fastly is BestEffort for outbound-batch-completion-order \
             (serial harvest), so each slot reports its observation rather \
             than its own completion: the only profile that does",
        );
        assert_eq!(
            OUTBOUND_PROFILE.deadline_support,
            SupportLevel::BestEffort,
            "outbound-deadlines is BestEffort on Fastly: the guest has no \
             awaitable sleep, so the gate wait can overrun",
        );
        assert_eq!(
            OUTBOUND_PROFILE.cancellation_support,
            SupportLevel::BestEffort,
            "and so is outbound-batch-cancellation; dropping the pending \
             handles cannot be proven to stop origin-side work",
        );
        assert_ne!(
            OUTBOUND_PROFILE.deadline_support,
            SupportLevel::Native,
            "no Native claim on Fastly, for any outbound row",
        );
        assert_ne!(
            OUTBOUND_PROFILE.cancellation_support,
            SupportLevel::Native,
            "including cancellation, whose BestEffort level is what the \
             pinned capability contract reports",
        );
    }
}
