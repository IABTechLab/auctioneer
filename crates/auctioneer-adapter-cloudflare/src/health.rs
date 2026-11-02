//! Health-only degradation shared by production fetches and host regressions.

use auctioneer_core::host_primitives::{
    BearerToken, HealthAccessPolicy, HealthBootError, decide_health_policy,
    install_health_primitives, read_bool_flag,
};

pub(super) const FLAG_VARIABLE: &str = "AUCTIONEER__RUNTIME__ALLOW_UNAUTHENTICATED_HEALTH";

/// Resolution faults close health without stopping routing. Installation
/// conflicts remain fatal because an isolate's frozen policy cannot change.
pub(super) fn install(
    raw_flag: Option<&str>,
    raw_token: Option<String>,
) -> Result<(), HealthBootError> {
    let resolved = read_bool_flag(FLAG_VARIABLE, raw_flag).and_then(|allow_unauth| {
        decide_health_policy(raw_token.and_then(BearerToken::new), allow_unauth)
    });
    let policy = match resolved {
        Ok(policy) => policy,
        Err(err) => {
            log::error!(
                target: "auctioneer::health",
                "health policy unresolved ({err}); serving with health CLOSED so \
                 /status and auctions remain available"
            );
            HealthAccessPolicy::Closed
        }
    };
    install_health_primitives(policy, "isolate")
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use crate::envelope;
    use auctioneer_core::App as AuctioneerApp;
    use auctioneer_core::host_primitives::{
        DelegateToDevStrategy, DevRng, HealthAccessPolicy, HealthAuthz, HealthBootError,
        SystemClock, bootstrap_adapter_with_envelope, counter_scope_handle, health_authorize,
        health_policy_handle, install_health_primitives, install_stderr_boot_logger_with_level,
    };
    use auctioneer_runtime::{ImmediateResolveTimer, NoopDecodeCooperator};
    use edgezero_adapter_cloudflare::CLOUDFLARE_PLATFORM;
    use edgezero_core::app::App;
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

    const CHILD: &str = "AUCTIONEER__TEST__CLOUDFLARE_HEALTH_CHILD";
    const TOKEN: &str = "health-token-must-not-appear-in-logs";
    const FLAG_TOKEN: &str = "flagHeadF8v3.flagBodyJ7m2.flagSigQ5n9";
    const PADDED_FLAG_TOKEN: &str = " \tflagHeadF8v3.flagBodyJ7m2.flagSigQ5n9\n";

    fn assert_no_health_credentials(rendered: &str) {
        for sensitive in [
            TOKEN,
            "health-token",
            "must-not-appear-in-logs",
            FLAG_TOKEN,
            "flagHeadF8v3",
            "flagBodyJ7m2",
            "flagSigQ5n9",
            "F8v3",
            "J7m2",
            "Q5n9",
        ] {
            assert!(
                !rendered.contains(sensitive),
                "health credentials and their segments must not be exposed"
            );
        }
    }

    #[test]
    fn rejected_health_flags_preserve_key_and_type_without_raw_values() {
        for raw_flag in [
            FLAG_TOKEN,
            PADDED_FLAG_TOKEN,
            TOKEN,
            "not-a-boolean",
            "TRUE",
            "",
        ] {
            let error = super::read_bool_flag(super::FLAG_VARIABLE, Some(raw_flag))
                .expect_err("a non-boolean flag must be rejected");
            assert!(matches!(
                &error,
                HealthBootError::NonBooleanFlag { key } if *key == super::FLAG_VARIABLE
            ));
            let display = error.to_string();
            let debug = format!("{error:?}");
            assert!(display.contains("not a strict boolean"));
            assert!(debug.contains("NonBooleanFlag"));
            for rendered in [display, debug] {
                assert!(rendered.contains(super::FLAG_VARIABLE));
                assert_no_health_credentials(&rendered);
                if !raw_flag.trim().is_empty() {
                    assert!(
                        !rendered.contains(raw_flag.trim()),
                        "neither Display nor Debug may expose the rejected flag"
                    );
                }
            }
        }
    }

    // A valid envelope over the canonical empty object keeps application
    // configuration independent of the health inputs being exercised.
    fn config_envelope() -> String {
        json!({
            "data": {},
            "sha256": "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a",
            "version": 1_u32,
            "generated_at": "1970-01-01T00:00:00Z",
        })
        .to_string()
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

    struct HealthyConfig;

    #[async_trait::async_trait(?Send)]
    impl ConfigStore for HealthyConfig {
        async fn get(&self, key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            assert_eq!(key, "auctioneer_runtime");
            Ok(Some(config_envelope().into()))
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
                Ok(Some(config_envelope())),
                clock,
                deadline,
                max_backend_bytes,
                max_value_bytes,
            )
        }
    }

    fn dispatch(app: &App, mut request: Request) -> Box<ResponseEgressEnvelope> {
        let config: ConfigRegistry = StoreRegistry::single_id(
            "auctioneer_runtime".to_owned(),
            ConfigStoreBinding {
                default_key: "auctioneer_runtime".to_owned(),
                handle: ConfigStoreHandle::new(Arc::new(HealthyConfig)),
            },
        );
        request.extensions_mut().insert(config);
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

    fn assert_closed_routes_still_dispatch() {
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
        let mut status = dispatch(&app, status_request);
        assert!(matches!(
            status.response_mut().status(),
            StatusCode::OK | StatusCode::NO_CONTENT
        ));
        assert_eq!(
            status.response_mut().headers()["x-auctioneer-config-status"],
            "ok",
            "a health-only fault must not change the application config verdict"
        );
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
        let mut auction = dispatch(&app, auction_request);
        let response = auction.response_mut();
        let body = response
            .body()
            .as_bytes()
            .expect("auction validation returns a buffered error body");
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "the auction reaches its own validation rather than failing health boot; body: {}",
            String::from_utf8_lossy(body)
        );
        drop(auction);

        for bearer in [None, Some("Bearer health-token-must-not-appear-in-logs")] {
            let mut health_builder = request_builder().uri("/_auctioneer/health/gvl");
            if let Some(value) = bearer {
                health_builder = health_builder.header("authorization", value);
            }
            let mut health = dispatch(&app, health_builder.body(Body::empty()).unwrap());
            assert_eq!(health.response_mut().status(), StatusCode::UNAUTHORIZED);
            assert_eq!(
                health.response_mut().headers()["www-authenticate"],
                "Bearer"
            );
        }
    }

    fn scenario_inputs(case: &str) -> (Option<&'static str>, Option<&'static str>) {
        match case {
            "malformed" => (Some("not-a-boolean"), None),
            "malformed-with-token" => (Some("TRUE"), Some(TOKEN)),
            "token-in-flag" => (Some(FLAG_TOKEN), None),
            "padded-token-in-flag" => (Some(PADDED_FLAG_TOKEN), None),
            "token-in-flag-with-token" => (Some(FLAG_TOKEN), Some(TOKEN)),
            "blank" => (Some(""), None),
            "conflicting" => (Some("true"), Some(TOKEN)),
            "absent" => (None, None),
            "default" => (Some("false"), None),
            "empty-token" => (Some("false"), Some("")),
            "open" | "installed-open" => (Some(" true "), None),
            "token" | "installed-token" => (Some("false"), Some(TOKEN)),
            _ => panic!("unknown health scenario"),
        }
    }

    fn assert_scope_conflict_preserves_policy() {
        install_health_primitives(HealthAccessPolicy::Closed, "other").unwrap();
        assert!(matches!(
            super::install(Some("invalid"), None),
            Err(HealthBootError::ConflictingPolicy)
        ));
        assert_eq!(counter_scope_handle(), "other");
        assert!(matches!(health_authorize(Some(TOKEN)), HealthAuthz::Deny));
    }

    fn assert_installed_policy_preserved(case: &str) {
        assert!(matches!(
            super::install(Some("invalid"), None),
            Err(HealthBootError::ConflictingPolicy)
        ));
        assert!(matches!(health_authorize(Some(TOKEN)), HealthAuthz::Allow));
        assert_eq!(
            health_authorize(None),
            if case == "installed-open" {
                HealthAuthz::Allow
            } else {
                HealthAuthz::Deny
            },
            "a failed fallback preserves the previously installed policy"
        );
    }

    fn assert_closed_policy_cannot_be_replaced() {
        assert!(matches!(
            health_policy_handle(),
            Some(HealthAccessPolicy::Closed)
        ));
        assert!(matches!(health_authorize(None), HealthAuthz::Deny));
        assert!(matches!(health_authorize(Some(TOKEN)), HealthAuthz::Deny));
        assert_closed_routes_still_dispatch();
        for (replacement_flag, replacement_token) in
            [(Some("true"), None), (Some("false"), Some(TOKEN))]
        {
            assert!(matches!(
                super::install(replacement_flag, replacement_token.map(str::to_owned)),
                Err(HealthBootError::ConflictingPolicy)
            ));
            assert!(matches!(health_authorize(Some(TOKEN)), HealthAuthz::Deny));
        }
    }

    fn assert_resolved_policy(case: &str) {
        match case {
            "open" => {
                assert!(matches!(
                    health_policy_handle(),
                    Some(HealthAccessPolicy::UnauthenticatedAcknowledged)
                ));
                assert!(matches!(health_authorize(None), HealthAuthz::Allow));
            }
            "token" => {
                assert!(matches!(
                    health_policy_handle(),
                    Some(HealthAccessPolicy::Token(_))
                ));
                assert!(matches!(health_authorize(None), HealthAuthz::Deny));
                assert!(matches!(health_authorize(Some("wrong")), HealthAuthz::Deny));
                assert!(matches!(health_authorize(Some(TOKEN)), HealthAuthz::Allow));
            }
            _ => assert_closed_policy_cannot_be_replaced(),
        }
    }

    // The policy and counter scope are process-global OnceLocks. Each
    // scenario needs its own process, including repeated Worker fetches.
    #[test]
    fn health_policy_child() {
        let Ok(case) = env::var(CHILD) else {
            return;
        };
        assert!(install_stderr_boot_logger_with_level(LevelFilter::Info));
        assert!(health_policy_handle().is_none());
        if case == "installed-scope" {
            assert_scope_conflict_preserves_policy();
            return;
        }
        let (flag, token) = scenario_inputs(&case);
        super::install(flag, token.map(str::to_owned))
            .expect("bad health input degrades, but does not fail request boot");
        super::install(flag, token.map(str::to_owned)).expect("repeat installs are idempotent");
        assert_eq!(counter_scope_handle(), "isolate");

        if case.starts_with("installed-") {
            assert_installed_policy_preserved(&case);
        } else {
            assert_resolved_policy(&case);
        }
    }

    #[test]
    fn health_faults_close_only_health_and_preserve_frozen_policy() {
        for (case, diagnostics) in [
            ("malformed", 2),
            ("malformed-with-token", 2),
            ("token-in-flag", 2),
            ("padded-token-in-flag", 2),
            ("token-in-flag-with-token", 2),
            ("blank", 2),
            ("conflicting", 2),
            ("absent", 0),
            ("default", 0),
            ("empty-token", 0),
            ("open", 0),
            ("token", 0),
            ("installed-open", 1),
            ("installed-token", 1),
            ("installed-scope", 1),
        ] {
            let output = Command::new(env::current_exe().expect("test binary"))
                .args([
                    "--exact",
                    "health::tests::health_policy_child",
                    "--nocapture",
                ])
                .env(CHILD, case)
                .output()
                .expect("isolated health process");
            let logs = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.status.success(), "{case}: {logs}");
            assert!(logs.contains("1 passed"), "the child must run: {logs}");
            assert_eq!(
                logs.matches("health policy unresolved").count(),
                diagnostics,
                "{case}: malformed input is diagnosed, not silently ignored: {logs}"
            );
            for diagnostic in logs
                .lines()
                .filter(|line| line.contains("health policy unresolved"))
            {
                assert!(diagnostic.contains("ERROR auctioneer::health:"));
                if case != "conflicting" {
                    assert!(
                        diagnostic.contains(super::FLAG_VARIABLE),
                        "{case}: the rejected flag key must remain diagnosable: {logs}"
                    );
                    assert!(
                        diagnostic.contains("not a strict boolean"),
                        "{case}: the wrong-type reason must remain diagnosable: {logs}"
                    );
                }
            }
            assert_no_health_credentials(&logs);
            if matches!(
                case,
                "malformed"
                    | "malformed-with-token"
                    | "token-in-flag"
                    | "padded-token-in-flag"
                    | "token-in-flag-with-token"
            ) {
                let (flag, _) = scenario_inputs(case);
                assert!(
                    !logs.contains(flag.expect("malformed flag scenario").trim()),
                    "the process logger must not expose the rejected flag"
                );
            }
        }
    }
}
