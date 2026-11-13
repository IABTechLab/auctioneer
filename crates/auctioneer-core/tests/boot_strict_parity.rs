//! Real boot versus non-installing strict accounting in fresh processes.
//!
//! This is the ONLY test in its binary, and that is load-bearing rather
//! than tidiness. `BOOT_GATES`, `BAKED_CONFIG_BYTES`, `PROBE_EGRESS_GATE` and
//! `OVER_CEILING_WARNED` are process-global `OnceLock`s. Each installing row
//! must run in a fresh child, observe empty boot state, and install once.
//! Neither a mutex nor --test-threads=1 makes serial reuse independent.
//! The parent never installs, and a counted receipt proves each child ran.
//!
//! The CLI parity parent also invokes this same probe and compares its retained
//! receipt with real CLI output. There is no second installing test harness.

#[cfg(test)]
use auctioneer_core::test_support::boot_parity_fixture as fixture;
#[cfg(test)]
mod boot_parity_retained;
use boot_parity_retained as retained;

#[cfg(test)]
mod tests {
    use super::fixture::{CASE_ENV as CHILD_CASE_ENV, CASES, CATALOG};
    use super::{fixture, retained};
    use auctioneer_core::auction::audit::construction_costs;
    use auctioneer_core::configure::configure_app;
    use auctioneer_core::host_primitives::gates::boot_target;
    use auctioneer_core::host_primitives::{
        admission_gate, aux_gate, boot_gates, boot_limits, egress_gate, install_boot_envelope,
        resolve_budget_inputs, static_egress_gate, stream_gate,
    };
    use auctioneer_runtime::{
        BudgetError, Derived, ResolvedIsolateLimits, TargetPlatformConstants,
        derive_isolate_budget_with_baked_config, validate_isolate_limits,
    };
    use edgezero_core::{
        HostIngressMemoryAccounting, InboundRequestPopulationBound, MemoryCeiling,
        MemoryCeilingScope, MemoryEnvelope, MemoryEnvelopeValidation, MemoryResource, PlatformFact,
        PlatformMetadata, PlatformResourceSource,
    };
    use edgezero_core::{app::App, router::RouterService};
    use serde::{Deserialize, Serialize};
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};
    use std::cell::RefCell;
    use std::env::{current_exe, var_os};
    use std::io::Read;
    use std::num::NonZeroU32;
    use std::process::{Command, Stdio, id};
    use std::thread::{sleep, spawn};
    use std::time::{Duration, Instant};

    const RECEIPT_PREFIX: &str = "BOOT_PARITY_RECEIPT ";
    const TEST_NAME: &str = "tests::boot_retains_resolved_inputs_and_matches_strict_accounting";
    const CAPTURE_LIMIT: u64 = 0x8000;

    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct ChildReceipt {
        case: String,
        process_id: u32,
        installs: u32,
        retained: Value,
        captured_extraction: Value,
        warnings: Vec<String>,
    }

    fn verify_child_receipt(output: &str, case: &str, process_id: u32) -> Result<(), &'static str> {
        let mut receipts = output
            .lines()
            .filter_map(|line| line.strip_prefix(RECEIPT_PREFIX));
        let encoded = receipts.next().ok_or("missing child receipt")?;
        if receipts.next().is_some() {
            return Err("duplicate child receipt");
        }
        let receipt: ChildReceipt =
            serde_json::from_str(encoded).map_err(|_error| "invalid child receipt")?;
        if receipt.case != case {
            return Err("wrong child case");
        }
        if receipt.process_id != process_id {
            return Err("wrong child process");
        }
        if receipt.installs != 1 {
            return Err("child must install exactly once");
        }
        Ok(())
    }

    fn assert_receipt_protocol() {
        let receipt = serde_json::json!({
            "case": "default_backend_cap",
            "process_id": 17_u32,
            "installs": 1_u32,
            "retained": {},
            "captured_extraction": {
                "max_backend_bytes": 0x0100_0000_u64,
                "max_blob_bytes": 3571_u64,
                "max_json_depth": 64_u32,
                "max_json_nodes": 127_u32,
                "max_secret_bytes": 0x0010_0000_u64,
                "max_total_bytes": 0x0100_0000_u64,
                "timeout_ms": 2_000_u64,
            },
            "warnings": [],
        });
        let valid =
            format!("running 1 test\n{RECEIPT_PREFIX}{receipt}\ntest result: ok. 1 passed\n");
        assert_eq!(
            verify_child_receipt(&valid, "default_backend_cap", 17),
            Ok(())
        );
        assert_eq!(
            verify_child_receipt("test result: ok. 0 passed", "default_backend_cap", 17),
            Err("missing child receipt"),
        );
        assert_eq!(
            verify_child_receipt(&format!("{valid}{valid}"), "default_backend_cap", 17),
            Err("duplicate child receipt"),
        );
        assert_eq!(
            verify_child_receipt(&valid, "default_blob_cap", 17),
            Err("wrong child case"),
        );
        assert_eq!(
            verify_child_receipt(&valid, "default_backend_cap", 18),
            Err("wrong child process"),
        );
        for installs in [0_u32, 2] {
            let mut invalid = receipt.clone();
            invalid["installs"] = serde_json::json!(installs);
            assert_eq!(
                verify_child_receipt(
                    &format!("{RECEIPT_PREFIX}{invalid}\n"),
                    "default_backend_cap",
                    17,
                ),
                Err("child must install exactly once"),
            );
        }
        for field in ["retained", "captured_extraction", "warnings"] {
            let mut invalid = receipt.clone();
            assert!(invalid.as_object_mut().unwrap().remove(field).is_some());
            assert_eq!(
                verify_child_receipt(
                    &format!("{RECEIPT_PREFIX}{invalid}\n"),
                    "default_backend_cap",
                    17,
                ),
                Err("invalid child receipt"),
            );
        }
        for invalid in [
        "{".to_owned(),
        serde_json::json!({"case": "default_backend_cap", "process_id": 17_u32}).to_string(),
        serde_json::json!({
            "case": "default_backend_cap", "process_id": 17_u32, "installs": 1_u32, "extra": true,
        })
        .to_string(),
    ] {
        assert_eq!(
            verify_child_receipt(
                &format!("{RECEIPT_PREFIX}{invalid}\n"),
                "default_backend_cap",
                17,
            ),
            Err("invalid child receipt"),
        );
    }
    }

    fn assert_no_boot_gates() {
        assert!(boot_gates().is_none());
        assert!(boot_limits().is_none());
        assert!(boot_target().is_none());
        assert!(admission_gate().is_none());
        assert!(stream_gate().is_none());
        assert!(egress_gate().is_none());
        assert!(aux_gate().is_none());
        assert!(static_egress_gate().is_none());
    }

    fn target() -> TargetPlatformConstants {
        fixture::profile_for("unequal_explicit_caps").target
    }

    fn assert_strict_verdict(derived: &Derived, limits: &ResolvedIsolateLimits, over: bool) {
        // All-known synthetic facts exercise the real SDK consumer, not provider
        // qualification. The already population-scaled total is passed exactly once.
        let source = PlatformResourceSource::RuntimeConfiguration {
            runtime: "boot-parity-test-only",
        };
        let platform = PlatformMetadata::new(
            PlatformFact::known(
                MemoryCeiling::new(
                    limits.isolate_total_bytes,
                    MemoryCeilingScope::PerInstance,
                    None,
                ),
                source,
            ),
            PlatformFact::known(
                InboundRequestPopulationBound::new(NonZeroU32::new(384).unwrap()),
                source,
            ),
            PlatformFact::known(HostIngressMemoryAccounting::CountsTowardCeiling, source),
        );
        let verdict = platform
            .validate_memory_envelope(MemoryEnvelope::new(
                MemoryCeilingScope::PerInstance,
                derived.total_bytes,
                0,
                None,
            ))
            .unwrap();
        let expected = if over {
            MemoryEnvelopeValidation::Exceeds {
                available_bytes: limits.isolate_total_bytes,
                required_bytes: derived.total_bytes,
                resource: MemoryResource::Primary,
            }
        } else {
            MemoryEnvelopeValidation::Fits {
                available_primary_bytes: limits.isolate_total_bytes,
                available_separate_stack_bytes: None,
                required_primary_bytes: derived.total_bytes,
                required_separate_stack_bytes: None,
            }
        };
        assert_eq!(verdict, expected);
    }

    fn assert_warning(
        derived: &Derived,
        limits: &ResolvedIsolateLimits,
        over: bool,
    ) -> Vec<String> {
        let bodies = RefCell::new(Vec::new());
        testing_logger::validate(|captured| {
            let warnings: Vec<_> = captured
                .iter()
                .filter(|line| line.level == log::Level::Warn)
                .collect();
            if !over {
                assert!(warnings.is_empty(), "funded boot must not warn");
                return;
            }
            assert_eq!(warnings.len(), 4, "one headline and three dominant terms");
            assert!(
                warnings
                    .iter()
                    .all(|line| line.target == edgezero_core::BOOT_LOG_TARGET)
            );
            let headlines: Vec<_> = warnings
                .iter()
                .filter(|line| line.body.contains("Booting anyway"))
                .collect();
            assert_eq!(headlines.len(), 1);
            let headline = &headlines.first().unwrap().body;
            for expected in [
                format!("requires {} bytes", derived.total_bytes),
                format!("{}-byte platform ceiling", limits.isolate_total_bytes),
                format!(
                    "{} bytes over",
                    derived
                        .total_bytes
                        .checked_sub(limits.isolate_total_bytes)
                        .unwrap()
                ),
            ] {
                assert!(
                    headline.contains(&expected),
                    "missing {expected}: {headline}"
                );
            }
            assert_eq!(
                warnings
                    .iter()
                    .filter(|line| line.body.contains("charges"))
                    .count(),
                3
            );
            assert_eq!(derived.margin_bytes, 0);
            *bodies.borrow_mut() = warnings.iter().map(|line| line.body.clone()).collect();
        });
        bodies.into_inner()
    }

    #[expect(
        clippy::print_stdout,
        reason = "child diagnostics and receipt are captured by the parent test"
    )]
    fn run_boot_case(name: &str, zero_mask: u8, over: bool) {
        assert_no_boot_gates();
        testing_logger::setup();
        let profile = fixture::profile_for(name);
        let raw = profile.limits;
        let original = raw;
        // Independent default literals, not SDK constants shared with the resolver.
        let mut expected = raw;
        expected.max_runtime_config_bytes = if zero_mask & 1 == 0 {
            7919
        } else {
            0x0100_0000
        };
        expected.max_runtime_config_blob_bytes = if zero_mask & 2 == 0 {
            3571
        } else {
            0x0080_0000
        };
        expected.max_config_json_nodes = if zero_mask & 4 == 0 { 127 } else { 8192 };
        let serialized = serde_json::to_vec(&(
            auctioneer_config_data::host_default(),
            auctioneer_config_data::account_default(),
        ))
        .unwrap();
        let baked = u64::try_from(serialized.len()).unwrap();
        assert!(baked > 0);

        let inputs = resolve_budget_inputs(&raw).unwrap();
        assert_eq!(inputs.limits, expected, "{name}: effective caps");
        assert_eq!(inputs.baked_config_bytes, baked, "complete baked tuple");
        let target = target();
        let construction = construction_costs();
        let metadata = auctioneer_bidder::outbound_meta_bytes(raw.max_impressions_per_auction);
        assert_eq!(profile.outbound_meta_bytes, metadata);
        assert_eq!(profile.construction_costs, construction);
        let strict = derive_isolate_budget_with_baked_config(
            &inputs.limits,
            &target,
            metadata,
            construction,
            inputs.baked_config_bytes,
            CATALOG,
        )
        .unwrap();
        assert_baked_charge_and_strict_policy(&inputs.limits, baked, &strict, over);
        assert_no_boot_gates();
        testing_logger::validate(|captured| {
            assert!(captured.is_empty(), "pure consumers do not warn");
        });

        let mut installs = 0_u32;
        install_boot_envelope(&raw, &target, metadata, construction, CATALOG).unwrap();
        installs = installs.checked_add(1).unwrap();
        assert_eq!(raw, original, "boot does not mutate its caller's raw row");
        assert_retained_budget(&expected, &strict, baked);
        let captured_extraction = capture_extraction(&expected);
        let warnings = assert_warning(&strict, &expected, over);
        println!(
            "case={name} raw_caps=({},{},{}) effective_caps=({},{},{}) baked={baked} total={} over={over}",
            raw.max_runtime_config_bytes,
            raw.max_runtime_config_blob_bytes,
            raw.max_config_json_nodes,
            expected.max_runtime_config_bytes,
            expected.max_runtime_config_blob_bytes,
            expected.max_config_json_nodes,
            strict.total_bytes,
        );
        println!(
            "{RECEIPT_PREFIX}{}",
            serde_json::to_string(&ChildReceipt {
                case: name.to_owned(),
                process_id: id(),
                installs,
                retained: retained_receipt(&raw),
                captured_extraction,
                warnings,
            })
            .unwrap()
        );
    }

    fn capture_extraction(expected: &ResolvedIsolateLimits) -> Value {
        assert!(
            boot_gates().is_some(),
            "capture must follow boot installation"
        );
        let mut app = App::new(RouterService::builder().build());
        configure_app(&mut app).expect("the installed row configures the actual SDK App");
        let limits = app.config_extraction_limits();
        assert_eq!(limits.timeout, Duration::from_millis(2_000));
        let captured = json!({
            "max_backend_bytes": limits.max_backend_bytes,
            "max_blob_bytes": limits.max_blob_bytes,
            "max_json_depth": limits.max_json_depth,
            "max_json_nodes": limits.max_json_nodes,
            "max_secret_bytes": limits.max_secret_bytes,
            "max_total_bytes": limits.max_total_bytes,
            "timeout_ms": u64::try_from(limits.timeout.as_millis()).unwrap(),
        });
        assert_eq!(
            captured,
            json!({
                "max_backend_bytes": expected.max_runtime_config_bytes,
                "max_blob_bytes": expected.max_runtime_config_blob_bytes,
                "max_json_depth": 64_u32,
                "max_json_nodes": expected.max_config_json_nodes,
                "max_secret_bytes": 0x0010_0000_u64,
                "max_total_bytes": 0x0100_0000_u64,
                "timeout_ms": 2_000_u64,
            })
        );
        captured
    }

    fn retained_receipt(raw: &ResolvedIsolateLimits) -> Value {
        let gates = boot_gates().unwrap();
        let baked = serde_json::to_vec(&(
            auctioneer_config_data::host_default(),
            auctioneer_config_data::account_default(),
        ))
        .unwrap();
        json!({
            "raw_limits": retained::limits(raw),
            "effective_limits": retained::limits(&gates.limits),
            "target_constants": retained::target(&gates.target),
            "construction_costs": retained::construction(&gates.construction_costs),
            "catalog_costs": retained::catalog(&gates.catalog_costs),
            "outbound_meta_bytes": gates.outbound_meta_bytes,
            "baked_config_bytes": gates.baked_config_bytes,
            "baked_config_sha256": hex::encode(Sha256::digest(&baked)),
            "terms": retained::terms(&gates.derived.isolate_terms),
            "auction_receipts": retained::receipts(&gates.derived.auction_charges),
            "derived": retained::derived(&gates.derived),
        })
    }

    fn assert_baked_charge_and_strict_policy(
        limits: &ResolvedIsolateLimits,
        baked: u64,
        strict: &Derived,
        over: bool,
    ) {
        let target = target();
        let construction = construction_costs();
        let metadata = auctioneer_bidder::outbound_meta_bytes(limits.max_impressions_per_auction);
        // The legacy runtime strict facade has no baked-byte parameter. Keep its
        // policy check explicit; the SDK verdict uses the complete demand.
        let without_baked = derive_isolate_budget_with_baked_config(
            limits,
            &target,
            metadata,
            construction,
            0,
            CATALOG,
        )
        .unwrap();
        assert_eq!(
            strict
                .runtime_config_term
                .checked_sub(without_baked.runtime_config_term)
                .unwrap(),
            baked.checked_mul(128).unwrap()
        );
        assert_eq!(
            strict
                .total_bytes
                .checked_sub(without_baked.total_bytes)
                .unwrap(),
            baked
                .checked_mul(128)
                .unwrap()
                .checked_mul(u64::from(limits.max_concurrent_auctions))
                .unwrap()
        );
        let legacy_expected = if over {
            Err(BudgetError::BudgetOverCeiling {
                required: without_baked.total_bytes,
                ceiling: limits.isolate_total_bytes,
            })
        } else {
            Ok(without_baked)
        };
        assert_eq!(
            validate_isolate_limits(limits, &target, metadata, construction, CATALOG),
            legacy_expected,
        );
        assert_strict_verdict(strict, limits, over);
    }

    fn assert_retained_budget(expected: &ResolvedIsolateLimits, strict: &Derived, baked: u64) {
        let gates = boot_gates().expect("the real boot consumer installs gates");
        assert_eq!(gates.limits, *expected);
        assert_eq!(boot_limits(), Some(expected));
        assert_eq!(gates.target, target());
        assert_eq!(gates.construction_costs, construction_costs());
        assert_eq!(
            gates.outbound_meta_bytes,
            auctioneer_bidder::outbound_meta_bytes(expected.max_impressions_per_auction)
        );
        assert_eq!(gates.catalog_costs, CATALOG);
        assert_eq!(gates.baked_config_bytes, baked);
        assert_eq!(
            gates.derived, *strict,
            "all retained terms and receipts match"
        );
        assert_eq!(gates.rederive_budget(), Ok(*strict));
        assert_eq!(gates.derived.catalog_resident_term, 1009);
        assert_eq!(gates.derived.catalog_initialization_term, 2017);
        assert_eq!(gates.derived.schema_validation_term, 3011);
        assert_eq!(gates.derived.catalog_context_term, 4001);
        assert!(
            gates
                .derived
                .isolate_terms
                .entries()
                .iter()
                .all(|(_term, bytes)| *bytes > 0)
        );
        assert_eq!(
            gates.derived.isolate_terms.checked_total(),
            Ok(strict.total_bytes)
        );
        assert_eq!(
            gates.derived.auction_charges.total(),
            Ok(strict.per_auction_peak)
        );
        assert_eq!(gates.streams.capacity(), 7);
        assert_eq!(gates.egress.capacity(), 3);
        assert_eq!(gates.aux.capacity(), 5);
    }

    #[expect(
        clippy::print_stdout,
        reason = "relay verified child evidence into the parent test log"
    )]
    fn run_child(name: &str) {
        let mut child = Command::new(current_exe().unwrap())
            .env_clear()
            .env(CHILD_CASE_ENV, name)
            .args(["--exact", TEST_NAME, "--nocapture"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let process_id = child.id();
        let mut stdout_pipe = child.stdout.take().unwrap();
        let mut stderr_pipe = child.stderr.take().unwrap();
        let capture = |pipe: &mut dyn Read| {
            let mut bytes = Vec::new();
            pipe.take(CAPTURE_LIMIT + 1)
                .read_to_end(&mut bytes)
                .unwrap();
            bytes
        };
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
        assert!(
            u64::try_from(stdout_bytes.len()).unwrap() <= CAPTURE_LIMIT,
            "{name}: stdout limit"
        );
        assert!(
            u64::try_from(stderr_bytes.len()).unwrap() <= CAPTURE_LIMIT,
            "{name}: stderr limit"
        );
        let stdout = String::from_utf8(stdout_bytes).unwrap();
        let stderr = String::from_utf8(stderr_bytes).unwrap();
        assert!(status.is_some(), "{name}: boot exceeded 30 seconds");
        assert!(status.unwrap().success(), "{name}: {stdout}\n{stderr}");
        assert_eq!(
            verify_child_receipt(&stdout, name, process_id),
            Ok(()),
            "{stdout}"
        );
        print!("{stdout}");
    }

    fn run_children() -> usize {
        let mut children = 0_usize;
        for (name, _zero_mask, _over) in CASES {
            run_child(name);
            children = children.checked_add(1).unwrap();
            assert_no_boot_gates();
        }
        children
    }

    #[test]
    #[expect(
        clippy::print_stdout,
        reason = "publish the nonzero exact child invocation count"
    )]
    fn boot_retains_resolved_inputs_and_matches_strict_accounting() {
        assert_receipt_protocol();
        assert_no_boot_gates();
        if let Some(encoded_name) = var_os(CHILD_CASE_ENV) {
            let name = encoded_name.to_str().expect("UTF-8 child case");
            let (_name, zero_mask, over) = CASES
                .iter()
                .find(|(case, _, _)| *case == name)
                .expect("only fixed child cases are accepted");
            run_boot_case(name, *zero_mask, *over);
            return;
        }
        let children = run_children();
        assert_eq!(children, 10, "every installing row must execute in a child");
        assert_no_boot_gates();
        println!("boot parity child invocations: {children}; installs: {children}");
    }
}
