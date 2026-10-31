//! Completed response storage outlives the stream leases that collected it.

#[cfg(test)]
use auctioneer_runtime::test_support::alloc_track;

#[global_allocator]
static ALLOCATOR: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use super::fanout_limits as envelope;
    use auctioneer_core::test_support::outbound_request;

    use super::alloc_track;
    use auctioneer_core::outbound::BoundedFetch;
    use auctioneer_core::outbound::wave::{WaveHost, drive_fanout};
    use auctioneer_exchange::{AuctionDeadline, BidderCall, TmaxSource};
    use auctioneer_runtime::{Clock as _, FakeClock, NoopDecodeCooperator, StreamGate, Timer as _};
    use edgezero_core::body::Body;
    use edgezero_core::error::EdgeError;
    use edgezero_core::http::{HeaderMap, StatusCode};
    use edgezero_core::outbound::{
        ContentTypeProvenance, HttpClient, IndependentOutboundProgress, OutboundBatch,
        OutboundBatchDriverEvent, OutboundDispatchRequest, OutboundExchangeOwner, OutboundFuture,
        OutboundHead, OutboundHttpClient, OutboundPending, OutboundPreparedBatch,
        OutboundProgressRuntime, OutboundRequest, OutboundResponse, OutboundSendMode, OutboundTime,
    };
    use edgezero_core::time::MonotonicClock;
    use futures::{executor::block_on, stream};
    use std::future::Future as _;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll, Waker};
    use std::time::Duration;
    use time::OffsetDateTime;

    struct Control {
        waves: AtomicUsize,
        bodies: AtomicUsize,
        released: AtomicUsize,
    }

    struct LazyBodies {
        control: Arc<Control>,
        clock: Arc<FakeClock>,
        body_bytes: usize,
    }

    impl OutboundHttpClient for LazyBodies {
        fn begin(&self, _: OutboundDispatchRequest) -> Result<OutboundPending, EdgeError> {
            Err(EdgeError::not_implemented(
                "retention fixture must use batch dispatch",
            ))
        }

        fn dispatch_head(&self, _: OutboundDispatchRequest) -> OutboundFuture<OutboundHead> {
            panic!("retention fixture must use batch dispatch");
        }

        fn progress(&self) -> Rc<dyn OutboundProgressRuntime> {
            Rc::new(IndependentOutboundProgress)
        }

        fn start_batch_until(&self, batch: OutboundPreparedBatch) -> OutboundBatch {
            let wave = self.control.waves.fetch_add(1, Ordering::SeqCst);
            let (context, slots) = batch.into_slots();
            let mut pending_slots = slots
                .into_iter()
                .map(|(index, request)| (index, request.request().method().clone()))
                .collect::<Vec<_>>()
                .into_iter();
            let control = Arc::clone(&self.control);
            let clock = context.clock().clone();
            let body_bytes = self.body_bytes;
            OutboundBatch::from_driver(
                context,
                stream::poll_fn(move |_cx| {
                    if let Some((index, method)) = pending_slots.next() {
                        let ordinal = control.bodies.fetch_add(1, Ordering::SeqCst);
                        // No queued response buffers exist before dispatch. Each
                        // payload is allocated only when its slot is observed.
                        let response = OutboundResponse::new(
                            method,
                            StatusCode::OK,
                            HeaderMap::new(),
                            Body::from_bytes(vec![u8::try_from(ordinal).unwrap(); body_bytes]),
                            clock.clone(),
                            ContentTypeProvenance::Unknown,
                        );
                        return Poll::Ready(Some(OutboundBatchDriverEvent::Item {
                            index,
                            completed_at: clock.now(),
                            outcome: Ok(response),
                        }));
                    }
                    if control.released.load(Ordering::SeqCst) > wave {
                        Poll::Ready(None)
                    } else {
                        Poll::Pending
                    }
                }),
                OutboundExchangeOwner::new(|| {}),
            )
        }

        fn time(&self) -> OutboundTime {
            let read_clock = Arc::clone(&self.clock);
            let waits = Arc::clone(&self.clock);
            OutboundTime::new(
                MonotonicClock::new(move || read_clock.now_monotonic()),
                move |deadline, clock| {
                    let waiter = Arc::clone(&waits);
                    Box::pin(async move {
                        if let Some(remaining) = deadline.remaining_at(clock.now()) {
                            waiter.sleep(remaining).await;
                        }
                        Ok(())
                    })
                },
            )
        }

        fn validate_supported_options(
            &self,
            _: &OutboundRequest,
            _: OutboundSendMode,
        ) -> Result<(), EdgeError> {
            Ok(())
        }
    }

    fn calls(count: usize) -> Vec<BidderCall> {
        (0..count)
            .map(|index| BidderCall {
                request: outbound_request::request(
                    "POST".into(),
                    format!("https://bidder.example/{index}"),
                    vec![],
                    b"{}".to_vec(),
                    vec![],
                )
                .unwrap(),
                timeout: Duration::from_secs(1),
                max_transport_bytes: 256 * 1024,
                max_decoded_bytes: 256 * 1024,
            })
            .collect()
    }

    fn live_since(baseline: i64) -> i64 {
        alloc_track::live().checked_sub(baseline).unwrap()
    }

    fn run_case(width: u32, count: usize, body_bytes: usize, cancel: bool) {
        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let control = Arc::new(Control {
            waves: AtomicUsize::new(0),
            bodies: AtomicUsize::new(0),
            released: AtomicUsize::new(0),
        });
        let provider = LazyBodies {
            control: Arc::clone(&control),
            clock: Arc::clone(&clock),
            body_bytes,
        };
        let gate = StreamGate::new(width);
        let mut limits = envelope::limits();
        limits.max_outbound_requests = u32::try_from(count).unwrap();
        limits.max_active_body_streams = width;
        let client = BoundedFetch::new(HttpClient::with_client(provider), limits);
        let deadline = AuctionDeadline {
            deadline_at: clock
                .now_monotonic()
                .checked_add(Duration::from_secs(2))
                .unwrap(),
            source: TmaxSource::Request,
        };
        let cooperator = NoopDecodeCooperator;
        let host = WaveHost {
            clock: clock.as_ref(),
            timer: clock.as_ref(),
            cooperator: &cooperator,
        };
        let baseline = alloc_track::reset_peak();
        let mut fanout = Box::pin(drive_fanout(
            calls(count),
            &deadline,
            host,
            &client,
            &limits,
            &gate,
        ));
        let mut cx = Context::from_waker(Waker::noop());
        let width_usize = usize::try_from(width).unwrap();
        let waves = count.div_ceil(width_usize);
        for wave in 0..waves {
            assert!(fanout.as_mut().poll(&mut cx).is_pending());
            let wave_end = wave.checked_add(1).unwrap();
            let produced = wave_end.checked_mul(width_usize).unwrap().min(count);
            assert_eq!(control.waves.load(Ordering::SeqCst), wave_end);
            assert_eq!(control.bodies.load(Ordering::SeqCst), produced);
            assert_eq!(
                gate.in_flight(),
                u32::try_from(
                    produced
                        .checked_sub(wave.checked_mul(width_usize).unwrap())
                        .unwrap()
                )
                .unwrap()
            );
            let retained = u64::try_from(live_since(baseline)).unwrap();
            let payload = u64::try_from(produced.checked_mul(body_bytes).unwrap()).unwrap();
            assert!(
                retained >= payload,
                "all earlier response bytes must remain live: {retained} < {payload}"
            );
            assert!(alloc_track::peak_since(baseline) >= payload);
            if cancel && wave == 1 {
                assert!(produced > width_usize);
                drop(fanout);
                assert_eq!(gate.in_flight(), 0);
                assert_eq!(gate.free(), width);
                assert!(
                    live_since(baseline) < i64::try_from(body_bytes).unwrap(),
                    "cancellation must release every retained response body"
                );
                return;
            }
            control.released.store(wave_end, Ordering::SeqCst);
        }
        assert!(!cancel, "every cancellation row must reach its second wave");
        let results = block_on(fanout).unwrap();
        assert_eq!(results.len(), count);
        assert_eq!(gate.in_flight(), 0);
        assert_eq!(gate.free(), width);
        assert!(count > width_usize);
        for (ordinal, result) in results.iter().enumerate() {
            let response = result.result.as_ref().unwrap();
            assert_eq!(response.body_len(), body_bytes);
            assert!(response.body_equals(&vec![u8::try_from(ordinal).unwrap(); body_bytes]));
        }
        assert!(
            live_since(baseline) >= i64::try_from(count.checked_mul(body_bytes).unwrap()).unwrap(),
            "idle stream permits do not release completed response storage"
        );
        drop(results);
        assert!(
            live_since(baseline) < i64::try_from(body_bytes).unwrap(),
            "dropping results must release every retained response body"
        );
    }

    #[test]
    fn completed_response_storage_scales_with_calls_not_stream_width() {
        for (width, count, body_bytes) in [(1, 5, 16_381), (2, 9, 32_749), (3, 10, 0xfff1)] {
            run_case(width, count, body_bytes, false);
            run_case(width, count, body_bytes, true);
        }
    }
}
