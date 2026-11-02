//! Native integration of Cloudflare's timer/cooperator with the bounded facade.
//!
//! Unlike the shared algorithm suite in `auctioneer-core`, these rows import the
//! adapter and construct its actual host primitives. `TestProxyClient` supplies
//! fake transport and a shared `FakeClock`; `RecordingDelay` stands in for Workers
//! delays. No Fetch, JS event loop, `AbortController`, SDK decoder, or streaming
//! response-cap enforcement runs here. WASM --no-run proves compilation only,
//! not provider-runtime deadlines, finite origin cancellation, or memory bounds.
//! Core-only entry-deadline and transport contracts remain in `auctioneer-core`.

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use auctioneer_adapter_cloudflare::cooperator::CloudflareDecodeCooperator;
    use auctioneer_adapter_cloudflare::delay::DelaySource;
    use auctioneer_adapter_cloudflare::envelope;
    use auctioneer_adapter_cloudflare::timer::CloudflareTimer;
    use auctioneer_core::conformance::TestProxyClient;
    use auctioneer_core::host_primitives::{DelegateToDevStrategy, EdgeZeroFanout};
    use auctioneer_core::test_support::outbound_request;
    use auctioneer_exchange::fanout::TransportErrorKind;
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderFanout as _, FanoutError, TimeoutCause, TmaxSource,
        fanout_error_codes,
    };
    use auctioneer_runtime::{Clock as _, FakeClock, StreamGate, Timer as _};
    use edgezero_core::http::HeaderMap;
    use edgezero_core::outbound::HttpClient;
    use futures::executor::block_on;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex, PoisonError};
    use std::task::{Context, Poll, Waker};
    use std::time::Duration;
    use time::OffsetDateTime;

    const A: &str = "https://a.example/rtb";
    const B: &str = "https://b.example/rtb";
    type Delays = Arc<Mutex<Vec<Duration>>>;

    struct RecordingDelay {
        clock: Arc<FakeClock>,
        calls: Delays,
        cpu_at_yield: Duration,
    }

    #[async_trait(?Send)]
    impl DelaySource for RecordingDelay {
        async fn delay(&self, duration: Duration) {
            self.calls
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(duration);
            if duration.is_zero() {
                self.clock.advance(self.cpu_at_yield);
            } else {
                self.clock.sleep(duration).await;
            }
        }
    }

    fn clock() -> Arc<FakeClock> {
        Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH))
    }

    fn call(url: &str, millis: u64) -> BidderCall {
        BidderCall {
            request: outbound_request::request(
                "POST".into(),
                url.into(),
                vec![],
                b"{}".to_vec(),
                vec![],
            )
            .expect("sink-built fixture"),
            timeout: Duration::from_millis(millis),
            max_transport_bytes: 256 * 1024,
            max_decoded_bytes: 256 * 1024,
        }
    }

    fn auction(clock: &FakeClock, millis: u64) -> AuctionDeadline {
        AuctionDeadline {
            deadline_at: clock
                .now_monotonic()
                .checked_add(Duration::from_millis(millis))
                .unwrap(),
            source: TmaxSource::Request,
        }
    }

    fn fanout(
        clock: &Arc<FakeClock>,
        client: &TestProxyClient,
        gate: &Arc<StreamGate>,
        delays: &Delays,
        cpu_at_yield: Duration,
    ) -> EdgeZeroFanout {
        EdgeZeroFanout::with_wave_scheduler(
            HttpClient::with_client(client.clone()),
            Arc::<FakeClock>::clone(clock),
            Arc::new(CloudflareTimer::new(RecordingDelay {
                clock: Arc::clone(clock),
                calls: Arc::clone(delays),
                cpu_at_yield: Duration::ZERO,
            })),
            Arc::new(CloudflareDecodeCooperator::new(RecordingDelay {
                clock: Arc::clone(clock),
                calls: Arc::clone(delays),
                cpu_at_yield,
            })),
            Arc::new(DelegateToDevStrategy),
            envelope::limits(),
            Arc::clone(gate),
        )
    }

    fn poll<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
        future.poll(&mut Context::from_waker(Waker::noop()))
    }

    fn ready<F: Future>(mut future: Pin<&mut F>) -> F::Output {
        for _ in 0..32_u32 {
            if let Poll::Ready(output) = poll(future.as_mut()) {
                return output;
            }
        }
        panic!("adapter integration did not settle within the poll allowance");
    }

    #[test]
    fn cf_adapter_empty_batch_never_dispatches() {
        let clock = clock();
        let client = TestProxyClient::builder(&clock).build();
        let gate = StreamGate::new(1);
        let delays = Delays::default();
        let fanout = fanout(&clock, &client, &gate, &delays, Duration::ZERO);
        assert!(
            block_on(fanout.send_all(vec![], auction(&clock, 200)))
                .unwrap()
                .is_empty()
        );
        assert_eq!(client.dispatch_count(), 0);
        assert_eq!(gate.free(), gate.capacity());
    }

    #[test]
    fn cf_adapter_preserves_positions_and_completion_elapsed() {
        let clock = clock();
        let client = TestProxyClient::builder(&clock)
            .respond_after(A, Duration::from_millis(30), 200, b"slow".to_vec())
            .respond_after(B, Duration::from_millis(10), 200, b"fast".to_vec())
            .build();
        let gate = StreamGate::new(2);
        let delays = Delays::default();
        let fanout = fanout(&clock, &client, &gate, &delays, Duration::ZERO);
        let mut sending =
            Box::pin(fanout.send_all(vec![call(A, 100), call(B, 100)], auction(&clock, 200)));
        assert!(poll(sending.as_mut()).is_pending());
        clock.advance(Duration::from_millis(10));
        assert!(poll(sending.as_mut()).is_pending());
        clock.advance(Duration::from_millis(20));
        let results = ready(sending.as_mut()).unwrap();
        assert!(results.iter().all(|slot| slot.result.is_ok()));
        assert_eq!(results[0].elapsed, Duration::from_millis(30));
        assert_eq!(results[1].elapsed, Duration::from_millis(10));
        assert_eq!(client.dispatched(), vec![A.to_owned(), B.to_owned()]);
        assert_eq!(gate.free(), gate.capacity());
    }

    #[test]
    fn cf_adapter_transport_error_preserves_category_and_wire_codes() {
        let clock = clock();
        let client = TestProxyClient::builder(&clock).transport_error(A).build();
        let gate = StreamGate::new(1);
        let delays = Delays::default();
        let fanout = fanout(&clock, &client, &gate, &delays, Duration::ZERO);
        let results = block_on(fanout.send_all(vec![call(A, 100)], auction(&clock, 200))).unwrap();
        assert_eq!(results.len(), 1);
        let error = results[0]
            .result
            .as_ref()
            .expect_err("the transport failure stays an error");
        assert!(matches!(
            error,
            FanoutError::Transport(transport) if transport.kind == TransportErrorKind::Initial
        ));
        assert_eq!(error.public_message(), "outbound send failed");
        assert_eq!(fanout_error_codes(error), (4, 100));
        assert_eq!(client.dispatch_count(), 1, "no retry");
        assert_eq!(gate.free(), gate.capacity());
    }

    #[test]
    fn cf_adapter_preexpired_auction_never_dispatches() {
        let clock = clock();
        let client = TestProxyClient::builder(&clock).build();
        let gate = StreamGate::new(1);
        let delays = Delays::default();
        let fanout = fanout(&clock, &client, &gate, &delays, Duration::ZERO);
        let deadline = auction(&clock, 20);
        clock.advance(Duration::from_millis(30));
        let results = block_on(fanout.send_all(vec![call(A, 100)], deadline)).unwrap();
        assert!(matches!(
            results[0].result,
            Err(FanoutError::Timeout(TimeoutCause::AuctionDeadline))
        ));
        assert_eq!(client.dispatch_count(), 0);
        assert_eq!(gate.free(), gate.capacity());
    }

    #[test]
    fn cf_adapter_contended_gate_awaits_its_delay_source() {
        let clock = clock();
        let client = TestProxyClient::builder(&clock).build();
        let gate = StreamGate::new(1);
        let held = block_on(gate.acquire_wave(1)).unwrap();
        let delays = Delays::default();
        let fanout = fanout(&clock, &client, &gate, &delays, Duration::ZERO);
        let mut sending = Box::pin(fanout.send_all(vec![call(A, 50)], auction(&clock, 200)));
        assert!(poll(sending.as_mut()).is_pending());
        assert_eq!(
            *delays.lock().unwrap_or_else(PoisonError::into_inner),
            vec![Duration::from_millis(50)]
        );
        clock.advance(Duration::from_millis(49));
        assert!(poll(sending.as_mut()).is_pending());
        clock.advance(Duration::from_millis(1));
        let results = ready(sending.as_mut()).unwrap();
        assert!(matches!(
            results[0].result,
            Err(FanoutError::Timeout(TimeoutCause::Bidder))
        ));
        assert_eq!(results[0].elapsed, Duration::from_millis(50));
        assert_eq!(client.dispatch_count(), 0);
        drop(held);
        assert_eq!(gate.free(), gate.capacity());
    }

    #[test]
    fn cf_adapter_upstream_expiry_keeps_composed_cause() {
        for (bidder_ms, auction_ms, expected) in [
            (20, 90, TimeoutCause::Bidder),
            (90, 20, TimeoutCause::AuctionDeadline),
            (20, 20, TimeoutCause::Bidder),
        ] {
            let clock = clock();
            let client = TestProxyClient::builder(&clock).never_responds(A).build();
            let gate = StreamGate::new(1);
            let delays = Delays::default();
            let fanout = fanout(&clock, &client, &gate, &delays, Duration::ZERO);
            let mut sending =
                Box::pin(fanout.send_all(vec![call(A, bidder_ms)], auction(&clock, auction_ms)));
            assert!(poll(sending.as_mut()).is_pending());
            clock.advance(Duration::from_millis(20));
            let results = ready(sending.as_mut()).unwrap();
            assert!(
                matches!(results[0].result, Err(FanoutError::Timeout(cause)) if cause == expected)
            );
            assert_eq!(client.dispatch_count(), 1, "no retry");
            assert_eq!(gate.free(), gate.capacity());
        }
    }

    #[test]
    fn cf_adapter_drop_releases_inflight_wave_lease() {
        let clock = clock();
        let client = TestProxyClient::builder(&clock)
            .never_responds(A)
            .respond(B, 200, b"{}".to_vec())
            .build();
        let gate = StreamGate::new(1);
        let delays = Delays::default();
        let fanout = fanout(&clock, &client, &gate, &delays, Duration::ZERO);
        let mut sending = Box::pin(fanout.send_all(vec![call(A, 100)], auction(&clock, 200)));
        assert!(poll(sending.as_mut()).is_pending());
        assert_eq!(client.dispatch_count(), 1);
        assert_eq!(gate.free(), 0);
        drop(sending);
        assert_eq!(gate.free(), gate.capacity());
        let slot = block_on(fanout.send_all(vec![call(B, 100)], auction(&clock, 200)))
            .expect("the released lease can be reused")
            .into_iter()
            .next()
            .expect("one bidder result");
        slot.result.expect("the second bidder succeeds");
        assert_eq!(client.dispatch_count(), 2);
    }

    #[test]
    fn cf_adapter_drop_cancels_queued_gate_wait() {
        let clock = clock();
        let client = TestProxyClient::builder(&clock).build();
        let gate = StreamGate::new(1);
        let held = block_on(gate.acquire_wave(1)).unwrap();
        let delays = Delays::default();
        let fanout = fanout(&clock, &client, &gate, &delays, Duration::ZERO);
        let mut sending = Box::pin(fanout.send_all(vec![call(A, 100)], auction(&clock, 200)));
        assert!(poll(sending.as_mut()).is_pending());
        drop(sending);
        drop(held);
        assert_eq!(client.dispatch_count(), 0);
        assert_eq!(gate.free(), gate.capacity());
    }

    #[test]
    fn cf_adapter_cooperator_refreshes_clock_before_settle_recheck() {
        let clock = clock();
        let mut headers = HeaderMap::new();
        headers.insert("content-encoding", "zstd".parse().unwrap());
        let client = TestProxyClient::builder(&clock)
            .respond_with_headers(A, 200, headers, b"{}".to_vec())
            .build();
        let gate = StreamGate::new(1);
        let delays = Delays::default();
        let fanout = fanout(&clock, &client, &gate, &delays, Duration::from_millis(60));
        let results = block_on(fanout.send_all(vec![call(A, 50)], auction(&clock, 200))).unwrap();
        assert_eq!(
            *delays.lock().unwrap_or_else(PoisonError::into_inner),
            vec![Duration::ZERO]
        );
        assert!(matches!(
            results[0].result,
            Err(FanoutError::Timeout(TimeoutCause::Bidder))
        ));
        assert_eq!(results[0].elapsed, Duration::from_millis(60));
        assert_eq!(client.dispatch_count(), 1);
        assert_eq!(gate.free(), gate.capacity());
    }
}
