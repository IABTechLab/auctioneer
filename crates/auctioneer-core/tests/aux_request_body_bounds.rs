//! Matched non-body routes enforce the inbound cap through the generated App.

#[cfg(test)]
mod config_funding_support;
use config_funding_support as config_funding;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use crate::config_funding;
    use async_trait::async_trait;
    use auctioneer_config::policy::HostPolicy;
    use auctioneer_core::config::AuctioneerAppConfig;
    use auctioneer_core::handlers::egress_deadline::REFUSAL_WRITE_BUDGET;
    use auctioneer_core::handlers::probe_config::{
        __probe_isolation_for_tests, config_resolutions,
    };
    use auctioneer_core::host_primitives::error_renderer::REQUEST_TIMEOUT_BODY;
    use auctioneer_core::host_primitives::ingress_lease::{
        RouteAdmissionMarker, RouteEgressPermit, admit,
    };
    use auctioneer_core::host_primitives::probe_egress_gate;
    use auctioneer_core::ingress::decide;
    use auctioneer_runtime::AuxGate;
    use auctioneer_runtime::body_caps::DEFAULT_CAP_BYTES;
    use edgezero_core::app::App;
    use edgezero_core::blob_envelope::BlobEnvelope;
    use edgezero_core::body::Body;
    use edgezero_core::config_store::{
        BoundedStoreRead, ConfigStore, ConfigStoreError, ConfigStoreHandle, ConfigValue,
    };
    use edgezero_core::error::EdgeError;
    use edgezero_core::http::{Method, Request, StatusCode, request_builder};
    use edgezero_core::ingress::{
        AdmissionDecision, DEFAULT_INBOUND_READ_BUDGET, IngressDispatchOutcome, IngressFraming,
        IngressGrant, IngressHeadAccounting,
    };
    use edgezero_core::response_egress::{ResponseEgressDeadline, ResponseEgressEnvelope};
    use edgezero_core::response_egress_framing::ResponseEgressFramingMode;
    use edgezero_core::store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry};
    use edgezero_core::time::{Deadline, MonotonicClock, MonotonicInstant};
    use futures::channel::oneshot;
    use futures::executor::block_on;
    use futures::stream;
    use futures::task::noop_waker;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll};

    struct CountedStore {
        reads: AtomicUsize,
        blob: ConfigValue,
    }

    #[async_trait(?Send)]
    impl ConfigStore for CountedStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            panic!("real config extraction must use the bounded store API");
        }

        async fn get_bounded(
            &self,
            _key: &str,
            _clock: &MonotonicClock,
            _deadline: Deadline,
            _max_backend_bytes: u64,
            _max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
            self.reads.fetch_add(1, Ordering::Relaxed);
            Ok(BoundedStoreRead {
                backend_bytes: u64::try_from(self.blob.len()).unwrap(),
                value: Some(self.blob.clone()),
            })
        }
    }

    struct Fixture {
        app: App,
        aux: Arc<AuxGate>,
        now: Arc<Mutex<MonotonicInstant>>,
        store: Arc<CountedStore>,
    }

    impl Fixture {
        fn new() -> Self {
            config_funding::install();
            let mut app =
                App::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default())
                    .expect("the generated App builds with auctioneer's error renderer");
            let now = Arc::new(Mutex::new(MonotonicInstant::now()));
            let source = Arc::clone(&now);
            app.set_monotonic_clock(MonotonicClock::new(move || *source.lock().unwrap()));
            let aux = AuxGate::new(1);
            let gate = Arc::clone(&aux);
            // Probes use the production policy; local aux capacity isolates
            // matched body collection from the process-wide aux gate.
            app.set_ingress_admission_policy(move |head| {
                if head.target().path() == "/status" {
                    return decide(head);
                }
                let permit = gate.try_acquire().expect("one aux response at a time");
                let (lease, completion) =
                    admit(RouteAdmissionMarker::Aux, RouteEgressPermit::Aux(permit));
                AdmissionDecision::Admit {
                    completion,
                    grant: IngressGrant::new(lease),
                    read_deadline: head.read_deadline_after(DEFAULT_INBOUND_READ_BUDGET),
                }
            });
            let envelope = BlobEnvelope::new(
                serde_json::to_value(AuctioneerAppConfig {
                    host: HostPolicy {
                        status_response: Some("ok".to_owned()),
                        ..HostPolicy::default()
                    },
                    ..AuctioneerAppConfig::default()
                })
                .unwrap(),
                "1970-01-01T00:00:00Z".to_owned(),
            );
            Self {
                app,
                aux,
                now,
                store: Arc::new(CountedStore {
                    reads: AtomicUsize::new(0),
                    blob: serde_json::to_string(&envelope).unwrap().into(),
                }),
            }
        }

        fn now(&self) -> MonotonicInstant {
            *self.now.lock().unwrap()
        }

        fn request(&self, path: &str, body: Body, framing: IngressFraming) -> Request {
            let mut builder = request_builder().method(Method::GET).uri(path);
            if let IngressFraming::ContentLength(length) = framing {
                builder = builder.header("content-length", length);
            }
            if matches!(framing, IngressFraming::Chunked) {
                builder = builder.header("transfer-encoding", "chunked");
            }
            let mut request = builder.body(body).expect("the fixture GET request builds");
            let store = Arc::clone(&self.store);
            let registry: ConfigRegistry = StoreRegistry::single_id(
                "auctioneer_runtime".to_owned(),
                ConfigStoreBinding {
                    default_key: "auctioneer_runtime".to_owned(),
                    handle: ConfigStoreHandle::new(store),
                },
            );
            request.extensions_mut().insert(registry);
            request
        }

        async fn dispatch(
            &self,
            request: Request,
            start: MonotonicInstant,
            framing: IngressFraming,
        ) -> ResponseEgressEnvelope {
            let outcome = self
                .app
                .dispatch_ingress(request, start, IngressHeadAccounting::HostManaged, framing)
                .await
                .expect("admission succeeds");
            let IngressDispatchOutcome::Response(envelope) = outcome else {
                panic!("the request must produce an admitted response");
            };
            *envelope
        }

        fn reads(&self) -> usize {
            self.store.reads.load(Ordering::Relaxed)
        }
    }

    fn bodies(size: usize) -> Vec<(Body, IngressFraming)> {
        let length = u64::try_from(size).unwrap();
        let mut cases = vec![(
            Body::from_bytes(vec![b'x'; size]),
            IngressFraming::ContentLength(length),
        )];
        for framing in [
            IngressFraming::ContentLength(length),
            IngressFraming::Chunked,
            IngressFraming::HostManaged,
        ] {
            let chunks = [
                2_048,
                size.checked_sub(2_048).expect("fixture spans two chunks"),
            ]
            .map(|len| Ok(Body::from_bytes(vec![b'x'; len]).into_bytes().unwrap()));
            cases.push((Body::from_stream(stream::iter(chunks)), framing));
        }
        cases
    }

    fn assert_body_error(envelope: &mut ResponseEgressEnvelope, status: StatusCode, body: &[u8]) {
        let response = envelope.response_mut();
        assert_eq!(response.status(), status);
        assert_eq!(response.body().as_bytes(), Some(body));
        assert_eq!(
            response.headers()["content-type"],
            "text/plain; charset=utf-8"
        );
        assert!(
            !response
                .headers()
                .contains_key("x-auctioneer-config-status")
        );
        assert_eq!(
            format!(
                "{:?}",
                response
                    .extensions()
                    .get::<ResponseEgressDeadline>()
                    .expect("the canonical renderer bounds every error write")
            ),
            format!("{:?}", ResponseEgressDeadline::after(REFUSAL_WRITE_BUDGET)),
        );
    }

    #[test]
    fn generated_app_status_rejects_a_declared_4097_byte_body() {
        let _isolation = __probe_isolation_for_tests();
        let fixture = Fixture::new();
        let framing = IngressFraming::ContentLength(4_097);
        let request = fixture.request(
            "/status",
            Body::from_bytes(vec![b'x'; DEFAULT_CAP_BYTES + 1]),
            framing,
        );
        let mut envelope = block_on(fixture.dispatch(request, fixture.now(), framing));
        assert_body_error(
            &mut envelope,
            StatusCode::PAYLOAD_TOO_LARGE,
            b"payload_too_large",
        );
        assert_eq!(fixture.reads(), 0);
    }

    #[test]
    fn exact_4096_bytes_reach_every_handler_for_declared_chunked_and_lengthless_bodies() {
        let _isolation = __probe_isolation_for_tests();
        let fixture = Fixture::new();
        let bidder = auctioneer_config_data::bidders()
            .first()
            .expect("the detail route requires a baked bidder");
        let detail_path = format!("/info/bidders/{}", bidder.name);
        for (path, status, expected_reads) in [
            ("/status", StatusCode::OK, 1),
            ("/info/bidders", StatusCode::OK, 1),
            (detail_path.as_str(), StatusCode::OK, 1),
            ("/_auctioneer/health/gvl", StatusCode::UNAUTHORIZED, 0),
        ] {
            for (body, framing) in bodies(DEFAULT_CAP_BYTES) {
                let reads = fixture.reads();
                let request = fixture.request(path, body, framing);
                let mut envelope = block_on(fixture.dispatch(request, fixture.now(), framing));
                assert_eq!(
                    envelope.response_mut().status(),
                    status,
                    "{path}: {framing:?}"
                );
                assert_eq!(
                    fixture.reads(),
                    reads + expected_reads,
                    "the exact cap reaches handler work"
                );
                assert!(
                    envelope
                        .response_mut()
                        .extensions()
                        .get::<ResponseEgressDeadline>()
                        .is_some()
                );
            }
        }
    }

    #[test]
    fn every_matched_non_body_route_rejects_4097_bytes_before_config_work() {
        let _isolation = __probe_isolation_for_tests();
        let fixture = Fixture::new();
        let resolutions = config_resolutions();
        for path in [
            "/status",
            "/info/bidders",
            "/info/bidders/fake_fixture_bidder",
            "/_auctioneer/health/gvl",
        ] {
            for (body, framing) in bodies(DEFAULT_CAP_BYTES + 1) {
                let request = fixture.request(path, body, framing);
                let mut envelope = block_on(fixture.dispatch(request, fixture.now(), framing));
                assert_body_error(
                    &mut envelope,
                    StatusCode::PAYLOAD_TOO_LARGE,
                    b"payload_too_large",
                );
                assert_eq!(fixture.reads(), 0, "{path} must not load config");
                assert_eq!(
                    config_resolutions(),
                    resolutions,
                    "no probe config work starts"
                );
                drop(envelope);
                assert_eq!(fixture.aux.in_use(), 0);
                assert_eq!(probe_egress_gate().in_use(), 0);
            }
        }
    }

    #[test]
    fn an_already_expired_read_returns_408_without_polling_the_body() {
        let _isolation = __probe_isolation_for_tests();
        let fixture = Fixture::new();
        let polls = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&polls);
        let body = Body::from_stream(stream::poll_fn(move |_cx| {
            counted.fetch_add(1, Ordering::Relaxed);
            Poll::Pending
        }));
        let request = fixture.request("/status", body, IngressFraming::Chunked);
        let start = fixture
            .now()
            .checked_sub(DEFAULT_INBOUND_READ_BUDGET)
            .unwrap();
        let mut envelope = block_on(fixture.dispatch(request, start, IngressFraming::Chunked));
        assert_body_error(
            &mut envelope,
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT_BODY.as_bytes(),
        );
        assert_eq!(polls.load(Ordering::Relaxed), 0);
        assert_eq!(fixture.reads(), 0);
    }

    struct DropCount(Arc<AtomicUsize>);

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for DropCount {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn adapter_timeout_body() -> (Body, oneshot::Sender<()>, Arc<AtomicUsize>) {
        let (wake, timeout) = oneshot::channel();
        let drops = Arc::new(AtomicUsize::new(0));
        let lifetime = DropCount(Arc::clone(&drops));
        // Native adapters wake a stalled read with RequestTimeout. The SDK's
        // body collector itself has no host timer; model that adapter signal.
        let body = Body::from_stream(stream::once(async move {
            let _lifetime = lifetime;
            timeout
                .await
                .expect("the fixture delivers the native timeout");
            Err(EdgeError::request_timeout("private native timeout detail"))
        }));
        (body, wake, drops)
    }

    fn poll<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
        future.poll(&mut Context::from_waker(&noop_waker()))
    }

    #[test]
    fn a_stalled_adapter_read_times_out_without_config_work_and_keeps_egress_bounded() {
        let _isolation = __probe_isolation_for_tests();
        let fixture = Fixture::new();
        let (body, wake, drops) = adapter_timeout_body();
        let request = fixture.request("/status", body, IngressFraming::Chunked);
        let start = fixture.now();
        let resolutions = config_resolutions();
        let mut dispatch = Box::pin(fixture.dispatch(request, start, IngressFraming::Chunked));
        assert!(poll(dispatch.as_mut()).is_pending());
        assert_eq!(fixture.reads(), 0);
        assert_eq!(config_resolutions(), resolutions);
        assert_eq!(drops.load(Ordering::Relaxed), 0);
        *fixture.now.lock().unwrap() = start.checked_add(DEFAULT_INBOUND_READ_BUDGET).unwrap();
        wake.send(()).unwrap();
        let Poll::Ready(mut envelope) = poll(dispatch.as_mut()) else {
            panic!("the adapter timeout wakes dispatch");
        };
        assert_body_error(
            &mut envelope,
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT_BODY.as_bytes(),
        );
        assert_eq!(fixture.reads(), 0);
        assert_eq!(config_resolutions(), resolutions);
        assert_eq!(drops.load(Ordering::Relaxed), 1);
        assert_eq!(
            probe_egress_gate().in_use(),
            1,
            "the unwritten 408 still owns its permit"
        );
        let egress_start = fixture.now();
        let (_response, policy, attempt, _clock) = envelope
            .begin(ResponseEgressFramingMode::ExplicitBufferedLength)
            .unwrap_or_else(|_| panic!("the bounded 408 enters egress"));
        assert_eq!(
            policy.write_deadline.instant(),
            egress_start.checked_add(REFUSAL_WRITE_BUDGET).unwrap()
        );
        drop(attempt);
        assert_eq!(probe_egress_gate().in_use(), 0);
    }

    #[test]
    fn cancelling_a_stalled_dispatch_releases_the_body_and_admitted_probe() {
        let _isolation = __probe_isolation_for_tests();
        let fixture = Fixture::new();
        let (body, _wake, drops) = adapter_timeout_body();
        let request = fixture.request("/status", body, IngressFraming::HostManaged);
        let resolutions = config_resolutions();
        let mut dispatch =
            Box::pin(fixture.dispatch(request, fixture.now(), IngressFraming::HostManaged));
        assert!(poll(dispatch.as_mut()).is_pending());
        assert_eq!(probe_egress_gate().in_use(), 1);
        assert_eq!(fixture.reads(), 0);
        assert_eq!(config_resolutions(), resolutions);
        drop(dispatch);
        assert_eq!(drops.load(Ordering::Relaxed), 1);
        assert_eq!(probe_egress_gate().in_use(), 0);
        assert_eq!(fixture.reads(), 0);
    }
}
