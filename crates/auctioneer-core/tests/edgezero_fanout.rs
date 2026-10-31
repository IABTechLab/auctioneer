//! `endpoint_compression: Gzip` has to survive all the way onto the
//! wire, headers included.
//!
//! Exchange finalization precedes fanout. The proxy must receive that
//! finalized body unchanged, with no duplicate encoding or adapter length.

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use super::fanout_limits as envelope;
    use auctioneer_core::test_support::outbound_request;

    use auctioneer_core::host_primitives::{EdgeZeroFanout, RaceTimersStrategy};
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderFanout as _, Compression, TmaxSource,
    };
    use auctioneer_runtime::{Clock, FakeClock, Timer};
    use edgezero_core::error::EdgeError;
    use edgezero_core::http::HeaderMap;
    use edgezero_core::http::StatusCode;
    use edgezero_core::outbound::{
        HttpClient, IndependentOutboundProgress, OutboundBatch, OutboundBatchDriverEvent,
        OutboundCachePolicy, OutboundDispatchRequest, OutboundExchangeOwner, OutboundFuture,
        OutboundHead, OutboundHeaderFidelity, OutboundHeaderInput, OutboundHttpClient,
        OutboundPending, OutboundPreparedBatch, OutboundProgressRuntime, OutboundRequest,
        OutboundSendMode, OutboundTime,
    };
    use edgezero_core::time::MonotonicClock;
    use flate2::read::GzDecoder;
    use futures::StreamExt as _;
    use futures::executor::block_on;
    use futures::stream;
    use std::io::Read as _;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
    use std::time::Duration;
    use time::OffsetDateTime;

    /// The gzip member header. Nothing else the fanout could send —
    /// least of all the plain JSON body — starts with these two bytes.
    const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

    // Raw-string `const`: `serde_json::json!` yields a `Value`, which no
    // `const` can hold. Converting means turning this into a function
    // returning a `String` and re-rendering it at every use.
    const PLAIN_BODY: &[u8] = br#"{"id":"gzip-regression","imp":[{"id":"imp-1"}]}"#;

    /// One outbound request exactly as the proxy received it.
    struct Recorded {
        cache_policy: OutboundCachePolicy,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    }

    /// Records every dispatch and answers 200. Cloneable over shared
    /// state because `HttpClient::with_client` takes the client by
    /// value and the test still has to read the recording back.
    #[derive(Clone)]
    struct MockProxyClient {
        recorded: Arc<Mutex<Vec<Recorded>>>,
        /// The fanout's own clock. A canned response's deferred body
        /// deadlines must read the source the fanout and its timer run
        /// on, not process-default real time.
        clock: Arc<FakeClock>,
    }

    impl MockProxyClient {
        fn new(clock: &Arc<FakeClock>) -> Self {
            Self {
                recorded: Arc::new(Mutex::new(Vec::new())),
                clock: Arc::clone(clock),
            }
        }

        fn locked(&self) -> MutexGuard<'_, Vec<Recorded>> {
            self.recorded.lock().unwrap_or_else(PoisonError::into_inner)
        }
    }

    impl OutboundHttpClient for MockProxyClient {
        fn begin(&self, request: OutboundDispatchRequest) -> Result<OutboundPending, EdgeError> {
            let pending_head = self.dispatch_head(request);
            Ok(OutboundPending::new(
                Box::pin(async move { pending_head.await?.finish().await }),
                OutboundExchangeOwner::new(|| {}),
            ))
        }

        fn dispatch_head(&self, request: OutboundDispatchRequest) -> OutboundFuture<OutboundHead> {
            let (parts, context) = request.into_parts();
            let (cache_policy, headers, body) = (parts.cache_policy, parts.headers, parts.body);
            self.locked().push(Recorded {
                cache_policy,
                headers: headers
                    .iter()
                    .map(|(name, value)| {
                        (
                            name.as_str().to_owned(),
                            value.to_str().unwrap_or_default().to_owned(),
                        )
                    })
                    .collect(),
                body: body
                    .into_bytes()
                    .map(|buffered| buffered.to_vec())
                    .unwrap_or_default(),
            });
            let head = OutboundHead::new(
                StatusCode::OK,
                OutboundHeaderInput::new(
                    "fixture",
                    HeaderMap::new(),
                    OutboundHeaderFidelity::ProviderFieldBoundariesUnknown,
                ),
                Box::pin(stream::iter([Ok(bytes::Bytes::from_static(b"{}"))])),
                context,
                OutboundExchangeOwner::new(|| {}),
            );
            Box::pin(async move { head })
        }

        fn progress(&self) -> Rc<dyn OutboundProgressRuntime> {
            Rc::new(IndependentOutboundProgress)
        }

        /// Completion order: input order. Dispatch records every request
        /// eagerly, while body finalization may cooperatively yield.
        /// Cutoff: never emitted; every canned answer is a 200.
        /// Cancellation / drop: dropping the stream drops pending finalizers.
        fn start_batch_until(&self, batch: OutboundPreparedBatch) -> OutboundBatch {
            let (context, slots) = batch.into_slots();
            let completions = slots
                .into_iter()
                .map(|(index, request)| {
                    let pending_head = self.dispatch_head(request);
                    let clock = context.clock().clone();
                    async move {
                        let outcome = match pending_head.await {
                            Ok(resolved_head) => resolved_head.finish().await,
                            Err(error) => Err(error),
                        };
                        OutboundBatchDriverEvent::Item {
                            index,
                            completed_at: clock.now(),
                            outcome,
                        }
                    }
                })
                .collect::<Vec<_>>();
            OutboundBatch::from_driver(
                context,
                stream::iter(completions).then(|completion| completion),
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

    fn header_values<'a>(recorded: &'a Recorded, name: &str) -> Vec<&'a str> {
        recorded
            .headers
            .iter()
            .filter(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
            .collect()
    }

    #[test]
    fn gzip_endpoint_compression_reaches_the_proxy_with_matching_headers() {
        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let client = MockProxyClient::new(&clock);
        let as_clock: Arc<dyn Clock + Send + Sync> = Arc::<FakeClock>::clone(&clock);
        let as_timer: Arc<dyn Timer + Send + Sync> = Arc::<FakeClock>::clone(&clock);
        let fanout = EdgeZeroFanout::with_wave_scheduler(
            HttpClient::with_client(client.clone()),
            as_clock,
            as_timer,
            Arc::new(auctioneer_runtime::NoopDecodeCooperator),
            Arc::new(RaceTimersStrategy),
            envelope::limits(),
            auctioneer_runtime::StreamGate::new(2),
        );
        let raw_request = outbound_request::request(
            "POST".into(),
            "https://bidder.example/rtb".into(),
            vec![],
            PLAIN_BODY.to_vec(),
            vec!["imp-1".into()],
        )
        .unwrap();
        let (_, request) = auctioneer_bidder::finalize_for_transport(
            raw_request,
            &auctioneer_bidder::TransportFinalize {
                gpc: None,
                compression: Some(Compression::Gzip),
                max_encoded_bytes: 1 << 18,
            },
        );
        let call = BidderCall {
            request: request.unwrap(),
            timeout: Duration::from_millis(500),
            max_transport_bytes: 1 << 20,
            max_decoded_bytes: 1 << 20,
        };
        let deadline = AuctionDeadline {
            deadline_at: clock
                .origin()
                .checked_add(Duration::from_secs(1))
                .expect("no overflow"),
            source: TmaxSource::Request,
        };

        let results = block_on(fanout.send_all(vec![call], deadline)).expect("send_all succeeds");
        assert_eq!(results.len(), 1, "one call in, one result out");
        assert!(
            results.first().is_some_and(|row| row.result.is_ok()),
            "the mock answers 200, so the call succeeds: {:?}",
            results.first().map(|row| &row.result),
        );

        let recorded = client.locked();
        assert_eq!(recorded.len(), 1, "exactly one outbound dispatch");
        let sent = recorded.first().expect("the single recorded dispatch");

        assert_eq!(
            sent.body.get(..2),
            Some(GZIP_MAGIC.as_slice()),
            "the wire body must be a gzip member, not the plain JSON",
        );
        let mut decoded = Vec::new();
        GzDecoder::new(sent.body.as_slice())
            .read_to_end(&mut decoded)
            .expect("the wire body is a valid gzip stream");
        assert_eq!(
            decoded, PLAIN_BODY,
            "compression must preserve the payload byte for byte",
        );

        assert_eq!(
            header_values(sent, "content-encoding"),
            vec!["gzip"],
            "exactly one Content-Encoding, naming gzip",
        );
        assert_eq!(
            header_values(sent, "content-length"),
            Vec::<&str>::new(),
            "only the transport may choose framing headers",
        );
    }

    /// Every bidder call must reach the wire with caching bypassed.
    ///
    /// `OutboundCachePolicy::PlatformDefault` is what an unset policy
    /// inherits, so this asserts the proxy adapter sets the value
    /// rather than leaving a cacheable bidder call to the platform.
    #[test]
    fn proxy_adapter_sets_cache_bypass() {
        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let client = MockProxyClient::new(&clock);
        let as_clock: Arc<dyn Clock + Send + Sync> = Arc::<FakeClock>::clone(&clock);
        let as_timer: Arc<dyn Timer + Send + Sync> = Arc::<FakeClock>::clone(&clock);
        let fanout = EdgeZeroFanout::with_wave_scheduler(
            HttpClient::with_client(client.clone()),
            as_clock,
            as_timer,
            Arc::new(auctioneer_runtime::NoopDecodeCooperator),
            Arc::new(RaceTimersStrategy),
            envelope::limits(),
            auctioneer_runtime::StreamGate::new(2),
        );
        let call = BidderCall {
            request: outbound_request::request(
                "POST".into(),
                "https://bidder.example/rtb".into(),
                Vec::new(),
                PLAIN_BODY.to_vec(),
                vec!["imp-1".to_owned()],
            )
            .expect("sink-built fixture"),
            timeout: Duration::from_millis(500),
            max_transport_bytes: 1 << 20,
            max_decoded_bytes: 1 << 20,
        };
        let deadline = AuctionDeadline {
            deadline_at: clock
                .origin()
                .checked_add(Duration::from_secs(1))
                .expect("no overflow"),
            source: TmaxSource::Request,
        };

        let results = block_on(fanout.send_all(vec![call], deadline)).expect("send_all succeeds");
        assert_eq!(results.len(), 1, "one call in, one result out");

        let recorded = client.locked();
        let sent = recorded.first().expect("the single recorded dispatch");
        assert!(
            matches!(sent.cache_policy, OutboundCachePolicy::Bypass),
            "a bidder call must never be cacheable, got {:?}",
            sent.cache_policy,
        );
    }
}
