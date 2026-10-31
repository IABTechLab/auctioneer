//! The Auctioneer resolver refuses unfunded SDK snapshots, not raw SDK extraction.

mod fanout_limits;
use fanout_limits as envelope;

#[cfg(test)]
mod tests {
    use super::envelope;
    use auctioneer_core::config::runtime::CONFIG_STATUS_HEADER;
    use auctioneer_core::handlers::probe_config::config_work_in_flight;
    use auctioneer_core::handlers::request_disclosure_audit::construction_costs;
    use auctioneer_core::host_primitives::{
        DelegateToDevStrategy, boot_gates, bootstrap_adapter, install_boot_envelope,
        probe_egress_gate,
    };
    use auctioneer_runtime::{
        FakeClock, ImmediateResolveTimer, NoopDecodeCooperator, ResolvedIsolateLimits, SeededRng,
        TargetPlatformConstants,
    };
    use edgezero_core::app::App;
    use edgezero_core::blob_envelope::BlobEnvelope;
    use edgezero_core::body::Body;
    use edgezero_core::config_store::{
        BoundedStoreRead, ConfigExtractionLimits, ConfigStore, ConfigStoreError, ConfigStoreHandle,
        ConfigValue,
    };
    use edgezero_core::http::{Method, Request, StatusCode, request_builder};
    use edgezero_core::ingress::{IngressDispatchOutcome, IngressFraming, IngressHeadAccounting};
    use edgezero_core::response_egress::ResponseEgressEnvelope;
    use edgezero_core::store_registry::{ConfigStoreBinding, StoreRegistry};
    use edgezero_core::time::{Deadline, MonotonicClock, MonotonicInstant};
    use futures::executor::block_on;
    use futures::future::pending;
    use futures::task::noop_waker;
    use std::future::Future as _;
    use std::iter::repeat_with;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::task::Context;

    const STORE_ID: &str = "auctioneer_runtime";
    const BANNER: &str = "funded-runtime";

    struct RecordingStore {
        value: ConfigValue,
        reads: Mutex<Vec<[u64; 2]>>,
        suspended: AtomicBool,
    }

    impl RecordingStore {
        fn new() -> Arc<Self> {
            let value = serde_json::to_string(&BlobEnvelope::new(
                serde_json::json!({"status": {"banner": BANNER}}),
                "2026-10-18T00:00:00Z".into(),
            ))
            .unwrap()
            .into();
            Arc::new(Self {
                value,
                reads: Mutex::new(Vec::new()),
                suspended: AtomicBool::new(false),
            })
        }

        fn read_count(&self) -> usize {
            self.reads.lock().unwrap().len()
        }
    }

