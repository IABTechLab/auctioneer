mod support;

#[cfg(test)]
pub mod config_funding_support;
use config_funding_support as config_funding;

#[cfg(test)]
use auctioneer_bidder::test_support::serving_catalog;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use crate::config_funding;
    use crate::serving_catalog::passthrough_info;
    mod envelope {
        pub use crate::fanout_limits::limits;
        pub fn target() -> auctioneer_runtime::TargetPlatformConstants {
            auctioneer_runtime::TargetPlatformConstants {
                platform_concurrent_ceiling: 384,
                header_entry_overhead_bytes: 64,
                parser_header_ceiling_bytes: 21_632,
                parser_header_knob_bytes: 21_633,
                max_request_target_bytes: 768,
                h1_max_buf_size_bytes: 24_576_u32,
                h1_max_headers: 128,
                parser_max_header_entries: 676,
                pseudoheader_allowance_bytes: 1_024,
                parser_transient_bytes: 0x0000_4000,
                egress_handoff_factor: 2,
            }
        }
    }
    use crate::support::ctx_with_config;
    use async_trait::async_trait;
    use auctioneer_core::auction::audit::construction_costs as core_construction_costs;
    use auctioneer_core::handlers::{TestFanoutHandle, auction};
    use auctioneer_core::host_primitives::{
        BootEnvelope, CredentialLoader, RaceTimersStrategy, StderrBootLog, TestClockHandle,
        TestMonotonicClockHandle, TestRngHandle, bootstrap_adapter_with_envelope, stream_gate,
    };
    use auctioneer_core::test_support::config_store::auction_context_for_route;
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
    };
    use auctioneer_runtime::bidder_credentials::{BidderCredentialSource, CredentialUnavailable};
    use auctioneer_runtime::{
        Clock as _, FakeClock, MonotonicInstant, NoopDecodeCooperator, SeededRng, Timer as _,
    };
    use bytes::Bytes;
    use edgezero_core::body::Body;
    use edgezero_core::config_store::BoundedStoreRead;
    use edgezero_core::error::EdgeError;
    use edgezero_core::http::{HeaderMap, Method, StatusCode};
    use edgezero_core::outbound::{
        HttpClient, IndependentOutboundProgress, OutboundBatch, OutboundBatchDriverEvent,
        OutboundDispatchRequest, OutboundExchangeOwner, OutboundFuture, OutboundHead,
        OutboundHttpClient, OutboundPending, OutboundPreparedBatch, OutboundProgressRuntime,
        OutboundRequest, OutboundResponse, OutboundSendMode, OutboundTime,
    };
    use edgezero_core::secret_store::{SecretError, SecretHandle, SecretStore};
    use edgezero_core::store_registry::BoundSecretStore;
    use edgezero_core::time::{Deadline, MonotonicClock};
    use futures::{executor::block_on, stream};
    use http::Request;
    use serde_json::json;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use time::OffsetDateTime;

    #[derive(Clone)]
    struct BatchOnly {
        clock: Arc<FakeClock>,
        batches: Arc<Mutex<Vec<usize>>>,
    }

    impl OutboundHttpClient for BatchOnly {
        fn begin(&self, _: OutboundDispatchRequest) -> Result<OutboundPending, EdgeError> {
            Err(EdgeError::not_implemented(
                "handler must use bounded batch dispatch, never legacy send",
            ))
        }

        fn dispatch_head(&self, _: OutboundDispatchRequest) -> OutboundFuture<OutboundHead> {
            panic!("handler must use bounded batch dispatch, never legacy send");
        }

        fn progress(&self) -> Rc<dyn OutboundProgressRuntime> {
            Rc::new(IndependentOutboundProgress)
        }

        fn start_batch_until(&self, batch: OutboundPreparedBatch) -> OutboundBatch {
            let (context, slots) = batch.into_slots();
            self.batches.lock().unwrap().push(context.slot_count());
            let events = slots
                .into_iter()
                .map(|(index, request)| {
                    let response = OutboundResponse::new(
                        request.request().method().clone(),
                        StatusCode::NO_CONTENT,
                        HeaderMap::new(),
                        Body::empty(),
                        request.clock().clone(),
                        edgezero_core::ContentTypeProvenance::Unknown,
                    );
                    OutboundBatchDriverEvent::Item {
                        index,
                        completed_at: context.clock().now(),
                        outcome: Ok(response),
                    }
                })
                .collect::<Vec<_>>();
            OutboundBatch::from_driver(
                context,
                stream::iter(events),
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

    struct EmptyFanout;
    #[async_trait(?Send)]
    impl BidderFanout for EmptyFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            Ok(calls
                .into_iter()
                .map(|_| BidderCallResult {
                    result: Ok(auctioneer_bidder::BidderHttpResponse::new(
                        204,
                        vec![],
                        vec![],
                    )),
                    elapsed: Duration::ZERO,
                })
                .collect())
        }
    }

    const CONFIG: &str = "schema_version = 1\n[host.gdpr]\nenabled = false\n[host.resource_limits]\nmax_bidders_per_auction = 50\nmax_outbound_requests = 100\n";

    fn request(client: &BatchOnly, test_fanout: bool) -> Request<Body> {
        let base = &passthrough_info().name;
        let body = json!({
            "id": "waves", "tmax": 1000_i64, "site": {"publisher": {"id": "test"}},
            "imp": [{"id": "i1", "banner": {"w": 300_i32, "h": 250_i32},
                "ext": {"bidder": {"fake_a": {}, "fake_b": {}, "fake_c": {}}}}],
            "ext": {"prebid": {"aliases": {"fake_a": base, "fake_b": base, "fake_c": base}}}
        });
        let mut request = Request::builder()
            .method(Method::POST)
            .uri("/openrtb2/auction")
            .body(Body::from_bytes(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        request
            .extensions_mut()
            .insert(HttpClient::with_client(client.clone()));
        request
            .extensions_mut()
            .insert(TestClockHandle(Arc::<FakeClock>::clone(&client.clock)));
        request
            .extensions_mut()
            .insert(TestRngHandle(Arc::new(SeededRng::new(42))));
        let clock = Arc::clone(&client.clock);
        request
            .extensions_mut()
            .insert(TestMonotonicClockHandle(MonotonicClock::new(move || {
                clock.now_monotonic()
            })));
        if test_fanout {
            request
                .extensions_mut()
                .insert(TestFanoutHandle(Arc::new(EmptyFanout)));
        }
        request
    }

    #[test]
    fn public_credential_loader_preserves_bound_sdk_store_clock_and_deadline() {
        struct FakeCredentialStore {
            clock: Arc<FakeClock>,
            deadline: MonotonicInstant,
            reads: AtomicUsize,
        }

        #[async_trait(?Send)]
        impl SecretStore for FakeCredentialStore {
            async fn get_bytes(&self, _: &str, _: &str) -> Result<Option<Bytes>, SecretError> {
                panic!("credential reads must use the bounded SDK operation");
            }

            async fn get_bytes_bounded(
                &self,
                store_name: &str,
                key: &str,
                clock: &MonotonicClock,
                deadline: Deadline,
                max_backend_bytes: u64,
                max_value_bytes: u64,
            ) -> Result<BoundedStoreRead<Bytes>, SecretError> {
                assert_eq!(store_name, "fake_host_secret_store");
                assert_eq!(key, "fake_bidder_credentials");
                assert_eq!((max_backend_bytes, max_value_bytes), (4096, 4096));
                assert_eq!(clock.now(), self.clock.now_monotonic());
                assert_eq!(deadline.instant(), self.deadline);
                assert_eq!(
                    deadline.remaining_at(clock.now()),
                    Some(Duration::from_secs(3))
                );
                assert_eq!(self.reads.fetch_add(1, Ordering::SeqCst), 0);
                let value = Bytes::from(
                    serde_json::to_vec(&json!({
                        "schema_version": 1_u8,
                        "username": "fake_user",
                        "password": "fake_password",
                        "tracker": "fake_tracker"
                    }))
                    .unwrap(),
                );
                Ok(BoundedStoreRead {
                    backend_bytes: u64::try_from(value.len()).unwrap(),
                    value: Some(value),
                })
            }
        }

        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let clock_source = Arc::clone(&clock);
        let monotonic = MonotonicClock::new(move || clock_source.now_monotonic());
        let deadline = monotonic
            .now()
            .checked_add(Duration::from_secs(10))
            .unwrap();
        let provider = Arc::new(FakeCredentialStore {
            clock: Arc::clone(&clock),
            deadline,
            reads: AtomicUsize::new(0),
        });
        let shared: Arc<dyn SecretStore> = Arc::<FakeCredentialStore>::clone(&provider);
        let loader = CredentialLoader::new(
            Some(BoundSecretStore::new(
                SecretHandle::new(shared),
                "fake_host_secret_store".to_owned(),
            )),
            monotonic.clone(),
        );
        assert_eq!(provider.reads.load(Ordering::SeqCst), 0);
        let source: &dyn BidderCredentialSource = &loader;
        let future = source.load("fake_bidder_credentials", deadline);
        assert_eq!(provider.reads.load(Ordering::SeqCst), 0);

        clock.advance(Duration::from_secs(7));
        let credentials = block_on(future).unwrap();
        assert_eq!(provider.reads.load(Ordering::SeqCst), 1);
        assert_eq!(credentials.username(), "fake_user");
        assert_eq!(credentials.tracker(), "fake_tracker");

        clock.advance(Duration::from_secs(3));
        assert_eq!(monotonic.now(), deadline);
        assert!(matches!(
            block_on(source.load("fake_bidder_credentials", deadline)),
            Err(CredentialUnavailable)
        ));
        assert_eq!(provider.reads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn handler_http_client_requires_boot_then_uses_bounded_wave_and_preserves_test_precedence() {
        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let client = BatchOnly {
            clock: Arc::clone(&clock),
            batches: Arc::default(),
        };
        // No primitive factory may be consulted when boot is missing.
        let missing_boot_response =
            block_on(auction(ctx_with_config(CONFIG, request(&client, false)))).unwrap();
        assert_eq!(
            missing_boot_response.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(missing_boot_response.headers()["cache-control"], "no-store");
        assert_eq!(
            missing_boot_response
                .into_body()
                .into_bytes()
                .unwrap()
                .as_ref(),
            b"overloaded\n"
        );
        assert!(client.batches.lock().unwrap().is_empty());

        let mut limits = envelope::limits();
        config_funding::fund_sdk_defaults(&mut limits, &envelope::target());
        bootstrap_adapter_with_envelope(
            BootEnvelope {
                log_sink: &StderrBootLog,
                catalog_costs: auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
                limits: &limits,
                target: &envelope::target(),
                outbound_meta_bytes: auctioneer_bidder::outbound_meta_bytes(100),
                construction_costs: core_construction_costs(),
            },
            || Arc::<FakeClock>::clone(&clock),
            || Arc::new(SeededRng::new(42)),
            || Arc::<FakeClock>::clone(&clock),
            || Arc::new(NoopDecodeCooperator),
            || Arc::new(RaceTimersStrategy),
        )
        .unwrap();
        let gate = stream_gate().unwrap();
        let before = gate.in_flight();
        let monotonic_source = Arc::clone(&clock);
        let monotonic = MonotonicClock::new(move || monotonic_source.now_monotonic());
        let (ctx, mut terminal) = auction_context_for_route(
            ctx_with_config(CONFIG, request(&client, false))
                .into_request()
                .unwrap(),
            "/openrtb2/auction",
            monotonic.clone(),
        );
        *terminal.response_mut() = block_on(auction(ctx)).unwrap();
        assert_eq!(terminal.response_mut().status(), StatusCode::OK);
        assert_eq!(
            *client.batches.lock().unwrap(),
            [2],
            "boot clamps bidder population before construction"
        );
        assert_eq!(gate.in_flight(), before, "terminal slots release permits");
        drop(terminal);
        client.batches.lock().unwrap().clear();
        let (test_context, mut test_terminal) = auction_context_for_route(
            ctx_with_config(CONFIG, request(&client, true))
                .into_request()
                .unwrap(),
            "/openrtb2/auction",
            monotonic,
        );
        *test_terminal.response_mut() = block_on(auction(test_context)).unwrap();
        assert_eq!(test_terminal.response_mut().status(), StatusCode::OK);
        assert!(
            client.batches.lock().unwrap().is_empty(),
            "test fanout still wins over HttpClient"
        );
        drop(test_terminal);
    }
}
