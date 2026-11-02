//! The Axum entrypoint installs whatever `axum_production_strategy`
//! returns, so this asserts the factory hands back the RACING strategy.
//!
//! It is a behavioural assertion, not a type check: only a strategy
//! that owns a timer can synthesize a `TimeoutCause` at all, so a
//! factory that quietly reverted to the delegating path would hang on
//! this fixture instead of reporting a per-bidder timeout.

#[cfg(test)]
mod tests {
    use auctioneer_core::test_support::outbound_request;

    use auctioneer_core::host_primitives::{EdgeZeroFanout, axum_production_strategy};
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout as _, FanoutError,
        FanoutInternalError, TimeoutCause, TmaxSource,
    };
    use auctioneer_runtime::{Clock, FakeClock, Timer};
    use edgezero_core::error::EdgeError;
    use edgezero_core::outbound::{
        HttpClient, IndependentOutboundProgress, OutboundBatch, OutboundBatchDriverEvent,
        OutboundDispatchRequest, OutboundExchangeOwner, OutboundFuture, OutboundHead,
        OutboundHttpClient, OutboundPending, OutboundPreparedBatch, OutboundProgressRuntime,
        OutboundRequest, OutboundSendMode, OutboundTime,
    };
    use edgezero_core::time::MonotonicClock;
    use futures::stream;
    use std::future::{Future, pending};
    use std::pin::Pin;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::task::{Context, Poll, Waker};
    use std::time::Duration;
    use time::OffsetDateTime;

    /// Never answers, so the only way this call can resolve is a timer
    /// arm — which the delegating strategy does not have.
    struct NeverResponds(Arc<FakeClock>);

    impl OutboundHttpClient for NeverResponds {
        fn begin(&self, _: OutboundDispatchRequest) -> Result<OutboundPending, EdgeError> {
            Ok(OutboundPending::new(
                Box::pin(pending()),
                OutboundExchangeOwner::new(|| {}),
            ))
        }

        fn dispatch_head(&self, _: OutboundDispatchRequest) -> OutboundFuture<OutboundHead> {
            Box::pin(pending())
        }

        fn progress(&self) -> Rc<dyn OutboundProgressRuntime> {
            Rc::new(IndependentOutboundProgress)
        }

        fn start_batch_until(&self, batch: OutboundPreparedBatch) -> OutboundBatch {
            Self::never_resolving_batch(batch)
        }

        fn time(&self) -> OutboundTime {
            let read_clock = Arc::clone(&self.0);
            let waits = Arc::clone(&self.0);
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

    impl NeverResponds {
        /// A batch that never resolves a slot and never reports a
        /// termination of its own.
        ///
        /// Completion order: no slot ever completes, so none is emitted.
        /// Cutoff: the driver arms no timer, so it emits no `Cutoff`
        /// either — expiry stays where this fixture has always driven
        /// it, on the strategy's own `FakeClock` timer. That is the
        /// point of the test: only a strategy that owns a timer can
        /// name a per-bidder timeout, and a batch that reported
        /// `Finished(Completed)` with zero items would let the
        /// delegating path pass for the wrong reason.
        /// Delay modelling: none — the single behaviour is "pending".
        /// Cancellation / drop: dropping the batch drops the pending
        /// stream; nothing was issued, so nothing is left behind.
        fn never_resolving_batch(batch: OutboundPreparedBatch) -> OutboundBatch {
            let (context, slots) = batch.into_slots();
            drop(slots);
            OutboundBatch::from_driver(
                context,
                stream::poll_fn(|_cx| Poll::<Option<OutboundBatchDriverEvent>>::Pending),
                OutboundExchangeOwner::new(|| {}),
            )
        }
    }

    /// The `FakeClock` fires a pending sleep only when a test advances
    /// it, so no executor would repoll on its own.
    fn poll_once<F>(driving: &mut Pin<Box<F>>) -> Poll<F::Output>
    where
        F: Future + ?Sized,
    {
        driving
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
    }

    fn ready_rows(
        outcome: Poll<Result<Vec<BidderCallResult>, FanoutInternalError>>,
    ) -> Vec<BidderCallResult> {
        match outcome {
            Poll::Ready(Ok(rows)) => rows,
            Poll::Ready(Err(err)) => panic!("send_all itself must succeed, got {err:?}"),
            Poll::Pending => panic!("the elapsed per-bidder budget must resolve the race"),
        }
    }

    #[test]
    fn axum_production_strategy_installs_the_racing_path() {
        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let as_clock: Arc<dyn Clock + Send + Sync> = Arc::<FakeClock>::clone(&clock);
        let as_timer: Arc<dyn Timer + Send + Sync> = Arc::<FakeClock>::clone(&clock);
        let fanout = EdgeZeroFanout::new(
            HttpClient::with_client(NeverResponds(Arc::clone(&clock))),
            as_clock,
            as_timer,
            axum_production_strategy(),
        );
        let call = BidderCall {
            request: outbound_request::request(
                "POST".to_owned(),
                "https://bidder.example/rtb".to_owned(),
                Vec::new(),
                br#"{"id":"strategy-flip"}"#.to_vec(),
                vec!["imp-1".to_owned()],
            )
            .expect("sink-built fixture"),
            // Comfortably inside the auction budget below, so the arm
            // that wins names the BIDDER and not the auction.
            timeout: Duration::from_millis(50),
            max_transport_bytes: 1 << 20,
            max_decoded_bytes: 1 << 20,
        };
        let deadline = AuctionDeadline {
            deadline_at: clock
                .origin()
                .checked_add(Duration::from_millis(500))
                .expect("no overflow"),
            source: TmaxSource::Request,
        };

        let mut driving = Box::pin(fanout.send_all(vec![call], deadline));
        assert!(
            poll_once(&mut driving).is_pending(),
            "neither budget has elapsed at t=0 and the upstream never answers",
        );
        clock.advance(Duration::from_millis(50));
        let results = ready_rows(poll_once(&mut driving));

        assert_eq!(results.len(), 1, "one call in, one result out");
        assert!(
            matches!(
                results.first().map(|row| &row.result),
                Some(Err(FanoutError::Timeout(TimeoutCause::Bidder))),
            ),
            "only the racing strategy can name a per-bidder timeout, got {:?}",
            results.first().map(|row| &row.result),
        );
    }
}