    #[async_trait::async_trait(?Send)]
    impl ConfigStore for RecordingStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            panic!("the resolver must use bounded extraction")
        }

        async fn get_bounded(
            &self,
            key: &str,
            clock: &MonotonicClock,
            deadline: Deadline,
            max_backend_bytes: u64,
            max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
            assert_eq!(key, STORE_ID);
            self.reads
                .lock()
                .unwrap()
                .push([max_backend_bytes, max_value_bytes]);
            if self.suspended.load(Ordering::SeqCst) {
                pending::<()>().await;
            }
            if deadline.is_expired_at(clock.now()) {
                return Err(ConfigStoreError::DeadlineExceeded);
            }
            let backend_bytes = u64::try_from(self.value.len()).unwrap();
            if backend_bytes > max_backend_bytes || backend_bytes > max_value_bytes {
                return Err(ConfigStoreError::ValueTooLarge);
            }
            Ok(BoundedStoreRead {
                backend_bytes,
                value: Some(self.value.clone()),
            })
        }
    }

    fn request(path: &str, bound_store: Option<&Arc<RecordingStore>>) -> Request {
        let mut request = request_builder()
            .method(if path == "/openrtb2/auction" {
                Method::POST
            } else {
                Method::GET
            })
            .uri(path)
            .header("x-funding-canary", "never-echo-this-input")
            .body(Body::empty())
            .unwrap();
        if let Some(store) = bound_store {
            request.extensions_mut().insert(StoreRegistry::single_id(
                STORE_ID.into(),
                ConfigStoreBinding {
                    default_key: STORE_ID.into(),
                    handle: ConfigStoreHandle::new(Arc::<RecordingStore>::clone(store)),
                },
            ));
        }
        request
    }

    fn app() -> App {
        App::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default()).unwrap()
    }

    fn dispatch(
        app: &App,
        path: &str,
        store: Option<&Arc<RecordingStore>>,
    ) -> ResponseEgressEnvelope {
        let outcome = block_on(app.dispatch_ingress(
            request(path, store),
            MonotonicInstant::now(),
            IngressHeadAccounting::HostManaged,
            IngressFraming::HostManaged,
        ))
        .unwrap();
        let IngressDispatchOutcome::Response(egress) = outcome else {
            panic!("the fixture has response capacity");
        };
        *egress
    }

    fn assert_unfunded(
        app: &App,
        store: &Arc<RecordingStore>,
        path: &str,
    ) -> ResponseEgressEnvelope {
        let before = store.read_count();
        let mut egress = dispatch(app, path, Some(store));
        assert_eq!(
            store.read_count(),
            before,
            "refusal must precede every KV read"
        );
        let response = egress.response_mut();
        let body = response.body().as_bytes().unwrap();
        if path == "/status" {
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[CONFIG_STATUS_HEADER], "degraded");
            assert!(
                body.len()
                    <= usize::try_from(auctioneer_runtime::PROBE_RESPONSE_BODY_MAX_BYTES).unwrap()
            );
            assert!(
                str::from_utf8(body)
                    .unwrap()
                    .ends_with("config: degraded (unfunded_extraction)\n")
            );
            assert!(!str::from_utf8(body).unwrap().contains(BANNER));
        } else {
            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(response.headers()["cache-control"], "no-store");
            assert!(body.len() <= 256);
            assert!(
                !str::from_utf8(body)
                    .unwrap()
                    .contains("never-echo-this-input")
            );
        }
        assert!(!response.headers().contains_key("retry-after"));
        egress
    }

    fn install() -> ResolvedIsolateLimits {
        let mut limits = envelope::limits();
        limits.max_runtime_config_bytes = 0x0002_0000;
        limits.max_runtime_config_blob_bytes = 0x0001_0000;
        limits.max_config_json_nodes = 512;
        let target = TargetPlatformConstants {
            platform_concurrent_ceiling: 384,
            header_entry_overhead_bytes: 64,
            parser_header_ceiling_bytes: 21_632,
            parser_header_knob_bytes: 21_633,
            max_request_target_bytes: 768,
            h1_max_buf_size_bytes: 24_576,
            h1_max_headers: 128,
            parser_max_header_entries: 676,
            pseudoheader_allowance_bytes: 1_024,
            parser_transient_bytes: 0x4000,
            egress_handoff_factor: 2,
        };
        install_boot_envelope(
            &limits,
            &target,
            4_384,
            construction_costs(),
            auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        boot_gates().unwrap().limits
    }

    // This target has one test: Cargo's fresh binary exclusively owns boot state.
    #[test]
    fn old_app_stays_unfunded_and_new_app_recovers_after_boot() {
        bootstrap_adapter(
            || Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH)),
            || Arc::new(SeededRng::new(42)),
            || Arc::new(ImmediateResolveTimer),
            || Arc::new(NoopDecodeCooperator),
            || Arc::new(DelegateToDevStrategy),
        );
        assert!(boot_gates().is_none());
        let store = RecordingStore::new();
        let old = app();
        let captured = old.config_extraction_limits();
        // Absence remains the baked missing-binding policy, not a funding fault.
        let mut absent = dispatch(&old, "/status", None);
        assert!(
            !str::from_utf8(absent.response_mut().body().as_bytes().unwrap())
                .unwrap()
                .contains("unfunded_extraction")
        );
        drop(absent);
        for _ in 0_u8..3_u8 {
            assert_unfunded(&old, &store, "/status");
        }
        assert_eq!(store.read_count(), 0);
        assert_eq!(probe_egress_gate().in_use(), 0);
        let mut held: Vec<_> = repeat_with(|| assert_unfunded(&old, &store, "/status"))
            .take(usize::try_from(auctioneer_runtime::PROBE_RESPONSE_PERMITS).unwrap())
            .collect();
        assert_eq!(
            probe_egress_gate().in_use(),
            auctioneer_runtime::PROBE_RESPONSE_PERMITS
        );
        assert!(matches!(
            block_on(old.dispatch_ingress(
                request("/status", Some(&store)),
                MonotonicInstant::now(),
                IngressHeadAccounting::HostManaged,
                IngressFraming::HostManaged,
            ))
            .unwrap(),
            IngressDispatchOutcome::Aborted
        ));
        assert_eq!(store.read_count(), 0);
        drop(held.pop());
        drop(assert_unfunded(&old, &store, "/status"));
        drop(held);
        assert_eq!(probe_egress_gate().in_use(), 0);
        let funded = install();
        assert!(captured.max_backend_bytes > funded.max_runtime_config_bytes);
        assert_eq!(old.config_extraction_limits(), captured);
        for path in [
            "/status",
            "/info/bidders",
            "/info/bidders/fake_bidder",
            "/openrtb2/auction",
        ] {
            assert_unfunded(&old, &store, path);
        }
        let new = app();
        let mut recovered = dispatch(&new, "/status", Some(&store));
        assert_eq!(
            recovered.response_mut().headers()[CONFIG_STATUS_HEADER],
            "ok"
        );
        assert_eq!(
            recovered.response_mut().body().as_bytes().unwrap(),
            BANNER.as_bytes()
        );
        assert_eq!(
            *store.reads.lock().unwrap(),
            [[
                funded.max_runtime_config_bytes,
                funded.max_runtime_config_blob_bytes
            ]]
        );
        drop(recovered);
        assert_config_read_cancellation_recovers(&new, &store);
        assert_independent_caps(&funded);
    }

    fn assert_config_read_cancellation_recovers(app: &App, store: &Arc<RecordingStore>) {
        let before = store.read_count();
        // Cancelling a funded config read must release both probe work and output.
        store.suspended.store(true, Ordering::SeqCst);
        let mut pending = Box::pin(app.dispatch_ingress(
            request("/status", Some(store)),
            MonotonicInstant::now(),
            IngressHeadAccounting::HostManaged,
            IngressFraming::HostManaged,
        ));
        let waker = noop_waker();
        assert!(
            pending
                .as_mut()
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
        assert_eq!(store.read_count(), before.checked_add(1).unwrap());
        assert_eq!(config_work_in_flight(), 1);
        assert_eq!(probe_egress_gate().in_use(), 1);
        drop(pending);
        assert_eq!(config_work_in_flight(), 0);
        assert_eq!(probe_egress_gate().in_use(), 0);
        store.suspended.store(false, Ordering::SeqCst);
        drop(dispatch(app, "/status", Some(store)));
        assert_eq!(store.read_count(), before.checked_add(2).unwrap());
        assert_eq!(boot_gates().unwrap().aux_work_budget.in_use(), 0);
    }

    fn assert_independent_caps(funded: &ResolvedIsolateLimits) {
        let store = RecordingStore::new();
        let mut app = app();
        let equal = app.config_extraction_limits();
        assert_eq!(equal.max_backend_bytes, funded.max_runtime_config_bytes);
        assert_eq!(equal.max_blob_bytes, funded.max_runtime_config_blob_bytes);
        assert_eq!(equal.max_json_nodes, funded.max_config_json_nodes);
        for caps in [
            equal,
            ConfigExtractionLimits {
                max_backend_bytes: equal.max_backend_bytes.checked_sub(1).unwrap(),
                max_blob_bytes: equal.max_blob_bytes.checked_sub(1).unwrap(),
                max_json_nodes: equal.max_json_nodes.checked_sub(1).unwrap(),
                ..equal
            },
        ] {
            app.set_config_extraction_limits(caps).unwrap();
            let mut output = dispatch(&app, "/status", Some(&store));
            assert_eq!(output.response_mut().headers()[CONFIG_STATUS_HEADER], "ok");
            assert_eq!(
                output.response_mut().body().as_bytes().unwrap(),
                BANNER.as_bytes()
            );
        }
        assert_eq!(store.read_count(), 2);
        for caps in [
            ConfigExtractionLimits {
                max_backend_bytes: equal.max_backend_bytes.checked_add(1).unwrap(),
                ..equal
            },
            ConfigExtractionLimits {
                max_blob_bytes: equal.max_blob_bytes.checked_add(1).unwrap(),
                max_total_bytes: equal.max_total_bytes.checked_add(1).unwrap(),
                ..equal
            },
            ConfigExtractionLimits {
                max_json_nodes: equal.max_json_nodes.checked_add(1).unwrap(),
                ..equal
            },
        ] {
            app.set_config_extraction_limits(caps).unwrap();
            for path in ["/status", "/info/bidders"] {
                assert_unfunded(&app, &store, path);
            }
        }
        assert_eq!(store.read_count(), 2);
        app.set_config_extraction_limits(equal).unwrap();
        drop(dispatch(&app, "/status", Some(&store)));
        assert_eq!(store.read_count(), 3);
    }
}
