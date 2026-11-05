//! Declared config node caps bind real extraction and the normalized budget.
//! App snapshots are exercised across boot; this is not an allocator bound.

#[cfg(test)]
mod bounded_store;
#[cfg(test)]
mod fanout_limits;
use fanout_limits as envelope;

#[cfg(test)]
mod tests {
    use super::{bounded_store::bounded_over_get, envelope};
    use auctioneer_core::configure::configure_app;
    use auctioneer_core::handlers::request_disclosure_audit::construction_costs;
    use auctioneer_core::host_primitives::{boot_limits, install_boot_envelope};
    use auctioneer_core::ingress::PROBE_CLASS;
    use auctioneer_runtime::budget::derive_aux_work_for_profile;
    use auctioneer_runtime::{
        ResolvedIsolateLimits, TargetPlatformConstants, min_aux_route_peak_bytes,
        serialized_json_bytes,
    };
    use edgezero_core::app::App;
    use edgezero_core::blob_envelope::BlobEnvelope;
    use edgezero_core::body::Body;
    use edgezero_core::config_store::{
        BoundedStoreRead, ConfigExtractionLimits, ConfigStore, ConfigStoreError, ConfigStoreHandle,
        ConfigValue, DEFAULT_CONFIG_JSON_NODES,
    };
    use edgezero_core::context::RequestContext;
    use edgezero_core::error::{EdgeError, StoreExtractionReason};
    use edgezero_core::extractor::AppConfig;
    use edgezero_core::ingress::{IngressDispatchOutcome, IngressFraming, IngressHeadAccounting};
    use edgezero_core::router::RouterService;
    use edgezero_core::store_registry::{ConfigStoreBinding, StoreRegistry};
    use edgezero_core::time::{Deadline, MonotonicClock, MonotonicInstant};
    use futures::executor::block_on;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use std::env::{current_exe, var};
    use std::io::Read;
    use std::process::{Command, Stdio, id};
    use std::sync::{Arc, Mutex};
    use std::thread::{sleep, spawn};
    use std::time::{Duration, Instant};

    const STORE_ID: &str = "auctioneer_runtime";
    const NODES_ENV: &str = "AUCTIONEER__TEST__CONFIG_NODE_CAP__CHILD_NODES";
    const SNAPSHOT_ENV: &str = "AUCTIONEER__TEST__CONFIG_NODE_CAP__SNAPSHOT_CHILD";
    const SNAPSHOT_TEST: &str = "tests::preboot_app_retains_extraction_limits_across_boot";
    const SNAPSHOT_RECEIPT: &str = "CONFIG_SNAPSHOT_RECEIPT ";
    const DECLARED_NODES: u32 = 0x1000;
    const PAYLOAD_NODES: u32 = 0x1800;
    const DECLARED_BACKEND_BYTES: u64 = 0x0002_0000;
    const DECLARED_BLOB_BYTES: u64 = 0x0001_0000;

    #[derive(Debug, Deserialize, edgezero_core::AppConfig, validator::Validate)]
    struct NodeConfig {
        nodes: Vec<()>,
    }

    struct Store(ConfigValue);

    #[async_trait::async_trait(?Send)]
    impl ConfigStore for Store {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
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
            bounded_over_get(
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

    struct RecordingStore {
        store: Store,
        read_caps: Mutex<Vec<[u64; 2]>>,
    }

    impl RecordingStore {
        fn record_read(&self, backend_bytes: u64, value_bytes: u64) {
            let mut read_caps = self.read_caps.lock().unwrap();
            assert!(
                read_caps.len() < 3,
                "only three snapshot reads are expected"
            );
            read_caps.push([backend_bytes, value_bytes]);
        }
    }

    #[async_trait::async_trait(?Send)]
    impl ConfigStore for RecordingStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            panic!("the SDK must use bounded config reads")
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
            self.record_read(max_backend_bytes, max_value_bytes);
            bounded_over_get(
                &self.store,
                key,
                clock,
                deadline,
                max_backend_bytes,
                max_value_bytes,
            )
            .await
        }
    }

