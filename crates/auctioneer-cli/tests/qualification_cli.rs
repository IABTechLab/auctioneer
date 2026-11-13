#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::{
        env::temp_dir,
        fs,
        process::{Output, id},
    };

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = temp_dir().join(format!(
                "auctioneer-qualification-{}-{}",
                id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
        fn json(&self, name: &str, value: &Value) -> PathBuf {
            let path = self.path(name);
            fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
            path
        }
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn validate(path: &Path) -> Output {
        Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .args(["qualify", "validate", "--profile"])
            .arg(path)
            .output()
            .unwrap()
    }

    fn evaluate(profile: &Path, observations: &Path, output: &Path) -> Output {
        Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .args(["qualify", "evaluate", "--profile"])
            .arg(profile)
            .arg("--observations")
            .arg(observations)
            .arg("--output")
            .arg(output)
            .env("QUALIFICATION_SECRET_CANARY", "SECRET_CANARY")
            .output()
            .unwrap()
    }

    fn fixture_scenarios(d: &str) -> Vec<Value> {
        let mut scenarios = Vec::new();
        for (family, subcases) in [
            (
                "baseline",
                vec!["cold_startup", "warm_baseline", "gvl_cold_parse"],
            ),
            ("maximum_admitted", vec!["maximum_shape", "fpd_expansion"]),
            (
                "decode_overlap",
                vec!["encoded_decoded", "slow_reader", "mixed_outcomes"],
            ),
            (
                "debug",
                vec![
                    "unrequested",
                    "denied",
                    "permitted",
                    "concurrent_capture",
                    "capture_release",
                ],
            ),
            (
                "auxiliary_config",
                vec![
                    "auxiliary_overlap",
                    "config_snapshot",
                    "gvl_miss_overlap",
                    "config_variant",
                    "config_restoration",
                ],
            ),
            (
                "saturation_recovery",
                vec![
                    "excess_admission",
                    "outbound_queue",
                    "stalled_io",
                    "cancellation",
                    "timeout",
                    "healthy_recovery",
                ],
            ),
        ] {
            for subcase in subcases {
                let mut effects = json!({"gvl_parses":0_i32,"fpd_expansions":0_i32,"decoded_responses":0_i32,"slow_reads":0_i32,
                "debug_captures":0_i32,"debug_denials":0_i32,"debug_output_bytes":0_i32,"auxiliary_requests":0_i32,"config_extractions":0_i32,
                "queued_calls":0_i32,"cancellations":0_i32,"stalled_reads":0_i32,"recovery_successes":0_i32});
                let key = match subcase {
                    "cold_startup" | "config_snapshot" | "config_variant"
                    | "config_restoration" => "config_extractions",
                    "gvl_cold_parse" | "gvl_miss_overlap" => "gvl_parses",
                    "fpd_expansion" => "fpd_expansions",
                    "encoded_decoded" | "mixed_outcomes" => "decoded_responses",
                    "slow_reader" => "slow_reads",
                    "denied" => "debug_denials",
                    "permitted" | "concurrent_capture" => "debug_captures",
                    "auxiliary_overlap" => "auxiliary_requests",
                    "outbound_queue" => "queued_calls",
                    "stalled_io" => "stalled_reads",
                    "cancellation" => "cancellations",
                    _ => "recovery_successes",
                };
                effects[key] = json!(if subcase == "gvl_miss_overlap" {
                    5_i32
                } else {
                    1_i32
                });
                if matches!(subcase, "permitted" | "concurrent_capture") {
                    effects["debug_output_bytes"] = json!(1_i32);
                }
                let rejected = u64::from(subcase == "excess_admission");
                let errors = u64::from(matches!(subcase, "mixed_outcomes" | "cancellation"));
                let timeouts = u64::from(matches!(subcase, "mixed_outcomes" | "timeout"));
                scenarios.push(json!({"family":family,"subcase":subcase,"variant_id":if subcase == "config_variant" {2_i32} else if subcase == "config_restoration" {3_i32} else {1_i32},
                "fixture":{"id":1_i32,"sha256":d},"seed":7_i32,"repetitions":1_i32,"warmup_ms":10_i32,"duration_ms":100_i32,"meter_window_ms":100_i32,
                "load":{"concurrency":1_i32,"calls":1_i32,"request_bytes":1_024_i32,"nodes":32_i32,"impressions":1_i32,"encoded_bytes":1_024_i32,"decoded_bytes":1_024_i32},
                "effects":effects,"outcomes":{"min_offered":10_u64.saturating_add(rejected),"min_admitted":10_i32,"min_success":10_u64.saturating_sub(errors).saturating_sub(timeouts),
                    "min_rejected":rejected,"min_errors":errors,"min_timeouts":timeouts,"max_rejected":rejected,"max_errors":errors,"max_timeouts":timeouts},
                "latency":{"start":"client_dispatch","end":"terminal_outcome","population":"all_offered","percentile":99_i32,"threshold_us":10_000_i32,"max_histogram_buckets":1_u8},
                "max_peak_bytes":100_000_000_i32,"max_retained_growth_bytes":1_024_i32,"maximum_mode":"simultaneous","justification_sha256":null}));
            }
        }
        scenarios
    }

    fn fixture() -> (Value, Value) {
        let d = "a".repeat(64);
        let version = json!({"major":1_i32,"minor":0_i32,"patch":0_i32});
        let memory = json!({"source":"process_rss","method":"high_water","units":"bytes","window_ms":100_i32,
        "peak_bytes":1_000_000_i32,"retained_growth_bytes":0_i32,"service_only":true,"limitation":"process_lifetime_high_water"});
        let scenarios = fixture_scenarios(&d);
        let variants: Vec<_> = (1_i32..=3_i32).map(|id| json!({"id":id,"config_sha256":if id == 2_i32 {"b".repeat(64)} else {d.clone()},
        "settings_sha256":if id == 2_i32 {"b".repeat(64)} else {d.clone()},"cold_window_ms":100_i32,"warm_window_ms":100_i32,
        "max_cold_peak_bytes":100_000_000_i32,"max_warm_peak_bytes":100_000_000_i32})).collect();
        let mut p = json!({"schema_version":2_i32,"scope":"deployment","identity":{"auctioneer_commit":"b".repeat(40),"artifact_sha256":d,
        "build_features":[],"edgezero_pin":"c".repeat(40),"target":"axum","runtime_version":version,"harness_sha256":d,
        "fixtures_sha256":d,"reference_data_sha256":d,"baked_config_sha256":d},
        "resources":{"limits":{"isolate_total_bytes":5_000_000_000_u64,"baseline_bytes":160_000_000_i32,
            "inbound_request_bytes":1_024_i32,"inbound_parse_factor":36_i32,"max_outbound_request_bytes":1_024_i32,"max_outbound_raw_bytes":1_024_i32,
            "max_transport_bytes":1_024_i32,"max_decoded_bytes":1_024_i32,"response_parse_factor":256_i32,"response_node_peak_bytes":320_i32,
            "final_response_bytes":3_072_i32,"egress_permits":1_i32,"response_write_deadline_ms":30_000_i32,"guest_decoder_bound_bytes":0_i32,
            "aux_route_permits":1_i32,"aux_route_peak_bytes":800_000_000_i32,"max_brotli_wbits":10_i32,"max_brotli_layers":1_i32,
            "gvl_cache_entries":4_i32,"gvl_parsed_bytes":512_000_i32,"max_bidders_per_auction":1_i32,"max_outbound_requests":1_i32,"max_concurrent_auctions":1_i32,
            "max_active_body_streams":1_i32,"max_runtime_config_bytes":0x0010_0000_i32,"max_runtime_config_blob_bytes":0x0010_0000_i32,"max_parsed_nodes":32_i32,
            "max_copied_header_bytes":4_096_i32,"max_admitted_header_bytes":0x4000_i32,"max_admitted_header_count":128_i32,"max_impressions_per_auction":1_i32,
            "max_config_json_nodes":8_192_i32,"terminal_response_permits":384_i32,"refusal_body_bytes":256_i32,"transmit_state_bytes":4_096_i32},
            "target_constants":{"platform_concurrent_ceiling":384_i32,"header_entry_overhead_bytes":64_i32,"parser_header_ceiling_bytes":21_632_i32,
                "parser_header_knob_bytes":21_633_i32,"max_request_target_bytes":768_i32,"h1_max_buf_size_bytes":24_576_i32,"h1_max_headers":128_i32,
                "parser_max_header_entries":676_i32,"pseudoheader_allowance_bytes":1_024_i32,"parser_transient_bytes":0x4000_i32,"egress_handoff_factor":2_i32},
            "construction_costs":{"fixed_per_auction_bytes":100_i32,"reusable_scratch_bytes":200_i32,"retained_per_scope_bytes":78_i32},
            "catalog_costs":{"resident_bytes":0_u64,"initialization_bytes":0_u64,
                "schema_validation_per_auction_bytes":0_u64,"context_per_auction_bytes":0_u64},
            "outbound_meta_bytes":4_384_i32,"baked_config_bytes":4_096_i32,"actual_gvl_cache_entries":4_i32,
            "sdk":{"ingress":{"max_connections":256_i32,"max_raw_head_bytes":0x4000_i32,"max_header_count":100_i32,"head_read_timeout_ms":10_000_i32},
                "config_store":{"max_entries":1_024_i32,"max_file_bytes":0x0100_0000_i32,"max_key_bytes":1_024_i32,"max_value_bytes":0x0080_0000_i32,
                    "max_resident_bytes":0x0200_0000_i32,"max_startup_bytes":0x0700_0000_i32},"http2_enabled":false}},
        "platform":{"memory_bytes":5_000_000_000_u64,"concurrent_requests":384_i32,"wall_time_ms":30_000_i32,"cpu_time_ms":30_000_i32},
        "hardware":{"architecture":"aarch64","cpu":"apple_m_series","logical_cores":8_i32,"memory_bytes":0x0002_0000_0000_u64,
            "page_bytes":0x4000_i32,"os":"macos","os_version":version,"meter":"process_rss","meter_version":version,"machine_sha256":d},
        "headroom_bytes":312_500_000_u64,"backend":"startup_snapshot","variants":variants,
        "required_capabilities":["ingress_admission","config_pre_extraction","transport_lifecycle","delivery_lifetime","deadline","cancellation"],
        "scenarios":scenarios});
        let rows: Vec<_> = scenarios.iter().map(|s| {
        let c = &s["outcomes"];
        json!({"subcase":s["subcase"],"variant_id":s["variant_id"],"repetition":1_i32,"fixture":s["fixture"],"seed":7_i32,
            "warmup_ms":10_i32,"duration_ms":100_i32,"load_window_ms":100_i32,"achieved":s["load"],"effects":s["effects"],"simultaneous":true,
            "outcomes":{"offered":c["min_offered"],"admitted":10_i32,"success":c["min_success"],"rejected":c["min_rejected"],"errors":c["min_errors"],"timeouts":c["min_timeouts"]},
            "latency":{"start":"client_dispatch","end":"terminal_outcome","population":"all_offered","percentile":99_i32,"samples":c["min_offered"],"value_us":1_000_i32,
                "histogram":[{"upper_us":1_000_u64,"count":c["min_offered"]}]},
            "memory":memory,"recovered":true})
    }).collect();
        let variant_rows: Vec<_> = variants.iter().enumerate().map(|(i, v)| json!({"id":v["id"],"process_id":i.saturating_add(1),
        "startup_sha256":format!("{:064x}", i.saturating_add(1)),"config_sha256":v["config_sha256"],"startup_snapshot_sha256":v["config_sha256"],
        "settings_sha256":v["settings_sha256"],"restarted":true,"input_immutable":true,"effective_settings_observed":true,
        "cold_window_ms":100_i32,"warm_window_ms":100_i32,"cold_memory":memory,"warm_memory":memory})).collect();
        let capabilities: Vec<_> = p["required_capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                json!({
        "capability":c,"status":"verified","evidence_sha256":d})
            })
            .collect();
        let mut o = json!({"schema_version":2_i32,"profile_sha256":hex::encode(Sha256::digest(serde_json::to_vec(&p).unwrap())),
        "identity":p["identity"],"resources":p["resources"],"platform":p["platform"],"hardware":p["hardware"],"backend":p["backend"],
        "variants":variant_rows,"capabilities":capabilities,"scenarios":rows});
        bind_debug(&mut p, &mut o);
        (p, o)
    }

    fn hash(value: &Value) -> String {
        hex::encode(Sha256::digest(serde_json::to_vec(value).unwrap()))
    }

    fn bind_debug(p: &mut Value, o: &mut Value) {
        let resources = hash(&p["resources"]);
        for (s, row) in p["scenarios"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(o["scenarios"].as_array_mut().unwrap())
        {
            s["debug_workload"] = Value::Null;
            row["debug"] = Value::Null;
            if !matches!(
                s["subcase"].as_str().unwrap(),
                "permitted" | "concurrent_capture" | "capture_release"
            ) {
                continue;
            }
            let reference = json!({"fixture":s["fixture"],"resources_sha256":resources,
                "review_sha256":"a".repeat(64),"limiting_factor":"fixture_shape",
                "limits":{"call_rows":1_u32,"input_bytes":1_024_u64,"body_nodes":32_u32,
                    "body_bytes":0x4000_u64,"capture_bytes":0x0001_0000_u64,"output_bytes":3_072_u64},
                "min_call_rows":1_u32,"max_call_rows":1_u32,"capture_bytes":{"min":512_u64,"max":512_u64},
                "output_bytes":{"min":1_024_u64,"max":1_024_u64}});
            let reference_digest = hash(&reference);
            s["debug_workload"] =
                json!({"reference_sha256":reference_digest,"reference":reference});
            s["effects"]["debug_captures"] = json!(1_u64);
            s["effects"]["debug_output_bytes"] = json!(1_024_u64);
            row["effects"]["debug_captures"] = json!(10_u64);
            row["effects"]["debug_output_bytes"] = json!(10_240_u64);
            row["debug"] = json!({"reference_sha256":reference_digest,"auctions":10_u64,"simultaneous_auctions":1_u32,
                "simultaneous_finalized_calls":1_u64,"min_call_rows":1_u32,"max_call_rows":1_u32,
                "capture_bytes":reference["capture_bytes"],"output_bytes":reference["output_bytes"]});
        }
        o["profile_sha256"] = json!(hash(p));
    }

    #[test]
    fn qualification_cli_validates_and_distinguishes_qualified_from_completed_pending() {
        let scratch = Scratch::new();
        let (p, mut o) = fixture();
        let profile = scratch.json("profile.json", &p);
        assert_eq!(validate(&profile).status.code(), Some(0_i32));
        let obs = scratch.json("observations.json", &o);
        let output = scratch.path("qualified.json");
        let result = evaluate(&profile, &obs, &output);
        assert_eq!(
            result.status.code(),
            Some(0_i32),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let bytes = fs::read(output).unwrap();
        assert!(bytes.len() <= 256 << 10_i32);
        assert!(!String::from_utf8_lossy(&bytes).contains("SECRET_CANARY"));
        let report: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(report["profile_qualification"], "qualified");
        o["capabilities"] = json!([]);
        let pending_obs = scratch.json("pending-observations.json", &o);
        let pending_output = scratch.path("pending.json");
        assert_eq!(
            evaluate(&profile, &pending_obs, &pending_output)
                .status
                .code(),
            Some(1_i32)
        );
        let pending_report: Value =
            serde_json::from_slice(&fs::read(pending_output).unwrap()).unwrap();
        assert_eq!(pending_report["local_acceptance"], "pass");
        assert_eq!(pending_report["profile_qualification"], "pending");
        assert_eq!(pending_report["publisher_pilot_ready"], false);
    }

    #[test]
    fn qualification_commands_enforce_headroom_before_publishing_a_report() {
        let scratch = Scratch::new();
        for (target, configured, platform, floor, accepted_exit) in [
            (
                "axum",
                4_000_000_000_u64,
                5_000_000_000_u64,
                0x1000_0000_u64,
                0_i32,
            ),
            ("cloudflare", 100_000_001, 100_000_017, 6_250_001, 1_i32),
            ("fastly", 100_000_001, 100_000_017, 6_250_001, 1_i32),
            ("spin", 100_000_001, 100_000_017, 6_250_001, 1_i32),
        ] {
            for headroom in [floor - 1, floor, floor + 1] {
                let (mut p, mut o) = fixture();
                p["identity"]["target"] = json!(target);
                p["resources"]["limits"]["isolate_total_bytes"] = json!(configured);
                p["platform"]["memory_bytes"] = json!(platform);
                p["headroom_bytes"] = json!(headroom);
                if target != "axum" {
                    p["hardware"]["architecture"] = json!("wasm32");
                    p["hardware"]["cpu"] = json!("provider_managed");
                    p["hardware"]["os"] = json!("provider");
                    p["hardware"]["meter"] = json!("wasm_guest");
                    for variant in o["variants"].as_array_mut().unwrap() {
                        for phase in ["cold_memory", "warm_memory"] {
                            variant[phase]["source"] = json!("wasm_guest");
                            variant[phase]["limitation"] = json!("guest_linear_memory_only");
                        }
                    }
                    for scenario in o["scenarios"].as_array_mut().unwrap() {
                        scenario["memory"]["source"] = json!("wasm_guest");
                        scenario["memory"]["limitation"] = json!("guest_linear_memory_only");
                    }
                }
                for field in ["identity", "resources", "platform", "hardware"] {
                    o[field] = p[field].clone();
                }
                bind_debug(&mut p, &mut o);
                let profile = scratch.json(&format!("{target}-{headroom}-profile.json"), &p);
                let observations =
                    scratch.json(&format!("{target}-{headroom}-observations.json"), &o);
                let report = scratch.path(&format!("{target}-{headroom}-report.json"));
                let validation = validate(&profile);
                let evaluated = evaluate(&profile, &observations, &report);
                if headroom < floor {
                    assert_eq!(validation.status.code(), Some(2_i32));
                    assert_eq!(evaluated.status.code(), Some(2_i32));
                    assert_eq!(validation.stderr, b"qualification_invalid_profile\n");
                    assert_eq!(evaluated.stderr, b"qualification_invalid_profile\n");
                    assert!(!report.exists());
                } else {
                    assert_eq!(validation.status.code(), Some(0_i32));
                    assert_eq!(evaluated.status.code(), Some(accepted_exit));
                    let result: Value = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
                    assert_eq!(result["accounted"]["required_headroom_bytes"], headroom);
                    assert_eq!(result["accounted"]["fits"], accepted_exit == 0_i32);
                    if accepted_exit != 0_i32 {
                        assert!(
                            result["reasons"]
                                .as_array()
                                .unwrap()
                                .contains(&json!("envelope_over_budget"))
                        );
                        assert_eq!(result["profile_qualification"], "nonqualified");
                        assert_eq!(result["publisher_pilot_ready"], false);
                    }
                }
            }
        }
    }

    #[test]
    fn qualification_cli_peak_threshold_controls_the_verdict_and_exit_code() {
        let scratch = Scratch::new();
        let (p, o) = fixture();
        let profile = scratch.json("profile.json", &p);
        let threshold = p["scenarios"][0]["max_peak_bytes"].as_u64().unwrap();
        for (index, peak, local, qualification, exit) in [
            (0_usize, threshold - 1, "pass", "qualified", 0_i32),
            (1, threshold, "pass", "qualified", 0_i32),
            (2, threshold + 1, "fail", "nonqualified", 1_i32),
            (3, 0, "fail", "nonqualified", 1_i32),
        ] {
            let mut observed = o.clone();
            // All lifetime samples agree; a refusal cannot come from monotonicity.
            for variant in observed["variants"].as_array_mut().unwrap() {
                for phase in ["cold_memory", "warm_memory"] {
                    variant[phase]["peak_bytes"] = json!(peak);
                }
            }
            for row in observed["scenarios"].as_array_mut().unwrap() {
                row["memory"]["peak_bytes"] = json!(peak);
            }
            let input = scratch.json(&format!("observations-{index}.json"), &observed);
            let output = scratch.path(&format!("report-{index}.json"));
            assert_eq!(
                evaluate(&profile, &input, &output).status.code(),
                Some(exit)
            );
            let report: Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
            assert_eq!(report["local_acceptance"], local, "peak {peak}");
            assert_eq!(
                report["profile_qualification"], qualification,
                "peak {peak}"
            );
            assert_eq!(
                report["publisher_pilot_ready"],
                exit == 0_i32,
                "peak {peak}"
            );
            assert_eq!(
                report["scenarios"][0]["reason"],
                if exit == 0_i32 {
                    "accepted"
                } else {
                    "peak_exceeded"
                },
                "peak {peak}"
            );
        }
    }

    #[test]
    fn qualification_cli_rejects_version_one_captures_without_creating_a_report() {
        let scratch = Scratch::new();
        let profile = scratch.path("profile.json");
        fs::write(
            &profile,
            include_bytes!(
                "../../../docs/testing/evidence/native-axum-rust-d68ba17-local-pass/profile.json"
            ),
        )
        .unwrap();
        let observations = scratch.path("observations.json");
        fs::write(&observations, include_bytes!("../../../docs/testing/evidence/native-axum-rust-d68ba17-local-pass/observations.json")).unwrap();
        let output = scratch.path("report.json");
        assert_eq!(validate(&profile).status.code(), Some(2_i32));
        assert_eq!(
            evaluate(&profile, &observations, &output).status.code(),
            Some(2_i32)
        );
        assert!(!output.exists());
    }

    fn synthetic_captured_inputs() -> (Value, Value) {
        // Reuse historical shapes as synthetic input; never rewrite captured evidence.
        let mut p: Value = serde_json::from_slice(include_bytes!(
            "../../../docs/testing/evidence/native-axum-rust-d68ba17-local-pass/profile.json"
        ))
        .unwrap();
        let mut o: Value = serde_json::from_slice(include_bytes!(
            "../../../docs/testing/evidence/native-axum-rust-d68ba17-local-pass/observations.json"
        ))
        .unwrap();
        assert_eq!(p["schema_version"], 1_u16);
        assert_eq!(o["schema_version"], 1_u16);
        let costs = json!({"resident_bytes":0_u64,"initialization_bytes":0_u64,
            "schema_validation_per_auction_bytes":0_u64,"context_per_auction_bytes":0_u64});
        p["schema_version"] = json!(2_u16);
        o["schema_version"] = json!(2_u16);
        p["resources"]["catalog_costs"] = costs.clone();
        o["resources"]["catalog_costs"] = costs;
        let resources = hash(&p["resources"]);
        for scenario in p["scenarios"].as_array_mut().unwrap() {
            if !scenario["debug_workload"].is_null() {
                scenario["debug_workload"]["reference"]["resources_sha256"] = json!(resources);
                let reference = hash(&scenario["debug_workload"]["reference"]);
                scenario["debug_workload"]["reference_sha256"] = json!(reference);
                let mut found = 0_usize;
                for row in o["scenarios"].as_array_mut().unwrap() {
                    if row["subcase"] == scenario["subcase"]
                        && row["variant_id"] == scenario["variant_id"]
                    {
                        row["debug"]["reference_sha256"] = json!(reference);
                        found = found.saturating_add(1);
                    }
                }
                assert_eq!(u64::try_from(found).unwrap(), scenario["repetitions"]);
            }
        }
        o["profile_sha256"] = json!(hash(&p));
        (p, o)
    }

    #[test]
    fn qualification_cli_rejects_slow_and_extra_synthetic_evidence() {
        let scratch = Scratch::new();
        let (p, baseline) = synthetic_captured_inputs();
        let profile = scratch.json("profile.json", &p);
        assert_eq!(baseline["scenarios"].as_array().unwrap().len(), 45);
        for case in ["baseline", "latency", "scenario", "variant"] {
            let mut o = baseline.clone();
            match case {
                "baseline" => {}
                "latency" => {
                    for row in o["scenarios"].as_array_mut().unwrap() {
                        row["latency"]["value_us"] = json!(9_000_000_u64);
                        row["latency"]["histogram"] =
                            json!([{"upper_us":9_000_000_u64,"count":row["latency"]["samples"]}]);
                    }
                }
                "scenario" => {
                    let mut extra = o["scenarios"][0].clone();
                    extra["repetition"] = json!(99_u16);
                    o["scenarios"].as_array_mut().unwrap().push(extra);
                }
                "variant" => {
                    let mut extra = o["variants"][0].clone();
                    extra["id"] = json!(99_u16);
                    extra["process_id"] = json!(99_u32);
                    extra["startup_sha256"] = json!("d".repeat(64));
                    o["variants"].as_array_mut().unwrap().push(extra);
                }
                _ => panic!("unknown evidence test case: {case}"),
            }
            let input = scratch.json(&format!("{case}.json"), &o);
            let output = scratch.path(&format!("{case}-report.json"));
            let result = evaluate(&profile, &input, &output);
            assert_eq!(
                result.status.code(),
                Some(1_i32),
                "{case}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            let report: Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
            assert_eq!(
                report["local_acceptance"],
                if case == "baseline" { "pass" } else { "fail" },
                "{case}"
            );
            assert_eq!(
                report["profile_qualification"],
                if case == "baseline" {
                    "unsupported"
                } else {
                    "nonqualified"
                },
                "{case}"
            );
            assert_eq!(report["publisher_pilot_ready"], false);
            assert_eq!(
                report["reasons"],
                if matches!(case, "scenario" | "variant") {
                    json!(["unexpected_evidence"])
                } else {
                    json!([])
                }
            );
            let rows = report["scenarios"].as_array().unwrap();
            assert_eq!(rows.len(), 45);
            for row in rows {
                assert_eq!(
                    row["reason"],
                    if case == "latency" {
                        "latency_exceeded"
                    } else {
                        "accepted"
                    },
                    "{case}"
                );
            }
        }
    }

    #[test]
    fn qualification_cli_requires_named_catalog_cost_objects() {
        let scratch = Scratch::new();
        for subject in ["profile", "observations"] {
            let (mut p, mut o) = fixture();
            let positional = json!([0_u64, 0_u64, 0_u64, 0_u64]);
            if subject == "profile" {
                p["resources"]["catalog_costs"] = positional;
                o["profile_sha256"] = json!(hash(&p));
            } else {
                o["resources"]["catalog_costs"] = positional;
            }
            let profile = scratch.json(&format!("{subject}-profile.json"), &p);
            let observations = scratch.json(&format!("{subject}-observations.json"), &o);
            let output = scratch.path(&format!("{subject}-report.json"));
            assert_eq!(
                evaluate(&profile, &observations, &output).status.code(),
                Some(2_i32)
            );
            assert!(!output.exists());
        }
    }

    #[test]
    fn qualification_cli_catalog_costs_bind_identity_accounting_and_evidence() {
        let scratch = Scratch::new();
        let (baseline, baseline_observations) = fixture();
        let baseline_profile = scratch.json("baseline-profile.json", &baseline);
        let baseline_input = scratch.json("baseline-observations.json", &baseline_observations);
        let baseline_output = scratch.path("baseline-report.json");
        assert_eq!(
            evaluate(&baseline_profile, &baseline_input, &baseline_output)
                .status
                .code(),
            Some(0_i32)
        );
        let baseline_report: Value =
            serde_json::from_slice(&fs::read(baseline_output).unwrap()).unwrap();
        for (field, term, bytes) in [
            ("resident_bytes", "catalog_resident_term", 101_u64),
            ("initialization_bytes", "catalog_initialization_term", 211),
            (
                "schema_validation_per_auction_bytes",
                "schema_validation_term",
                307,
            ),
            ("context_per_auction_bytes", "catalog_context_term", 401),
        ] {
            let (mut p, mut o) = fixture();
            p["resources"]["catalog_costs"][field] = json!(bytes);
            o["resources"] = p["resources"].clone();
            bind_debug(&mut p, &mut o);
            let profile = scratch.json(&format!("{field}-profile.json"), &p);
            assert_eq!(validate(&profile).status.code(), Some(0_i32));
            let observations = scratch.json(&format!("{field}-observations.json"), &o);
            let output = scratch.path(&format!("{field}-report.json"));
            assert_eq!(
                evaluate(&profile, &observations, &output).status.code(),
                Some(0_i32)
            );
            let report: Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
            assert_eq!(report["schema_version"], 2_u16);
            assert_eq!(report["profile"]["schema_version"], 2_u16);
            assert_eq!(report["observations"]["schema_version"], 2_u16);
            assert_eq!(
                report["profile"]["resources"]["catalog_costs"],
                p["resources"]["catalog_costs"]
            );
            assert_eq!(
                report["observations"]["resources"]["catalog_costs"],
                p["resources"]["catalog_costs"]
            );
            assert_eq!(report["accounted"][term], bytes);
            assert_eq!(
                report["accounted"]["total_bytes"].as_u64().unwrap(),
                baseline_report["accounted"]["total_bytes"]
                    .as_u64()
                    .unwrap()
                    + bytes
            );
            assert_ne!(
                report["accounted"]["effective_profile_sha256"],
                baseline_report["accounted"]["effective_profile_sha256"]
            );
            assert_ne!(report["profile_sha256"], baseline_report["profile_sha256"]);
            o["resources"]["catalog_costs"][field] = json!(0_u64);
            let stale_input = scratch.json(&format!("{field}-stale.json"), &o);
            let stale_output = scratch.path(&format!("{field}-stale-report.json"));
            assert_eq!(
                evaluate(&profile, &stale_input, &stale_output)
                    .status
                    .code(),
                Some(1_i32)
            );
            let refused: Value = serde_json::from_slice(&fs::read(stale_output).unwrap()).unwrap();
            assert_eq!(refused["profile_qualification"], "nonqualified");
            assert_eq!(refused["reasons"], json!(["identity_mismatch"]));
        }
    }

    #[test]
    fn qualification_cli_rejects_decreasing_process_lifetime_high_water() {
        let scratch = Scratch::new();
        let (mut p, mut o) = fixture();
        p["variants"][0]["max_cold_peak_bytes"] = json!(200_000_000_u64);
        o["profile_sha256"] = json!(hash(&p));
        o["variants"][0]["cold_memory"]["peak_bytes"] = json!(150_000_000_u64);
        let profile = scratch.json("profile.json", &p);
        let observations = scratch.json("observations.json", &o);
        let output = scratch.path("report.json");
        assert_eq!(
            evaluate(&profile, &observations, &output).status.code(),
            Some(1_i32)
        );
        let report: Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        assert_eq!(report["local_acceptance"], "fail");
        assert_eq!(report["profile_qualification"], "nonqualified");
        assert_eq!(report["publisher_pilot_ready"], false);
    }

    #[test]
    fn qualification_cli_reports_test_harness_override_alone_as_local_only() {
        let scratch = Scratch::new();
        let (mut p, mut o) = fixture();
        p["identity"]["build_features"] = json!(["test_harness_override"]);
        o["identity"] = p["identity"].clone();
        o["profile_sha256"] = json!(hash(&p));
        let profile = scratch.json("profile.json", &p);
        assert_eq!(validate(&profile).status.code(), Some(0_i32));
        let observations = scratch.json("observations.json", &o);
        let output = scratch.path("report.json");
        assert_eq!(
            evaluate(&profile, &observations, &output).status.code(),
            Some(1_i32)
        );
        let report: Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        assert_eq!(report["observations"]["identity"], p["identity"]);
        assert_eq!(report["local_acceptance"], "pass");
        assert_eq!(report["profile_qualification"], "unsupported");
        assert_eq!(report["publisher_pilot_ready"], false);
    }

    #[test]
    fn qualification_cli_writes_one_newline_terminated_json_record() {
        let scratch = Scratch::new();
        let (p, o) = fixture();
        let profile = scratch.json("profile.json", &p);
        let observations = scratch.json("observations.json", &o);
        let output = scratch.path("report.json");
        assert_eq!(
            evaluate(&profile, &observations, &output).status.code(),
            Some(0_i32)
        );
        let bytes = fs::read(output).unwrap();
        let report: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(report["profile_qualification"], "qualified");
        let record = bytes.strip_suffix(b"\n").unwrap();
        assert!(!record.contains(&b'\n'));
    }

    #[test]
    fn qualification_cli_caps_inputs_and_never_echoes_rejected_content() {
        let scratch = Scratch::new();
        let (p, _) = fixture();
        let profile = scratch.json("SECRET_CANARY.json", &p);
        let mut bytes = serde_json::to_vec(&p).unwrap();
        bytes.resize(1 << 20, b' ');
        fs::write(&profile, &bytes).unwrap();
        assert_eq!(validate(&profile).status.code(), Some(0_i32));
        bytes.push(b' ');
        for (rejected_bytes, diagnostic) in [
            (bytes, "qualification_input_limit"),
            (
                serde_json::to_vec(&vec![0_i32; 8192]).unwrap(),
                "qualification_node_limit",
            ),
            (
                b"{\"SECRET_CANARY\":\"SECRET_CANARY\"}".to_vec(),
                "qualification_invalid_json",
            ),
        ] {
            fs::write(&profile, rejected_bytes).unwrap();
            let result = validate(&profile);
            assert_eq!(result.status.code(), Some(2_i32));
            assert!(String::from_utf8_lossy(&result.stderr).contains(diagnostic));
            for stream in [&result.stdout, &result.stderr] {
                assert!(!String::from_utf8_lossy(stream).contains("SECRET_CANARY"));
                assert!(stream.len() < 1024);
            }
        }
    }

    #[test]
    fn qualification_cli_refuses_invalid_invocations_and_does_not_clobber_outputs() {
        let result = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .args(["qualify", "SECRET_CANARY"])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2_i32));
        assert_eq!(result.stderr, b"qualification_invalid_invocation\n");
        let scratch = Scratch::new();
        let (p, o) = fixture();
        let profile = scratch.json("profile.json", &p);
        let observations = scratch.json("observations.json", &o);
        let output = scratch.path("SECRET_CANARY");
        fs::write(&output, b"existing").unwrap();
        let output_refusal = evaluate(&profile, &observations, &output);
        assert_eq!(output_refusal.status.code(), Some(2_i32));
        assert_eq!(output_refusal.stderr, b"qualification_output_io\n");
        assert_eq!(fs::read(output).unwrap(), b"existing");
    }

    #[test]
    fn qualification_cli_refuses_schedules_that_cannot_export_complete_evidence() {
        let scratch = Scratch::new();
        let (mut p, o) = fixture();
        p["scenarios"][1]["repetitions"] = json!(41_u8);
        let profile = scratch.json("profile.json", &p);
        let observations = scratch.json("observations.json", &o);
        let output = scratch.path("report.json");
        let validation = validate(&profile);
        assert_eq!(validation.status.code(), Some(2_i32));
        assert_eq!(validation.stderr, b"qualification_profile_capacity\n");
        let evaluation = evaluate(&profile, &observations, &output);
        assert_eq!(evaluation.status.code(), Some(2_i32));
        assert_eq!(evaluation.stderr, b"qualification_profile_capacity\n");
        assert!(!output.exists());
    }

    #[test]
    fn qualification_help_lists_offline_commands() {
        let output = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .args(["qualify", "--help"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("validate"));
        assert!(stdout.contains("evaluate"));
    }

    #[test]
    fn qualification_input_failure_does_not_echo_secret_path() {
        let output = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .args(["qualify", "validate", "--profile", "/missing/SECRET_CANARY"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2_i32));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("SECRET_CANARY"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("SECRET_CANARY"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("qualification_input_io"));
    }
}
