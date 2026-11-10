//! Adapters own only the immutable health counter scope.

use auctioneer_core::host_primitives::{HealthBootError, install_health_counter_scope};

pub(super) fn install() -> Result<(), HealthBootError> {
    install_health_counter_scope("isolate")
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use crate::envelope;
    use auctioneer_core::App as AuctioneerApp;
    use auctioneer_core::host_primitives::{
        DelegateToDevStrategy, DevRng, HealthBootError, SystemClock, TestClockHandle,
        bootstrap_adapter_with_envelope, counter_scope_handle, install_health_counter_scope,
        install_stderr_boot_logger_with_level,
    };
    use auctioneer_runtime::{FakeClock, ImmediateResolveTimer, NoopDecodeCooperator};
    use edgezero_adapter_cloudflare::CLOUDFLARE_PLATFORM;
    use edgezero_core::app::App;
    use edgezero_core::blob_envelope::BlobEnvelope;
    use edgezero_core::body::Body;
    use edgezero_core::config_store::{
        BoundedStoreRead, ConfigStore, ConfigStoreError, ConfigStoreHandle, ConfigValue,
        finish_bounded_config_read,
    };
    use edgezero_core::error::EdgeError;
    use edgezero_core::http::{Request, StatusCode, request_builder};
    use edgezero_core::ingress::{IngressDispatchOutcome, IngressFraming, IngressHeadAccounting};
    use edgezero_core::outbound::{
        HttpClient, IndependentOutboundProgress, OutboundBatch, OutboundDispatchRequest,
        OutboundFuture, OutboundHead, OutboundHttpClient, OutboundPending, OutboundPreparedBatch,
        OutboundProgressRuntime, OutboundRequest, OutboundSendMode, OutboundTime,
    };
    use edgezero_core::response_egress::ResponseEgressEnvelope;
    use edgezero_core::store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry};
    use edgezero_core::time::{Deadline, MonotonicClock, MonotonicInstant};
    use futures::executor::block_on;
    use log::LevelFilter;
    use serde_json::json;
    use std::env;
    use std::process::Command;
    use std::rc::Rc;
    use std::sync::Arc;
    use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

    const CHILD: &str = "AUCTIONEER__TEST__CLOUDFLARE_HEALTH_CHILD";
    const TOKEN: &str = "health-token-must-not-appear-in-logs";

    fn config_envelope(case: &str) -> String {
        let mut data = json!({"schema_version": 1_u8});
        match case {
            "anonymous" => data["gvl"] = json!({"authenticated_health": {"enabled": false}}),
            "missing-secret" => {
                data["gvl"] = json!({"authenticated_health": {
                    "enabled": true, "token_secret": "health_gvl_token"
                }});
            }
            "malformed-config" => {
                data["gvl"] = json!({"authenticated_health": {"enabled": "true"}});
            }
            _ => {}
        }
        serde_json::to_string(&BlobEnvelope::new(data, "1970-01-01T00:00:00Z".to_owned())).unwrap()
    }

    // SDK request conversion supplies this extension before app dispatch.
    // The invalid auction needs a fanout constructed, but must never send.
    struct ValidationOnlyClient;

    impl OutboundHttpClient for ValidationOnlyClient {
        fn begin(&self, _request: OutboundDispatchRequest) -> Result<OutboundPending, EdgeError> {
            Err(EdgeError::not_implemented(
                "an invalid auction must fail validation before outbound HTTP",
            ))
        }

        fn dispatch_head(&self, _request: OutboundDispatchRequest) -> OutboundFuture<OutboundHead> {
            panic!("an invalid auction must fail validation before outbound HTTP");
        }

        fn progress(&self) -> Rc<dyn OutboundProgressRuntime> {
            Rc::new(IndependentOutboundProgress)
        }

        fn start_batch_until(&self, _batch: OutboundPreparedBatch) -> OutboundBatch {
            panic!("an invalid auction must fail validation before outbound fanout");
        }

        fn time(&self) -> OutboundTime {
            panic!("an invalid auction must fail validation before outbound preparation");
        }

        fn validate_supported_options(
            &self,
            _request: &OutboundRequest,
            _mode: OutboundSendMode,
        ) -> Result<(), EdgeError> {
            Err(EdgeError::not_implemented(
                "an invalid auction must fail validation before outbound preparation",
            ))
        }
    }

    struct HealthyConfig(&'static str);

    #[async_trait::async_trait(?Send)]
    impl ConfigStore for HealthyConfig {
        async fn get(&self, key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            assert_eq!(key, "auctioneer_runtime");
            Ok(Some(config_envelope(self.0).into()))
        }

        async fn get_bounded(
            &self,
            key: &str,
            clock: &MonotonicClock,
            deadline: Deadline,
            max_backend_bytes: u64,
            max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
            assert_eq!(key, "auctioneer_runtime");
            finish_bounded_config_read(
                Ok(Some(config_envelope(self.0))),
                clock,
                deadline,
                max_backend_bytes,
                max_value_bytes,
            )
        }
    }

    fn dispatch(
        app: &App,
        mut request: Request,
        case: &'static str,
    ) -> Box<ResponseEgressEnvelope> {
        let config: ConfigRegistry = StoreRegistry::single_id(
            "auctioneer_runtime".to_owned(),
            ConfigStoreBinding {
                default_key: "auctioneer_runtime".to_owned(),
                handle: ConfigStoreHandle::new(Arc::new(HealthyConfig(case))),
            },
        );
        if case != "missing-config" {
            request.extensions_mut().insert(config);
        }
        let generated =
            OffsetDateTime::parse(auctioneer_config_data::gvl_archive_generated_at(), &Rfc3339)
                .unwrap();
        let expires =
            OffsetDateTime::from_unix_timestamp(auctioneer_config_data::gcl_expires_at_unix_secs())
                .unwrap();
        let now = generated
            .saturating_add(Duration::days(1))
            .min(expires.saturating_sub(Duration::hours(1)));
        request
            .extensions_mut()
            .insert(TestClockHandle(Arc::new(FakeClock::new(now))));
        match block_on(app.dispatch_ingress(
            request,
            MonotonicInstant::now(),
            IngressHeadAccounting::HostManaged,
            IngressFraming::HostManaged,
        ))
        .expect("health degradation must not prevent routing")
        {
            IngressDispatchOutcome::Response(envelope) => envelope,
            IngressDispatchOutcome::Aborted => {
                panic!("health degradation must not abort an ordinary request")
            }
            _ => panic!("unrecognized ingress dispatch outcome"),
        }
    }

    fn assert_routes_still_dispatch(case: &'static str) {
        let limits = envelope::limits();
        let target = envelope::target();
        bootstrap_adapter_with_envelope(
            envelope::envelope(&limits, &target),
            || Arc::new(SystemClock::new()),
            || Arc::new(DevRng::new()),
            || Arc::new(ImmediateResolveTimer),
            || Arc::new(NoopDecodeCooperator),
            || Arc::new(DelegateToDevStrategy),
        )
        .expect("the shipped envelope boots");
        let app =
            App::build::<AuctioneerApp>(CLOUDFLARE_PLATFORM).expect("the real application builds");

        let status_request = request_builder()
            .uri("/status")
            .body(Body::empty())
            .unwrap();
        let mut status = dispatch(&app, status_request, case);
        assert!(matches!(
            status.response_mut().status(),
            StatusCode::OK | StatusCode::NO_CONTENT
        ));
        assert_eq!(
            status.response_mut().headers()["x-auctioneer-config-status"],
            if matches!(case, "missing-config" | "malformed-config") {
                "degraded"
            } else {
                "ok"
            }
        );
        if matches!(case, "missing-config" | "malformed-config") {
            assert_eq!(
                status.response_mut().body().as_bytes().unwrap(),
                b"config: degraded (malformed_config)\n"
            );
        }
        drop(status);

        let mut auction_request = request_builder()
            .method("POST")
            .uri("/openrtb2/auction")
            .header("content-type", "application/json")
            .body(Body::from_bytes(
                json!({"id": "no-imps", "tmax": 1_000_u32})
                    .to_string()
                    .into_bytes(),
            ))
            .unwrap();
        auction_request
            .extensions_mut()
            .insert(HttpClient::with_client(ValidationOnlyClient));
        let mut auction = dispatch(&app, auction_request, case);
        let response = auction.response_mut();
        let body = response
            .body()
            .as_bytes()
            .expect("auction validation returns a buffered error body");
        assert_eq!(
            response.status(),
            if matches!(case, "missing-config" | "malformed-config") {
                StatusCode::INTERNAL_SERVER_ERROR
            } else {
                StatusCode::BAD_REQUEST
            },
            "the auction reaches its own config or validation; body: {}",
            String::from_utf8_lossy(body)
        );
        if matches!(case, "missing-config" | "malformed-config") {
            assert_eq!(body, b"Malformed host configuration");
        }
        drop(auction);

        for bearer in [None, Some("Bearer health-token-must-not-appear-in-logs")] {
            let mut health_builder = request_builder().uri("/_auctioneer/health/gvl");
            if let Some(value) = bearer {
                health_builder = health_builder.header("authorization", value);
            }
            let mut health = dispatch(&app, health_builder.body(Body::empty()).unwrap(), case);
            if case == "anonymous" {
                assert_eq!(health.response_mut().status(), StatusCode::OK);
            } else {
                assert_eq!(health.response_mut().status(), StatusCode::UNAUTHORIZED);
                assert_eq!(
                    health.response_mut().headers()["www-authenticate"],
                    "Bearer"
                );
            }
        }
    }

    // The counter scope is process-global; each independent case needs a child.
    #[test]
    fn health_scope_child() {
        let Ok(case) = env::var(CHILD) else {
            return;
        };
        assert!(install_stderr_boot_logger_with_level(LevelFilter::Info));
        if case == "scope-conflict" {
            install_health_counter_scope("other").unwrap();
            assert!(matches!(
                super::install(),
                Err(HealthBootError::ConflictingScope)
            ));
            assert_eq!(counter_scope_handle(), "other");
            return;
        }
        super::install().expect("counter scope has no config or secret prerequisite");
        super::install().expect("same scope installs idempotently");
        assert_eq!(counter_scope_handle(), "isolate");
        let scenario = match case.as_str() {
            "absent" => "absent",
            "anonymous" => "anonymous",
            "missing-secret" => "missing-secret",
            "missing-config" => "missing-config",
            "malformed-config" => "malformed-config",
            _ => panic!("unknown scenario"),
        };
        assert_routes_still_dispatch(scenario);
    }

    #[test]
    fn runtime_policy_controls_health_without_provider_boot_credentials() {
        for case in [
            "absent",
            "anonymous",
            "missing-secret",
            "missing-config",
            "malformed-config",
            "scope-conflict",
        ] {
            let output = Command::new(env::current_exe().unwrap())
                .args([
                    "--exact",
                    "health::tests::health_scope_child",
                    "--nocapture",
                ])
                .env(CHILD, case)
                .output()
                .unwrap();
            let logs = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.status.success(), "{case}: {logs}");
            assert!(logs.contains("1 passed"), "the child must run: {logs}");
            assert!(
                !logs.contains(TOKEN),
                "health credentials must not appear in logs"
            );
        }
    }
}
