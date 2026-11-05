//! Routed HTTP timeout handling and terminal admission ownership.

mod bounded_store;
#[cfg(test)]
use auctioneer_core::test_support::live_catalog;

#[expect(
    dead_code,
    reason = "shared support exposes helpers used by other test targets"
)]
mod usersync_app;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use super::{bounded_store, live_catalog, usersync_app};
    use auctioneer_core::{config::AuctioneerAppConfig, host_primitives::aux_gate};
    use edgezero_core::response_egress_framing::ResponseEgressFramingMode;
    use edgezero_core::{
        app::App,
        blob_envelope::BlobEnvelope,
        body::Body,
        config_store::{
            BoundedStoreRead, ConfigStore, ConfigStoreError, ConfigStoreHandle, ConfigValue,
        },
        error::EdgeError,
        http::{Method, StatusCode, request_builder},
        ingress::{IngressDispatchOutcome, IngressFraming, IngressHeadAccounting},
        response_egress::{ResponseEgressDeadline, ResponseEgressOutcome},
        store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry},
        time::{Deadline, MonotonicClock, MonotonicInstant},
    };
    use futures::{executor::block_on, stream};
    use serde_json::{Value, json};
    use std::{
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        task::Poll,
        time::Duration,
    };
    use usersync_app::body;

    struct StaticConfigStore(ConfigValue);

    #[async_trait::async_trait(?Send)]
    impl ConfigStore for StaticConfigStore {
        async fn get(&self, key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            assert_eq!(key, "auctioneer_runtime");
            Ok(Some(self.0.clone()))
        }

        async fn get_bounded(
            &self,
            key: &str,
            clock: &MonotonicClock,
            deadline: Deadline,
            max_backend_bytes: u64,
            max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
            bounded_store::bounded_over_get(
                self,
                key,
                clock,
                deadline,
                max_backend_bytes,
                max_value_bytes,
            )
            .await
        }
    }

    fn config_registry() -> ConfigRegistry {
        let config = BlobEnvelope::new(
            serde_json::to_value(toml::from_str::<AuctioneerAppConfig>(
                "schema_version = 1\n[host.user_sync]\nexternal_url='https://sync.example.test'\n",
            ).unwrap()).unwrap(),
            "1970-01-01T00:00:00Z".to_owned(),
        );
        StoreRegistry::single_id(
            "auctioneer_runtime".to_owned(),
            ConfigStoreBinding {
                default_key: "auctioneer_runtime".to_owned(),
                handle: ConfigStoreHandle::new(Arc::new(StaticConfigStore(
                    serde_json::to_string(&config).unwrap().into(),
                ))),
            },
        )
    }

    #[test]
    fn cookie_sync_rejects_non_object_json_before_reading_signing_keys() {
        let origin = "https://publisher.example";
        let app = usersync_app::TestApp::new(
            "schema_version = 1\n[host.user_sync]\nexternal_url = \"https://sync.example.test\"\nallowed_origins = ['https://publisher.example']\n",
        );
        for payload in [
            json!([]),
            json!([[]]),
            json!([[], 0_i32]),
            json!([
                [live_catalog::redirect().name()],
                0_i32,
                null,
                null,
                null,
                null,
                false,
                1_i32,
                null,
                "publisher",
                true
            ]),
            json!(null),
            json!(true),
            json!(0_i32),
            json!("request"),
        ] {
            let response = app.dispatch(
                Method::POST,
                "/cookie_sync",
                &[("origin", origin), ("content-type", "application/json")],
                Body::from_bytes(serde_json::to_vec(&payload).unwrap()),
            );
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{payload}");
            assert_eq!(body(&response), b"JSON parsing failed: invalid JSON\n");
            assert_eq!(
                response.headers()["content-type"],
                "text/plain; charset=utf-8"
            );
            assert_eq!(response.headers()["cache-control"], "no-store");
            assert_eq!(response.headers()["access-control-allow-origin"], origin);
            assert_eq!(
                response.headers()["access-control-allow-credentials"],
                "true"
            );
            assert!(!response.headers().contains_key("set-cookie"));
            assert_eq!(app.secrets().reads(), 0);
        }

        let response = app.dispatch(
            Method::POST,
            "/cookie_sync",
            &[("origin", origin), ("content-type", "application/json")],
            Body::from_bytes(
                serde_json::to_vec(
                    &json!({"gdpr": 0_i32, "bidders": [live_catalog::redirect().name()]}),
                )
                .unwrap(),
            ),
        );
        assert_eq!(response.status(), StatusCode::OK);
        let payload: Value = serde_json::from_slice(body(&response)).unwrap();
        assert_eq!(payload["bidder_status"].as_array().unwrap().len(), 1);
        assert_eq!(
            payload["bidder_status"][0]["bidder"],
            live_catalog::redirect().name()
        );
        assert_eq!(app.secrets().reads(), 1);
    }

    #[test]
    fn cookie_sync_body_timeout_uses_bounded_request_timeout_response() {
        let app = usersync_app::TestApp::new(
            "schema_version = 1\n[host.user_sync]\nexternal_url = \"https://sync.example.test\"\nallowed_origins = ['https://publisher.example']\n",
        );
        let response = app.dispatch(
            Method::POST,
            "/cookie_sync",
            &[("origin", "https://publisher.example")],
            Body::from_stream(stream::iter([Err(EdgeError::request_timeout(
                "private body timeout detail",
            ))])),
        );
        assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
        assert_eq!(body(&response), b"request timeout\n");
        assert_eq!(
            response.headers()["content-type"],
            "text/plain; charset=utf-8"
        );
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(!response.headers().contains_key("set-cookie"));
        assert!(
            !response
                .headers()
                .contains_key("access-control-allow-origin")
        );
        assert!(
            response
                .extensions()
                .get::<ResponseEgressDeadline>()
                .is_some()
        );
    }

    #[test]
    fn cookie_sync_body_timeout_outlives_cpu_deadline_and_retains_admission_until_egress() {
        let _serial = usersync_app::TestApp::new(
            "schema_version = 1\n[host.user_sync]\nexternal_url = 'https://sync.example.test'\n",
        );
        let gate = aux_gate().unwrap();
        assert_eq!(gate.in_use(), 0);
        let mut app =
            App::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default()).unwrap();
        let start = MonotonicInstant::now();
        let expired = start + Duration::from_secs(31);
        let clock = Arc::new(Mutex::new(start));
        let observed = Arc::clone(&clock);
        app.set_monotonic_clock(MonotonicClock::new(move || *observed.lock().unwrap()));
        let polls = Arc::new(AtomicUsize::new(0));
        let body_polls = Arc::clone(&polls);
        let body_gate = Arc::clone(&gate);
        let request_body = Body::from_stream(stream::poll_fn(move |_| {
            assert_eq!(body_gate.in_use(), 1);
            assert_eq!(body_polls.fetch_add(1, Ordering::SeqCst), 0);
            *clock.lock().unwrap() = expired;
            Poll::Ready(Some(Err(EdgeError::request_timeout(
                "private timed-out body detail",
            ))))
        }));
        let mut request = request_builder()
            .method(Method::POST)
            .uri("/cookie_sync")
            .body(request_body)
            .unwrap();
        request.extensions_mut().insert(config_registry());
        let outcome = block_on(app.dispatch_ingress(
            request,
            start,
            IngressHeadAccounting::HostManaged,
            IngressFraming::HostManaged,
        ))
        .unwrap();
        let IngressDispatchOutcome::Response(mut envelope) = outcome else {
            panic!("body timeout must receive a bounded response");
        };
        let response = envelope.response_mut();
        assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
        assert_eq!(body(response), b"request timeout\n");
        assert_eq!(
            response.headers()["content-type"],
            "text/plain; charset=utf-8"
        );
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(!response.headers().contains_key("set-cookie"));
        assert!(
            response
                .extensions()
                .get::<ResponseEgressDeadline>()
                .is_some()
        );
        assert_eq!(polls.load(Ordering::SeqCst), 1);
        assert_eq!(
            gate.in_use(),
            1,
            "handler return retains the admission receipt"
        );
        let (prepared, _policy, mut attempt, egress_clock) = envelope
            .begin(ResponseEgressFramingMode::ExplicitBufferedLength)
            .unwrap_or_else(|_| panic!("timeout response begins egress"));
        assert!(attempt.begin_writing());
        drop(prepared);
        assert_eq!(
            gate.in_use(),
            1,
            "prepared response drop must not release the receipt"
        );
        assert!(attempt.terminate(ResponseEgressOutcome::Completed, egress_clock.now()));
        assert_eq!(gate.in_use(), 0);
        assert!(!attempt.terminate(ResponseEgressOutcome::Completed, egress_clock.now()));
        drop(attempt);
        assert_eq!(
            gate.in_use(),
            0,
            "terminal egress releases the receipt once"
        );
    }
}