    async fn extract_nodes(ctx: RequestContext) -> Result<String, EdgeError> {
        match AppConfig::<NodeConfig>::from_store(&ctx, STORE_ID, None).await {
            Ok(config) => Ok(config.nodes.len().to_string()),
            Err(error) => {
                assert_eq!(
                    error.store_extraction_reason(),
                    Some(StoreExtractionReason::StructuralLimit),
                    "only the node cap may reject these valid, small, shallow envelopes",
                );
                Ok("structural_limit".into())
            }
        }
    }

    #[test]
    fn declared_node_cap_binds_real_extraction_in_fresh_processes() {
        if let Ok(nodes) = var(NODES_ENV) {
            assert_extraction_boundary(nodes.parse().unwrap());
            return;
        }
        for nodes in [
            DEFAULT_CONFIG_JSON_NODES.saturating_div(2),
            DEFAULT_CONFIG_JSON_NODES * 2,
            0,
        ] {
            let child = Command::new(current_exe().unwrap())
                .args([
                    "--exact",
                    "tests::declared_node_cap_binds_real_extraction_in_fresh_processes",
                    "--nocapture",
                ])
                .env(NODES_ENV, nodes.to_string())
                .output()
                .unwrap();
            let output = format!(
                "{}{}",
                String::from_utf8_lossy(&child.stdout),
                String::from_utf8_lossy(&child.stderr),
            );
            assert!(child.status.success(), "declared nodes={nodes}: {output}");
            assert!(output.contains("1 passed"), "child must execute: {output}");
        }
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    #[serde(deny_unknown_fields)]
    struct SnapshotReceipt {
        process_id: u32,
        installations: u32,
        payload_nodes: u32,
        node_caps: [u32; 3],
        read_caps: Vec<[u64; 2]>,
        outcomes: [String; 3],
    }

    fn expected_snapshot_receipt(process_id: u32) -> SnapshotReceipt {
        let defaults = ConfigExtractionLimits::default();
        let default_reads = [defaults.max_backend_bytes, defaults.max_blob_bytes];
        let accepted = (PAYLOAD_NODES - 6).to_string();
        SnapshotReceipt {
            process_id,
            installations: 1,
            payload_nodes: PAYLOAD_NODES,
            node_caps: [
                defaults.max_json_nodes,
                defaults.max_json_nodes,
                DECLARED_NODES,
            ],
            read_caps: vec![
                default_reads,
                default_reads,
                [DECLARED_BACKEND_BYTES, DECLARED_BLOB_BYTES],
            ],
            outcomes: [accepted.clone(), accepted, "structural_limit".into()],
        }
    }

    fn verify_snapshot_receipt(output: &str, process_id: u32) -> Result<(), &'static str> {
        let mut receipts = output
            .lines()
            .filter_map(|line| line.strip_prefix(SNAPSHOT_RECEIPT));
        let encoded = receipts.next().ok_or("missing snapshot receipt")?;
        if receipts.next().is_some() {
            return Err("duplicate snapshot receipt");
        }
        let receipt: SnapshotReceipt =
            serde_json::from_str(encoded).map_err(|_error| "invalid snapshot receipt")?;
        if receipt != expected_snapshot_receipt(process_id) {
            return Err("snapshot execution or counts differ");
        }
        Ok(())
    }

    #[test]
    fn snapshot_receipt_requires_all_three_extractions() {
        let receipt = expected_snapshot_receipt(17);
        let output = format!(
            "{SNAPSHOT_RECEIPT}{}\n",
            serde_json::to_string(&receipt).unwrap()
        );
        assert_eq!(verify_snapshot_receipt(&output, 17), Ok(()));
        assert!(verify_snapshot_receipt("test result: ok. 0 passed", 17).is_err());
        assert!(verify_snapshot_receipt(&format!("{output}{output}"), 17).is_err());
        assert!(verify_snapshot_receipt(&output, 18).is_err());
        for reads in [0, 1, 2, 4] {
            let mut wrong_count = expected_snapshot_receipt(17);
            wrong_count.read_caps.resize(reads, [0, 0]);
            let invalid = format!(
                "{SNAPSHOT_RECEIPT}{}\n",
                serde_json::to_string(&wrong_count).unwrap()
            );
            assert!(verify_snapshot_receipt(&invalid, 17).is_err());
        }
        for installations in [0, 2] {
            let mut wrong_installs = expected_snapshot_receipt(17);
            wrong_installs.installations = installations;
            let invalid = format!(
                "{SNAPSHOT_RECEIPT}{}\n",
                serde_json::to_string(&wrong_installs).unwrap()
            );
            assert!(verify_snapshot_receipt(&invalid, 17).is_err());
        }
    }

