//! Offline artifact inspection and native startup-snapshot behavior.

#[cfg(test)]
mod tests {
    use auctioneer_adapter_axum::envelope::resource_profile;
    use auctioneer_config_data::types::{BidderInfo, MediaType};
    use auctioneer_core::config::AuctioneerAppConfig;
    use auctioneer_core::host_primitives::{boot_gates, install_boot_envelope};
    use auctioneer_privacy::gvl::GVL_PARSE_CACHE_MAX_ENTRIES;
    use auctioneer_runtime::{
        Derived, ResolvedIsolateLimits, TargetPlatformConstants,
        derive_isolate_budget_with_baked_config,
    };
    use edgezero_adapter_axum::{
        config_store_limits::ConfigStoreLimits, ingress_config::AxumIngressConfig,
    };
    use edgezero_cli::args::ConfigPushArgs;
    use edgezero_core::blob_envelope::BlobEnvelope;
    use serde_json::value::RawValue;
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};
    use std::collections::BTreeMap;
    use std::env::{current_exe, temp_dir, var_os};
    use std::fs::{self, DirBuilder, File};
    use std::io::{Read as _, Write as _};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::path::PathBuf;
    use std::process::{Child, Command, ExitStatus, Stdio, id};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    const CANARY: &str = "private-resource-profile-canary-74191";
    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let path = temp_dir().join(format!(
                "auctioneer-resource-profile-{}-{}",
                id(),
                NEXT_DIR.fetch_add(1, Ordering::Relaxed)
            ));
            DirBuilder::new().create(&path).unwrap();
            Self(path)
        }

        fn command(&self) -> Command {
            let mut command = Command::new(env!("CARGO_BIN_EXE_auctioneer-adapter-axum"));
            command.current_dir(&self.0).env_clear();
            command
        }

        fn run(&self, command: &mut Command) -> (ExitStatus, Vec<u8>, Vec<u8>) {
            command
                .stdout(File::create(self.0.join("stdout")).unwrap())
                .stderr(File::create(self.0.join("stderr")).unwrap());
            let mut child = Process(command.spawn().unwrap());
            let deadline = Instant::now().checked_add(Duration::from_secs(10)).unwrap();
            let status = loop {
                if let Some(status) = child.0.try_wait().unwrap() {
                    break status;
                }
                assert!(Instant::now() < deadline, "offline command did not exit");
                for name in ["stdout", "stderr"] {
                    assert!(fs::metadata(self.0.join(name)).unwrap().len() <= 0x8000);
                }
                sleep(Duration::from_millis(10));
            };
            let stdout = fs::read(self.0.join("stdout")).unwrap();
            let stderr = fs::read(self.0.join("stderr")).unwrap();
            assert!(stdout.len() <= 0x8000);
            assert!(stderr.len() <= 0x8000);
            for output in [&stdout, &stderr] {
                assert!(!String::from_utf8_lossy(output).contains(CANARY));
            }
            (status, stdout, stderr)
        }

        fn profile(&self, settings: &[(&str, &str)]) -> Value {
            let mut command = self.command();
            command
                .arg("--resource-profile")
                .env("AUCTIONEER__RUNTIME__HEALTH_GVL_TOKEN", CANARY)
                .env("AUCTIONEER__RUNTIME__ALLOW_UNAUTHENTICATED_HEALTH", CANARY)
                .env("EDGEZERO__SECRETS__USERSYNC_COOKIE_KEYRING", CANARY)
                .env("EDGEZERO__LOGGING__LEVEL", "trace")
                .env("EDGEZERO__ADAPTER__HOST", CANARY)
                .env("EDGEZERO__ADAPTER__PORT", CANARY)
                .envs(settings.iter().copied());
            let (status, stdout, stderr) = self.run(&mut command);
            assert!(status.success(), "profile must exit before serving startup");
            assert!(
                stderr.is_empty(),
                "offline export must not initialize logging"
            );
            assert!(stdout.ends_with(b"\n"));
            serde_json::from_slice(&stdout).unwrap()
        }
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    struct Process(Child);

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for Process {
        fn drop(&mut self) {
            if self.0.try_wait().unwrap().is_none() {
                self.0.kill().unwrap();
            }
            self.0.wait().unwrap();
        }
    }

    macro_rules! exact_fields {
        ($value:expr, $model:expr, $($field:ident),+ $(,)?) => {{
            let fields = [$( (stringify!($field).to_owned(), json!($model.$field)) ),+];
            assert_eq!($value, &Value::Object(fields.into_iter().collect()));
        }};
    }

    #[test]
    fn declared_qualification_limits_match_the_actual_artifact() {
        let candidate: Value = serde_json::from_str(include_str!(
            "../../../docs/testing/fixtures/resource-qualification/native-axum.json"
        ))
        .unwrap();
        let expected = candidate["expected_limits"].as_object().unwrap();
        assert_eq!(expected.len(), 8);
        let profile = Fixture::new().profile(&[]);
        for (name, value) in expected {
            assert_eq!(
                &profile["resources"]["limits"][name], value,
                "declared qualification limit {name}"
            );
        }
    }

    #[test]
    fn offline_export_matches_every_bootstrap_term() {
        let profile = Fixture::new().profile(&[]);
        assert_eq!(profile["schema_version"], 2_u16);
        assert_eq!(profile["target"], "axum");
        assert_eq!(profile["backend"], "startup_snapshot");
        let resources = &profile["resources"];
        assert_eq!(resources.as_object().unwrap().len(), 8);
        let declared = resource_profile();
        let limits = declared.limits;
        let target = declared.target;
        let boot = declared.boot_envelope();
        install_boot_envelope(
            &limits,
            &target,
            boot.outbound_meta_bytes,
            boot.construction_costs,
            boot.catalog_costs,
        )
        .unwrap();
        let gates = boot_gates().unwrap();
        let expected = derive_isolate_budget_with_baked_config(
            &gates.limits,
            &declared.target,
            declared.outbound_meta_bytes,
            declared.construction_costs,
            gates.baked_config_bytes,
            declared.catalog_costs,
        )
        .unwrap();
        assert_eq!(gates.catalog_costs, declared.catalog_costs);
        assert_eq!(gates.derived, expected);
        assert_eq!(
            gates.derived.catalog_context_term,
            declared.catalog_costs.context_per_auction_bytes,
        );
        assert!(gates.derived.catalog_context_term > 0);
        assert_limits(&resources["limits"], &gates.limits);
        assert_target(&resources["target_constants"], &gates.target);
        exact_fields!(
            &resources["construction_costs"],
            gates.construction_costs,
            fixed_per_auction_bytes,
            reusable_scratch_bytes,
            retained_per_scope_bytes,
        );
        exact_fields!(
            &resources["catalog_costs"],
            gates.catalog_costs,
            resident_bytes,
            initialization_bytes,
            schema_validation_per_auction_bytes,
            context_per_auction_bytes,
        );
        assert_eq!(resources["outbound_meta_bytes"], gates.outbound_meta_bytes);
        assert_eq!(resources["baked_config_bytes"], gates.baked_config_bytes);
        assert_eq!(
            resources["actual_gvl_cache_entries"],
            GVL_PARSE_CACHE_MAX_ENTRIES
        );
        assert_terms(&profile["derived"], &gates.derived);
        assert!(gates.derived.debug_term > 0);
        assert_eq!(gates.rederive_budget().unwrap(), gates.derived);
        assert_eq!(resources["limits"]["isolate_total_bytes"], 4_u64 << 30_u32);
        assert_eq!(resources["actual_gvl_cache_entries"], 4_u32);
        assert_eq!(resources["limits"]["max_parsed_nodes"], 2_048_u32);
        assert_eq!(profile["derived"]["total_bytes"], expected.total_bytes);
        assert_eq!(
            profile["derived"]["margin_bytes"],
            gates
                .limits
                .isolate_total_bytes
                .checked_sub(expected.total_bytes)
                .unwrap(),
        );
        assert!(
            gates.derived.total_bytes + (256_u64 << 20_u32) <= gates.limits.isolate_total_bytes
        );
    }

    fn assert_limits(value: &Value, limits: &ResolvedIsolateLimits) {
        exact_fields!(
            value,
            limits,
            isolate_total_bytes,
            baseline_bytes,
            inbound_request_bytes,
            inbound_parse_factor,
            max_outbound_request_bytes,
            max_outbound_raw_bytes,
            max_transport_bytes,
            max_decoded_bytes,
            response_parse_factor,
            response_node_peak_bytes,
            final_response_bytes,
            egress_permits,
            response_write_deadline_ms,
            guest_decoder_bound_bytes,
            aux_route_permits,
            aux_route_peak_bytes,
            max_brotli_wbits,
            max_brotli_layers,
            gvl_cache_entries,
            gvl_parsed_bytes,
            max_bidders_per_auction,
            max_outbound_requests,
            max_concurrent_auctions,
            max_active_body_streams,
            max_runtime_config_bytes,
            max_runtime_config_blob_bytes,
            max_parsed_nodes,
            max_copied_header_bytes,
            max_admitted_header_bytes,
            max_admitted_header_count,
            max_impressions_per_auction,
            max_config_json_nodes,
            terminal_response_permits,
            refusal_body_bytes,
            transmit_state_bytes,
        );
    }

    fn assert_target(value: &Value, target: &TargetPlatformConstants) {
        exact_fields!(
            value,
            target,
            platform_concurrent_ceiling,
            header_entry_overhead_bytes,
            parser_header_ceiling_bytes,
            parser_header_knob_bytes,
            max_request_target_bytes,
            h1_max_buf_size_bytes,
            h1_max_headers,
            parser_max_header_entries,
            pseudoheader_allowance_bytes,
            parser_transient_bytes,
            egress_handoff_factor,
        );
    }

    fn assert_terms(value: &Value, expected: &Derived) {
        let mut derived = value.clone();
        let charges = derived
            .as_object_mut()
            .unwrap()
            .remove("auction_charges")
            .unwrap();
        let costs = derived
            .as_object_mut()
            .unwrap()
            .remove("construction_costs")
            .unwrap();
        exact_fields!(
            &costs,
            expected.construction_costs,
            fixed_per_auction_bytes,
            reusable_scratch_bytes,
            retained_per_scope_bytes
        );
        exact_fields!(
            &charges,
            expected.auction_charges,
            config,
            ingress,
            privacy,
            projection,
            outbound,
            response,
        );
        exact_fields!(
            &derived,
            expected,
            catalog_resident_term,
            catalog_initialization_term,
            schema_validation_term,
            catalog_context_term,
            runtime_config_term,
            request_side_term,
            call_side_term,
            bookkeeping_term,
            debug_term,
            assembly_scratch_bytes,
            per_auction_peak,
            per_stream_peak,
            decoder_chain_bytes,
            gvl_terms,
            egress_term,
            aux_term,
            terminal_term,
            probe_output_term,
            h2_slot_peak,
            h1_slot_peak,
            pre_admission_parser_term,
            total_bytes,
            margin_bytes,
            max_outbound_raw_bytes_usize,
        );
    }

    #[test]
    fn sdk_environment_changes_identity_without_changing_compiled_inputs() {
        let fixture = Fixture::new();
        let original = fixture.profile(&[]);
        let changed = fixture.profile(&[("EDGEZERO__ADAPTER__INGRESS__MAX_CONNECTIONS", "128")]);
        assert_ne!(
            original["effective_profile_sha256"],
            changed["effective_profile_sha256"]
        );
        let mut resources = original["resources"].clone();
        resources["sdk"]["ingress"]["max_connections"] = json!(128_u32);
        assert_eq!(resources, changed["resources"]);
        assert_eq!(original["derived"], changed["derived"]);
        for key in [
            "baked_host_sha256",
            "baked_account_sha256",
            "baked_config_sha256",
            "reference_data_sha256",
            "edgezero_pin",
            "build_features",
        ] {
            assert_eq!(original[key], changed[key]);
        }
        assert_eq!(original, fixture.profile(&[("UNRELATED_SECRET", CANARY)]));
    }

    #[test]
    fn sdk_settings_are_resolved_by_the_pinned_parsers() {
        let profile = Fixture::new().profile(&[]);
        let ingress = AxumIngressConfig::default();
        let store = ConfigStoreLimits::default();
        assert_eq!(
            profile["resources"]["sdk"],
            json!({
                "ingress": {
                    "max_connections": ingress.max_connections(),
                    "max_raw_head_bytes": ingress.max_raw_head_bytes(),
                    "max_header_count": ingress.max_header_count(),
                    "head_read_timeout_ms": ingress.head_read_timeout().as_millis(),
                },
                "config_store": {
                    "max_entries": store.max_entries(), "max_file_bytes": store.max_file_bytes(),
                    "max_key_bytes": store.max_key_bytes(), "max_value_bytes": store.max_value_bytes(),
                    "max_resident_bytes": store.max_resident_bytes(),
                    "max_startup_bytes": store.max_startup_bytes(),
                },
                "http2_enabled": false,
            })
        );
    }

    #[test]
    fn all_resolved_sdk_overrides_are_safe_and_identity_bearing() {
        let fixture = Fixture::new();
        let defaults = fixture.profile(&[]);
        for (key, value, section, field) in [
            (
                "EDGEZERO__ADAPTER__INGRESS__MAX_CONNECTIONS",
                "128",
                "ingress",
                "max_connections",
            ),
            (
                "EDGEZERO__ADAPTER__INGRESS__MAX_RAW_HEAD_BYTES",
                "32768",
                "ingress",
                "max_raw_head_bytes",
            ),
            (
                "EDGEZERO__ADAPTER__INGRESS__MAX_HEADER_COUNT",
                "64",
                "ingress",
                "max_header_count",
            ),
            (
                "EDGEZERO__ADAPTER__INGRESS__HEAD_READ_TIMEOUT_MS",
                "5000",
                "ingress",
                "head_read_timeout_ms",
            ),
            (
                "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_ENTRIES",
                "16",
                "config_store",
                "max_entries",
            ),
            (
                "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_FILE_BYTES",
                "8388608",
                "config_store",
                "max_file_bytes",
            ),
            (
                "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_KEY_BYTES",
                "512",
                "config_store",
                "max_key_bytes",
            ),
            (
                "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_VALUE_BYTES",
                "4194304",
                "config_store",
                "max_value_bytes",
            ),
            (
                "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_RESIDENT_BYTES",
                "16777216",
                "config_store",
                "max_resident_bytes",
            ),
            (
                "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_STARTUP_BYTES",
                "100000000",
                "config_store",
                "max_startup_bytes",
            ),
        ] {
            let changed = fixture.profile(&[(key, value)]);
            assert_eq!(
                changed["resources"]["sdk"][section][field],
                value.parse::<u64>().unwrap()
            );
            let mut expected = defaults["resources"].clone();
            expected["sdk"][section][field] = json!(value.parse::<u64>().unwrap());
            assert_eq!(changed["resources"], expected);
            assert_ne!(
                changed["effective_profile_sha256"],
                defaults["effective_profile_sha256"]
            );
        }
    }

    fn sha256(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    #[test]
    fn artifact_identity_binds_compiled_config_references_features_and_sdk_pin() {
        let profile = Fixture::new().profile(&[]);
        let mut keys: Vec<_> = profile
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "backend",
                "baked_account_sha256",
                "baked_config_sha256",
                "baked_host_sha256",
                "build_features",
                "derived",
                "edgezero_pin",
                "effective_profile_sha256",
                "reference_data_sha256",
                "resources",
                "schema_version",
                "target"
            ]
        );
        for key in [
            "baked_host_sha256",
            "baked_account_sha256",
            "baked_config_sha256",
            "reference_data_sha256",
            "effective_profile_sha256",
        ] {
            let digest = profile[key].as_str().unwrap();
            assert_eq!(digest.len(), 64);
            assert!(
                digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            );
        }
        let host = auctioneer_config_data::host_default();
        let account = auctioneer_config_data::account_default();
        assert_eq!(
            profile["baked_host_sha256"],
            sha256(&serde_json::to_vec(host).unwrap())
        );
        assert_eq!(
            profile["baked_account_sha256"],
            sha256(&serde_json::to_vec(account).unwrap())
        );
        assert_eq!(
            profile["baked_config_sha256"],
            sha256(&serde_json::to_vec(&(host, account)).unwrap())
        );
        let manifest: toml::Value = toml::from_str(include_str!("../../../Cargo.toml")).unwrap();
        for sdk in ["edgezero-core", "edgezero-adapter-axum"] {
            assert_eq!(
                profile["edgezero_pin"].as_str(),
                manifest["workspace"]["dependencies"][sdk]["rev"].as_str()
            );
        }
        let endpoint = "http://127.0.0.1:8999";
        let info = BidderInfo {
            endpoint: endpoint.into(),
            ..Default::default()
        };
        let mut features: Vec<_> =
            if auctioneer_exchange::validate_endpoint(endpoint, &info).is_ok() {
                vec!["insecure_bidders"]
            } else {
                Vec::new()
            };
        if auctioneer_core::CONFORMANCE_SUITE_ENABLED {
            features.push("conformance_suite");
        }
        if auctioneer_config_data::TEST_HARNESS_OVERRIDE_ENABLED {
            features.push("test_harness_override");
        }
        assert_eq!(profile["build_features"], json!(features));
    }

    #[test]
    fn profile_reports_the_linked_core_conformance_feature() {
        let profile = Fixture::new().profile(&[]);
        let features = profile["build_features"].as_array().unwrap();
        if auctioneer_core::CONFORMANCE_SUITE_ENABLED {
            assert!(features.contains(&json!("conformance_suite")));
        } else {
            assert!(!features.contains(&json!("conformance_suite")));
        }
    }

    #[test]
    fn config_override_probe_checks_explicit_expectations() {
        for (expected_state, should_succeed) in [
            (
                "enabled",
                auctioneer_config_data::TEST_HARNESS_OVERRIDE_ENABLED,
            ),
            (
                "disabled",
                !auctioneer_config_data::TEST_HARNESS_OVERRIDE_ENABLED,
            ),
            ("", false),
            ("true", false),
            ("false", false),
            (" enabled ", false),
        ] {
            let fixture = Fixture::new();
            let mut command = Command::new(current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "tests::profile_reports_the_linked_config_test_harness_override_feature",
                ])
                .env("AUCTIONEER__TEST__EXPECT_CONFIG_OVERRIDE", expected_state);
            let (status, stdout, stderr) = fixture.run(&mut command);
            let stdout_text = String::from_utf8_lossy(&stdout);
            assert!(stdout_text.contains("running 1 test"), "{stdout_text}");
            assert_eq!(
                status.success(),
                should_succeed,
                "expected state {expected_state:?}: {stdout_text}{}",
                String::from_utf8_lossy(&stderr)
            );
        }
    }

    #[test]
    fn profile_reports_the_linked_config_test_harness_override_feature() {
        let profile = Fixture::new().profile(&[]);
        let features = profile["build_features"].as_array().unwrap();
        let exported_override = features.contains(&json!("test_harness_override"));
        assert_eq!(
            exported_override,
            auctioneer_config_data::TEST_HARNESS_OVERRIDE_ENABLED
        );
        if let Some(expected_state) = var_os("AUCTIONEER__TEST__EXPECT_CONFIG_OVERRIDE") {
            let expected_override = match expected_state.to_str() {
                Some("enabled") => true,
                Some("disabled") => false,
                _ => panic!("AUCTIONEER__TEST__EXPECT_CONFIG_OVERRIDE must be enabled or disabled"),
            };
            assert_eq!(exported_override, expected_override);
        }
    }

    #[test]
    fn effective_identity_hashes_exactly_the_exported_resource_schema() {
        let fixture = Fixture::new();
        let (status, stdout, stderr) = fixture.run(fixture.command().arg("--resource-profile"));
        assert!(status.success());
        assert!(stderr.is_empty());
        let fields: BTreeMap<&str, &RawValue> = serde_json::from_slice(&stdout).unwrap();
        let identity: String =
            serde_json::from_str(fields["effective_profile_sha256"].get()).unwrap();
        let resources = fields["resources"].get();
        assert_eq!(identity, sha256(resources.as_bytes()));
        let mut previous = None;
        for field in [
            "limits",
            "target_constants",
            "construction_costs",
            "catalog_costs",
            "outbound_meta_bytes",
            "baked_config_bytes",
            "actual_gvl_cache_entries",
            "sdk",
        ] {
            let position = resources.find(&format!("\"{field}\":")).unwrap();
            assert!(previous.is_none_or(|prior| prior < position), "{field}");
            previous = Some(position);
        }
    }

    #[test]
    fn reference_identity_hashes_the_baked_archive_and_lookup_data() {
        let profile = Fixture::new().profile(&[]);
        let entries: Vec<_> = auctioneer_config_data::gvl_archive()
            .iter()
            .map(|entry| {
                (
                    entry.spec,
                    entry.vendor_list_version,
                    entry.tcf_policy_version,
                    entry.sha256,
                    entry.last_updated,
                )
            })
            .collect();
        let references = (
            entries,
            auctioneer_config_data::gvl_archive_generated_at(),
            auctioneer_config_data::gvl_archive_max_age_days(),
            auctioneer_config_data::gcl_expires_at_unix_secs(),
            auctioneer_config_data::baked_gcl_json(),
            auctioneer_config_data::eea_set(),
            auctioneer_config_data::bidders(),
        );
        assert_eq!(
            profile["reference_data_sha256"],
            sha256(&serde_json::to_vec(&references).unwrap())
        );
    }

    struct Server {
        process: Process,
        addr: SocketAddr,
    }

    impl Server {
        fn start(fixture: &Fixture) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            drop(listener);
            let mut command = fixture.command();
            command
                .env("EDGEZERO__ADAPTER__HOST", "127.0.0.1")
                .env("EDGEZERO__ADAPTER__PORT", addr.port().to_string())
                .env("EDGEZERO__LOGGING__LEVEL", "off")
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            let mut server = Self {
                process: Process(command.spawn().unwrap()),
                addr,
            };
            let deadline = Instant::now().checked_add(Duration::from_secs(10)).unwrap();
            loop {
                assert!(
                    server.process.0.try_wait().unwrap().is_none(),
                    "serving child exited before readiness"
                );
                if TcpStream::connect_timeout(&addr, Duration::from_millis(50)).is_ok() {
                    return server;
                }
                assert!(
                    Instant::now() < deadline,
                    "serving child readiness timed out"
                );
                sleep(Duration::from_millis(10));
            }
        }

        fn request(&self, path: &str, body: &[u8]) -> (u16, String) {
            let mut stream =
                TcpStream::connect_timeout(&self.addr, Duration::from_secs(2)).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let method = if body.is_empty() { "GET" } else { "POST" };
            write!(stream, "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len()).unwrap();
            stream.write_all(body).unwrap();
            let mut bytes = Vec::new();
            stream.take(0x8001).read_to_end(&mut bytes).unwrap();
            assert!(bytes.len() <= 0x8000);
            let response = String::from_utf8(bytes).unwrap();
            let (head, response_body) = response.split_once("\r\n\r\n").unwrap();
            let status = head.split_whitespace().nth(1).unwrap().parse().unwrap();
            if path == "/status" {
                assert!(
                    head.to_ascii_lowercase()
                        .contains("x-auctioneer-config-status: ok")
                );
            }
            (status, response_body.into())
        }
    }

    fn snapshot_bidder_name() -> &'static str {
        &auctioneer_config_data::bidders()
            .iter()
            .find(|info| !info.disabled && info.capabilities.site.contains(&MediaType::Banner))
            .expect("the serving catalog has an enabled site banner bidder")
            .name
    }

    fn push_snapshot(fixture: &Fixture, banner: &str, nodes: u32) -> String {
        let mut manifest: toml::Value =
            toml::from_str(include_str!("../../../edgezero.toml")).unwrap();
        manifest["adapters"]
            .as_table_mut()
            .unwrap()
            .retain(|name, _| name == "axum");
        let adapter = manifest["adapters"]["axum"]["adapter"]
            .as_table_mut()
            .unwrap();
        adapter.remove("manifest");
        fs::write(
            fixture.0.join("edgezero.toml"),
            toml::to_string(&manifest).unwrap(),
        )
        .unwrap();
        // Zero sampling preserves a valid auction while prohibiting outbound work.
        let config_value = json!({
            "status": {"banner": banner}, "auction": {"max_parsed_nodes": nodes},
            "bidders": {(snapshot_bidder_name()): {"sample_rate": 0.0_f64}},
        });
        let config: AuctioneerAppConfig = serde_json::from_value(config_value).unwrap();
        fs::write(
            fixture.0.join("auctioneer.toml"),
            toml::to_string(&config).unwrap(),
        )
        .unwrap();
        let mut command = Command::new(current_exe().unwrap());
        command
            .env_clear()
            .env("AUCTIONEER__TEST__PUSH_DIR", &fixture.0)
            .args([
                "--exact",
                "tests::native_store_keeps_startup_snapshot_until_restart",
            ]);
        assert!(
            fixture.run(&mut command).0.success(),
            "typed push child must succeed"
        );
        let store_bytes = fs::read(
            fixture
                .0
                .join(".edgezero/local-config-auctioneer_runtime.json"),
        )
        .unwrap();
        let store: Value = serde_json::from_slice(&store_bytes).unwrap();
        let blob: BlobEnvelope =
            serde_json::from_str(store["auctioneer_runtime"].as_str().unwrap()).unwrap();
        blob.verify().unwrap();
        assert_eq!(blob.data["status"]["banner"], banner);
        assert_eq!(blob.data["auction"]["max_parsed_nodes"], nodes);
        blob.sha256
    }

    fn node_request(nodes: usize) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "id": "snapshot", "site": {"page": "https://publisher.example"},
            "imp": [{"id": "one", "banner": {"w": 300, "h": 250},
                "ext": {"bidder": {(snapshot_bidder_name()): {}}}}],
            "ext": {"padding": vec![0_u8; nodes]},
        }))
        .unwrap()
    }

    #[test]
    fn native_store_keeps_startup_snapshot_until_restart() {
        if let Some(dir) = var_os("AUCTIONEER__TEST__PUSH_DIR") {
            let mut args = ConfigPushArgs::default();
            args.adapter = "axum".into();
            args.manifest = PathBuf::from(&dir).join("edgezero.toml");
            args.app_config = Some(PathBuf::from(&dir).join("auctioneer.toml"));
            args.local = true;
            args.yes = true;
            args.no_env = true;
            args.no_diff = true;
            edgezero_cli::run_config_push_typed::<AuctioneerAppConfig>(&args).unwrap();
            return;
        }
        let fixture = Fixture::new();
        let snapshot_a = push_snapshot(&fixture, "snapshot_a", 32);
        let server_a = Server::start(&fixture);
        assert_eq!(server_a.request("/status", b""), (200, "snapshot_a".into()));
        let body = node_request(64);
        assert_eq!(
            server_a.request("/openrtb2/auction", &body),
            (400, "Invalid request: too many JSON nodes\n".into())
        );
        let snapshot_b = push_snapshot(&fixture, "snapshot_b", 1_000_000);
        assert_ne!(snapshot_a, snapshot_b);
        assert_eq!(server_a.request("/status", b""), (200, "snapshot_a".into()));
        assert_eq!(server_a.request("/openrtb2/auction", &body).0, 400);
        drop(server_a);
        let server_b = Server::start(&fixture);
        assert_eq!(server_b.request("/status", b""), (200, "snapshot_b".into()));
        let admitted = server_b.request("/openrtb2/auction", &body);
        assert_eq!(admitted.0, 200, "{}", admitted.1);
        let boot_nodes = usize::try_from(resource_profile().limits.max_parsed_nodes).unwrap();
        let too_many = node_request(boot_nodes);
        assert!(
            too_many.len()
                < usize::try_from(resource_profile().limits.inbound_request_bytes).unwrap()
        );
        assert_eq!(
            server_b.request("/openrtb2/auction", &too_many),
            (400, "Invalid request: too many JSON nodes\n".into())
        );
    }

    #[test]
    fn unknown_arguments_fail_with_a_fixed_diagnostic() {
        let fixture = Fixture::new();
        for args in [
            vec![CANARY],
            vec!["--resource-profile", CANARY],
            vec!["--resource-profile", "--resource-profile"],
        ] {
            let (status, stdout, stderr) = fixture.run(fixture.command().args(args).env(
                "AUCTIONEER__RUNTIME__ALLOW_UNAUTHENTICATED_HEALTH",
                "invalid",
            ));
            assert!(!status.success());
            assert!(stdout.is_empty());
            assert_eq!(
                String::from_utf8(stderr).unwrap(),
                "Error: unsupported arguments\n"
            );
        }
    }

    #[test]
    fn invalid_or_unfunded_sdk_settings_are_refused_without_echoing_values() {
        let fixture = Fixture::new();
        for (key, value) in [
            ("EDGEZERO__ADAPTER__INGRESS__MAX_CONNECTIONS", "385"),
            ("EDGEZERO__ADAPTER__INGRESS__MAX_RAW_HEAD_BYTES", "65537"),
            ("EDGEZERO__ADAPTER__INGRESS__MAX_HEADER_COUNT", "129"),
            ("EDGEZERO__ADAPTER__INGRESS__HEAD_READ_TIMEOUT_MS", "10001"),
            ("EDGEZERO__ADAPTER__CONFIG_STORE__MAX_ENTRIES", "1025"),
            ("EDGEZERO__ADAPTER__CONFIG_STORE__MAX_KEY_BYTES", "1025"),
            (
                "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_VALUE_BYTES",
                "8388609",
            ),
            (
                "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_FILE_BYTES",
                "16777217",
            ),
            (
                "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_RESIDENT_BYTES",
                "33554433",
            ),
            (
                "EDGEZERO__ADAPTER__CONFIG_STORE__MAX_STARTUP_BYTES",
                "117440513",
            ),
            ("EDGEZERO__ADAPTER__INGRESS__MAX_CONNECTIONS", CANARY),
            ("EDGEZERO__ADAPTER__CONFIG_STORE__MAX_ENTRIES", CANARY),
        ] {
            let (status, stdout, stderr) =
                fixture.run(fixture.command().arg("--resource-profile").env(key, value));
            assert!(!status.success(), "{key} must be refused");
            assert!(stdout.is_empty());
            assert_eq!(stderr, b"Error: resource profile unavailable\n");
        }
    }
}