    #[test]
    #[expect(clippy::print_stdout, reason = "emit the counted child snapshot proof")]
    fn preboot_app_retains_extraction_limits_across_boot() {
        assert!(boot_limits().is_none());
        if let Ok(child) = var(SNAPSHOT_ENV) {
            assert_eq!(child, "1");
            let receipt = exercise_app_snapshot();
            println!(
                "\n{SNAPSHOT_RECEIPT}{}",
                serde_json::to_string(&receipt).unwrap()
            );
            return;
        }
        run_snapshot_child();
        assert!(boot_limits().is_none(), "the parent must never install");
    }

    fn run_snapshot_child() {
        const CAPTURE_LIMIT: u64 = 0x8000;
        let mut child = Command::new(current_exe().unwrap())
            .env_clear()
            .env(SNAPSHOT_ENV, "1")
            .args(["--exact", SNAPSHOT_TEST, "--nocapture", "--test-threads=1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let process_id = child.id();
        let capture = |pipe: &mut dyn Read| {
            let mut bytes = Vec::new();
            pipe.take(CAPTURE_LIMIT + 1)
                .read_to_end(&mut bytes)
                .unwrap();
            bytes
        };
        let mut stdout_pipe = child.stdout.take().unwrap();
        let mut stderr_pipe = child.stderr.take().unwrap();
        let stdout_reader = spawn(move || capture(&mut stdout_pipe));
        let stderr_reader = spawn(move || capture(&mut stderr_pipe));
        let deadline = Instant::now().checked_add(Duration::from_secs(30)).unwrap();
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break Some(status);
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                break None;
            }
            sleep(Duration::from_millis(10));
        };
        let stdout_bytes = stdout_reader.join().unwrap();
        let stderr_bytes = stderr_reader.join().unwrap();
        assert!(u64::try_from(stdout_bytes.len()).unwrap() <= CAPTURE_LIMIT);
        assert!(u64::try_from(stderr_bytes.len()).unwrap() <= CAPTURE_LIMIT);
        let stdout = String::from_utf8(stdout_bytes).unwrap();
        let stderr = String::from_utf8(stderr_bytes).unwrap();
        assert!(status.is_some(), "snapshot child exceeded 30 seconds");
        assert!(status.unwrap().success(), "{stdout}\n{stderr}");
        assert_eq!(
            verify_snapshot_receipt(&stdout, process_id),
            Ok(()),
            "{stdout}"
        );
    }

    fn configured_probe_app() -> App {
        let mut app = App::new(
            RouterService::builder()
                .route_with_class("/status", http::Method::GET, PROBE_CLASS, extract_nodes)
                .build(),
        );
        configure_app(&mut app).unwrap();
        app
    }

    fn extract_snapshot(app: &App, store: &Arc<RecordingStore>) -> String {
        let before = store.read_caps.lock().unwrap().len();
        let shared_store = Arc::clone(store);
        let registry = StoreRegistry::single_id(
            STORE_ID.into(),
            ConfigStoreBinding {
                default_key: STORE_ID.into(),
                handle: ConfigStoreHandle::new(shared_store),
            },
        );
        let mut request = http::Request::builder()
            .uri("/status")
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(registry);
        let IngressDispatchOutcome::Response(mut egress) = block_on(app.dispatch_ingress(
            request,
            MonotonicInstant::now(),
            IngressHeadAccounting::HostManaged,
            IngressFraming::HostManaged,
        ))
        .unwrap() else {
            panic!("probe admission must reach actual extraction, even before boot");
        };
        let response = egress.response_mut();
        assert_eq!(response.status(), http::StatusCode::OK);
        assert_eq!(
            store.read_caps.lock().unwrap().len(),
            before.checked_add(1).unwrap()
        );
        String::from_utf8(response.body().as_bytes().unwrap().to_vec()).unwrap()
    }

    fn config_value_nodes(value: &serde_json::Value) -> u32 {
        let children = match value {
            serde_json::Value::Array(values) => values.iter().map(config_value_nodes).sum::<u32>(),
            serde_json::Value::Object(values) => {
                values.values().map(config_value_nodes).sum::<u32>()
            }
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_) => 0,
        };
        children.checked_add(1).unwrap()
    }

    fn exercise_app_snapshot() -> SnapshotReceipt {
        let defaults = ConfigExtractionLimits::default();
        let prebuilt = configured_probe_app();
        let captured = prebuilt.config_extraction_limits();
        assert_eq!(captured.max_backend_bytes, defaults.max_backend_bytes);
        assert_eq!(captured.max_blob_bytes, defaults.max_blob_bytes);
        assert_eq!(captured.max_json_nodes, defaults.max_json_nodes);

        let payload = serde_json::to_value(BlobEnvelope::new(
            json!({"nodes": vec![(); usize::try_from(PAYLOAD_NODES - 6).unwrap()]}),
            "2026-10-18T00:00:00Z".into(),
        ))
        .unwrap();
        let payload_nodes = config_value_nodes(&payload);
        assert_eq!(payload_nodes, PAYLOAD_NODES);
        assert!(DECLARED_NODES < payload_nodes && payload_nodes < defaults.max_json_nodes);
        let blob = serde_json::to_string(&payload).unwrap();
        let bytes = u64::try_from(blob.len()).unwrap();
        assert!(bytes < DECLARED_BLOB_BYTES.min(DECLARED_BACKEND_BYTES));
        let store = Arc::new(RecordingStore {
            store: Store(blob.into()),
            read_caps: Mutex::new(Vec::new()),
        });
        let before_boot = extract_snapshot(&prebuilt, &store);
        assert!(boot_limits().is_none());

        let mut declared = envelope::limits();
        declared.max_runtime_config_bytes = DECLARED_BACKEND_BYTES;
        declared.max_runtime_config_blob_bytes = DECLARED_BLOB_BYTES;
        declared.max_config_json_nodes = DECLARED_NODES;
        let target = target();
        fund_auxiliary_work(&mut declared, &target, DECLARED_NODES);
        let mut installations = 0_u32;
        install_boot_envelope(
            &declared,
            &target,
            4_384,
            construction_costs(),
            auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        installations = installations.checked_add(1).unwrap();
        let installed = boot_limits().unwrap();
        assert_eq!(installed.max_runtime_config_bytes, DECLARED_BACKEND_BYTES);
        assert_eq!(installed.max_runtime_config_blob_bytes, DECLARED_BLOB_BYTES);
        assert_eq!(installed.max_config_json_nodes, DECLARED_NODES);

        assert_eq!(prebuilt.config_extraction_limits(), captured);
        let retained_after_boot = extract_snapshot(&prebuilt, &store);
        let postboot = configured_probe_app();
        let postboot_caps = postboot.config_extraction_limits();
        assert_eq!(
            postboot_caps.max_backend_bytes,
            installed.max_runtime_config_bytes
        );
        assert_eq!(
            postboot_caps.max_blob_bytes,
            installed.max_runtime_config_blob_bytes
        );
        assert_eq!(
            postboot_caps.max_json_nodes,
            installed.max_config_json_nodes
        );
        let newly_configured = extract_snapshot(&postboot, &store);
        SnapshotReceipt {
            process_id: id(),
            installations,
            payload_nodes,
            node_caps: [
                captured.max_json_nodes,
                prebuilt.config_extraction_limits().max_json_nodes,
                postboot_caps.max_json_nodes,
            ],
            read_caps: store.read_caps.lock().unwrap().clone(),
            outcomes: [before_boot, retained_after_boot, newly_configured],
        }
    }

    fn fund_auxiliary_work(
        declared: &mut ResolvedIsolateLimits,
        target: &TargetPlatformConstants,
        expected_nodes: u32,
    ) {
        let mut effective = *declared;
        effective.max_config_json_nodes = expected_nodes;
        let baked_bytes = serialized_json_bytes(&(
            auctioneer_config_data::host_default(),
            auctioneer_config_data::account_default(),
        ))
        .unwrap();
        declared.aux_route_peak_bytes = declared.aux_route_peak_bytes.max(
            derive_aux_work_for_profile(&effective, target, baked_bytes)
                .unwrap()
                .peak_bytes(),
        );
    }

    fn assert_extraction_boundary(declared_nodes: u32) {
        let expected_nodes = if declared_nodes == 0 {
            DEFAULT_CONFIG_JSON_NODES
        } else {
            declared_nodes
        };
        let mut declared = envelope::limits();
        declared.max_config_json_nodes = declared_nodes;
        // Make the byte caps visibly different from SDK defaults as well.
        declared.max_runtime_config_bytes = 0x0002_0000;
        declared.max_runtime_config_blob_bytes = 0x0002_0000;
        // Fund either cap so the pre-fix failure reaches extraction, not boot.
        declared.aux_route_peak_bytes = min_aux_route_peak_bytes(
            declared.max_runtime_config_bytes,
            declared.max_runtime_config_blob_bytes,
            u64::from(expected_nodes.max(DEFAULT_CONFIG_JSON_NODES)),
        );
        let target = target();
        fund_auxiliary_work(&mut declared, &target, expected_nodes);
        let catalog = auctioneer_runtime::CatalogResourceCosts::UNCHANGED;
        install_boot_envelope(&declared, &target, 4_384, construction_costs(), catalog).unwrap();
        let mut app = App::new(
            RouterService::builder()
                .get("/info/bidders", extract_nodes)
                .build(),
        );
        configure_app(&mut app).unwrap();
        let caps = app.config_extraction_limits();
        assert_eq!(caps.max_backend_bytes, declared.max_runtime_config_bytes);
        assert_eq!(caps.max_blob_bytes, declared.max_runtime_config_blob_bytes);

        let mut test_nodes = vec![
            expected_nodes.checked_sub(1).unwrap(),
            expected_nodes,
            expected_nodes.checked_add(1).unwrap(),
        ];
        test_nodes.extend([
            DEFAULT_CONFIG_JSON_NODES - 1,
            DEFAULT_CONFIG_JSON_NODES,
            DEFAULT_CONFIG_JSON_NODES + 1,
        ]);
        test_nodes.sort_unstable();
        test_nodes.dedup();
        for whole_envelope_nodes in test_nodes {
            // Root, data object, array and three envelope scalars count as six
            // values; JSON object keys do not consume the SDK's node budget.
            let array_len = usize::try_from(whole_envelope_nodes.checked_sub(6).unwrap()).unwrap();
            let blob = serde_json::to_string(&BlobEnvelope::new(
                json!({"nodes": vec![(); array_len]}),
                "2026-10-12T00:00:00Z".into(),
            ))
            .unwrap();
            assert!(u64::try_from(blob.len()).unwrap() < caps.max_backend_bytes);
            assert!(u64::try_from(blob.len()).unwrap() < caps.max_blob_bytes);
            let registry = StoreRegistry::single_id(
                STORE_ID.into(),
                ConfigStoreBinding {
                    default_key: STORE_ID.into(),
                    handle: ConfigStoreHandle::new(Arc::new(Store(blob.into()))),
                },
            );
            let mut request = http::Request::builder()
                .uri("/info/bidders")
                .body(Body::empty())
                .unwrap();
            request.extensions_mut().insert(registry);
            let IngressDispatchOutcome::Response(mut egress) = block_on(app.dispatch_ingress(
                request,
                MonotonicInstant::now(),
                IngressHeadAccounting::HostManaged,
                IngressFraming::HostManaged,
            ))
            .unwrap() else {
                panic!("the booted fixture has aux response capacity");
            };
            let response = egress.response_mut();
            assert_eq!(response.status(), http::StatusCode::OK);
            let expected = if whole_envelope_nodes <= expected_nodes {
                array_len.to_string()
            } else {
                "structural_limit".into()
            };
            assert_eq!(
                response.body().as_bytes().unwrap(),
                expected.as_bytes(),
                "declared cap={declared_nodes}, whole envelope nodes={whole_envelope_nodes}",
            );
        }
        assert_eq!(boot_limits().unwrap().max_config_json_nodes, expected_nodes);
        assert_eq!(caps.max_json_nodes, expected_nodes);
        assert_eq!(declared.max_config_json_nodes, declared_nodes);
    }

    fn target() -> TargetPlatformConstants {
        TargetPlatformConstants {
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
        }
    }
}
