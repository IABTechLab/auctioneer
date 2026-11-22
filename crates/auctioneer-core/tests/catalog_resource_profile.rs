//! Pure profile arithmetic. This binary must never install process-wide gates.

#[cfg(test)]
mod fanout_limits;
use fanout_limits as envelope;

#[cfg(test)]
mod tests {
    use super::envelope;
    use auctioneer_bidder::catalog::{
        CatalogBuildIdentity, CatalogCostError, CatalogOwner, CatalogRequirement,
        CatalogRequirements, aggregate_catalog_costs,
    };
    use auctioneer_core::auction::audit::construction_costs;
    use auctioneer_core::host_primitives::{boot_gates, resolve_budget_inputs};
    use auctioneer_runtime::budget::derive_aux_work_for_profile;
    use auctioneer_runtime::{
        AuctionCharges, BudgetError, CatalogResourceCosts, Derived, IsolateTerms,
        RequestConstructionCosts, ResolvedIsolateLimits, TargetPlatformConstants,
        derive_isolate_budget_with_baked_config, validate_isolate_limits,
    };
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};
    use std::{fs, panic::catch_unwind, path::Path};

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
            pseudoheader_allowance_bytes: 1024,
            parser_transient_bytes: 0x4000,
            egress_handoff_factor: 2,
        }
    }

    fn derive(
        raw: &ResolvedIsolateLimits,
        costs: CatalogResourceCosts,
    ) -> Result<Derived, BudgetError> {
        let inputs = resolve_budget_inputs(raw)?;
        derive_isolate_budget_with_baked_config(
            &inputs.limits,
            &target(),
            auctioneer_bidder::outbound_meta_bytes(raw.max_impressions_per_auction),
            construction_costs(),
            inputs.baked_config_bytes,
            costs,
        )
    }

    fn derive_raw(
        limits: &ResolvedIsolateLimits,
        platform: &TargetPlatformConstants,
        construction: RequestConstructionCosts,
        baked_config_bytes: u64,
        costs: CatalogResourceCosts,
    ) -> Derived {
        derive_isolate_budget_with_baked_config(
            limits,
            platform,
            auctioneer_bidder::outbound_meta_bytes(limits.max_impressions_per_auction),
            construction,
            baked_config_bytes,
            costs,
        )
        .unwrap()
    }

    fn captured_u64(value: &Value, field: &str) -> u64 {
        value
            .get(field)
            .and_then(Value::as_u64)
            .expect("captured unsigned integer")
    }

    fn captured_i64(value: &Value, field: &str) -> i64 {
        value
            .get(field)
            .and_then(Value::as_i64)
            .expect("captured signed integer")
    }

    fn captured_hypothetical(report: &Value) -> &Value {
        report
            .pointer("/candidate/hypothetical_requested_storage")
            .expect("captured candidate hypothetical requested storage")
    }

    fn assert_captured_projection(side: &Value) {
        let projection = side
            .get("source_recipe_projection")
            .expect("captured source recipe");
        assert_eq!(captured_u64(projection, "schema_version"), 1_u64);
        for (field, expected) in [
            ("kind", "schema_source_recipe_projection"),
            ("common_status", "SourceComplete"),
            ("target_status", "TargetIncomplete"),
        ] {
            assert_eq!(
                projection.get(field).and_then(Value::as_str),
                Some(expected)
            );
        }
        let identity = projection
            .get("source_recipe_identity")
            .and_then(Value::as_str)
            .expect("captured source recipe identity");
        assert_eq!(identity.len(), 64_usize);
        assert!(identity.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(projection.get("numeric_costs"), Some(&Value::Null));
        for field in [
            "authenticated_qualification_receipt",
            "physical_fit",
            "serving_permission",
        ] {
            assert_eq!(projection.get(field).and_then(Value::as_bool), Some(false));
        }
        let slots = projection
            .get("required_slots")
            .expect("captured target slots");
        let programs = slots
            .get("programs")
            .and_then(Value::as_array)
            .expect("captured program slots");
        let groups = slots
            .get("linked_inline_groups")
            .and_then(Value::as_array)
            .expect("captured linked/inline slots");
        assert!(!programs.is_empty());
        assert!(!groups.is_empty());
        assert_eq!(programs.len(), 5_usize);
        assert_eq!(groups.len(), 19_usize);
        assert_eq!(captured_u64(side, "program_count"), 5_u64);
    }

    fn assert_captured_observations(side: &Value) {
        let metadata = side
            .get("metadata_constructor_observation")
            .expect("captured metadata observation");
        let registry = side
            .get("registry_constructor_observation")
            .expect("captured registry observation");
        let zero = json!({
            "allocated_bytes": 0_u64,
            "peak_live_bytes": 0_i64,
            "retained_bytes": 0_i64,
            "allocation_calls": 0_u64
        });
        // Observations remain data, never inputs to the physical catalog charge.
        for observation in [metadata, registry] {
            let cold = observation.get("cold").expect("captured cold observation");
            for field in [
                "allocated_bytes",
                "peak_live_bytes",
                "retained_bytes",
                "allocation_calls",
            ] {
                assert!(captured_u64(cold, field) > 0_u64);
            }
        }
        assert_eq!(metadata.get("repeat"), Some(&zero));
        assert_eq!(registry.get("lookup"), Some(&zero));
        let registry_cold = registry.get("cold").expect("captured cold registry");
        let release = registry.get("release").expect("captured registry release");
        assert_eq!(
            captured_i64(registry_cold, "retained_bytes")
                .checked_add(captured_i64(release, "retained_bytes")),
            Some(0_i64)
        );
        let metadata_cold = metadata.get("cold").expect("captured cold metadata");
        let prefix = captured_i64(registry, "metadata_prefix_retained_bytes");
        assert_eq!(prefix, captured_i64(metadata_cold, "retained_bytes"));
        assert_eq!(
            captured_i64(registry, "prefix_plus_registry_peak_bytes"),
            prefix
                .checked_add(captured_i64(registry_cold, "peak_live_bytes"))
                .unwrap()
        );
    }

    fn assert_captured_work_units(side: &Value) {
        let work = side
            .get("constructor_work")
            .expect("captured constructor-work semantics");
        assert_eq!(
            work.get("units").and_then(Value::as_str),
            Some("source collection-record/byte visits")
        );
        assert_eq!(
            work.get("bound_kind").and_then(Value::as_str),
            Some("checked upper bound, not an exact dynamic execution count")
        );
        assert_eq!(
            work.get("excludes"),
            Some(&json!([
                "scalar moves",
                "atomics",
                "allocator internals",
                "machine operations",
                "elapsed time"
            ]))
        );
        for field in [
            "latency_bound",
            "cpu_instruction_bound",
            "runtime_validation_token_bound",
        ] {
            assert_eq!(work.get(field).and_then(Value::as_bool), Some(false));
        }
    }

    fn assert_captured_hypothetical(hypothetical: &Value) {
        assert_eq!(
            hypothetical.get("abi").and_then(Value::as_str),
            Some("hypothetical-only-not-observed-or-physical")
        );
        for field in [
            "observed_target_layouts",
            "physical_or_serving_qualification",
        ] {
            assert_eq!(
                hypothetical.get(field).and_then(Value::as_bool),
                Some(false)
            );
        }
        let resident = captured_u64(hypothetical, "retained_requested_bytes");
        let initialization = captured_u64(hypothetical, "construction_only_requested_bytes");
        assert!(resident > 0_u64 && initialization > 0_u64);
        let additive = resident.checked_add(initialization).unwrap();
        assert_eq!(
            captured_u64(hypothetical, "additive_requested_envelope"),
            additive
        );
        assert!(additive > resident.max(initialization));
        assert!(captured_u64(hypothetical, "linked_inline_attributed_bytes") > 0_u64);
        assert!(captured_u64(hypothetical, "constructor_source_visits") > 0_u64);
        let traces = hypothetical
            .get("trace_peaks")
            .and_then(Value::as_array)
            .expect("captured constructor traces");
        assert!(!traces.is_empty());
        assert_eq!(traces.len(), 65_usize);
        assert!(traces.iter().all(|trace| {
            let bytes = trace.as_u64().expect("captured trace bytes");
            bytes > 0_u64 && bytes <= additive
        }));
    }

    fn assert_captured_source_context(report: &Value, lane: &str, recorded_measurement_lock: &str) {
        assert_eq!(recorded_measurement_lock.len(), 64_usize);
        assert_eq!(
            report.get("metadata_registry_scope"),
            Some(&json!({
                "reference":"equal-input-proxy-of-candidate",
                "candidate":"actual-current-constructor",
                "supports_nonzero_metadata_registry_delta":false
            }))
        );
        assert!(report.get("task8_complete").is_none());
        for (side, kind) in [
            ("reference", "equal-input-proxy-of-candidate"),
            ("candidate", "actual-current-constructor"),
        ] {
            assert_eq!(
                report
                    .get(side)
                    .and_then(|value| value.get("metadata_registry_observation_kind"))
                    .and_then(Value::as_str),
                Some(kind)
            );
        }
        let historical_lock = "78118dcb53fd3f486a6032c47124ba249bd5fbf361a4a1973824e56d4f55001c";
        assert_eq!(
            report
                .pointer("/reference/source/lock_sha256")
                .and_then(Value::as_str),
            Some(historical_lock)
        );
        let current_lock = hex::encode(Sha256::digest(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../Cargo.lock"
        ))));
        assert_ne!(current_lock, historical_lock);
        for pointer in [
            "/candidate/source/lock_sha256",
            "/reference/measurement/measurement_lock_sha256",
            "/candidate/measurement/measurement_lock_sha256",
        ] {
            assert_eq!(
                report.pointer(pointer).and_then(Value::as_str),
                Some(current_lock.as_str()),
                "{lane}: {pointer}"
            );
        }
        let measurement = report
            .pointer("/reference/measurement")
            .expect("captured current measurement tuple");
        assert_eq!(report.pointer("/candidate/measurement"), Some(measurement));
        let features = measurement
            .pointer("/build/enabled_features")
            .and_then(Value::as_array)
            .expect("captured measurement features");
        assert_eq!(
            features
                .iter()
                .any(|feature| feature.as_str() == Some("insecure-bidders")),
            lane == "development"
        );
        assert_eq!(
            report
                .pointer("/matched_metadata_measurement/development")
                .and_then(Value::as_bool),
            Some(lane == "development")
        );
    }

    // Synthetic context-only records, not allocation samples or qualification claims.
    fn captured_source_context_fixture(lane: &str, recorded_measurement_lock: &str) -> Value {
        let features = if lane == "development" {
            json!(["test-support", "insecure-bidders"])
        } else {
            json!(["test-support"])
        };
        let measurement = json!({
            "measurement_lock_sha256":recorded_measurement_lock,
            "build":{"enabled_features":features}
        });
        json!({
            "metadata_registry_scope":{
                "reference":"equal-input-proxy-of-candidate",
                "candidate":"actual-current-constructor",
                "supports_nonzero_metadata_registry_delta":false
            },
            "reference":{
                "metadata_registry_observation_kind":"equal-input-proxy-of-candidate",
                "source":{"lock_sha256":
                    "78118dcb53fd3f486a6032c47124ba249bd5fbf361a4a1973824e56d4f55001c"},
                "measurement":measurement
            },
            "candidate":{
                "metadata_registry_observation_kind":"actual-current-constructor",
                "source":{"lock_sha256":recorded_measurement_lock},
                "measurement":measurement
            },
            "matched_metadata_measurement":{"development":lane == "development"}
        })
    }

    #[test]
    fn captured_context_accepts_independent_recorded_epochs_in_both_feature_lanes() {
        assert!(boot_gates().is_none());
        let mut checked = 0_u32;
        for recorded_measurement_lock in [
            "ae849e777f8216f983112f631168bf585c4604076381d265459249d62f88df38".to_owned(),
            "f".repeat(64_usize),
        ] {
            for lane in ["default", "development"] {
                let report = captured_source_context_fixture(lane, &recorded_measurement_lock);
                assert_captured_source_context(&report, lane, &recorded_measurement_lock);
                checked = checked.checked_add(1_u32).unwrap();
            }
        }
        assert_eq!(checked, 4_u32);
        assert!(boot_gates().is_none());
    }

    #[test]
    fn captured_context_refuses_each_changed_lock_and_coherently_wrong_measurement() {
        assert!(boot_gates().is_none());
        let recorded_measurement_lock =
            "ae849e777f8216f983112f631168bf585c4604076381d265459249d62f88df38";
        let cases: [(&[&str], &str); 5_usize] = [
            (
                &["/reference/source/lock_sha256"],
                "78118dcb53fd3f486a6032c47124ba249bd5fbf361a4a1973824e56d4f55001c",
            ),
            (&["/candidate/source/lock_sha256"], recorded_measurement_lock),
            (
                &["/reference/measurement/measurement_lock_sha256"],
                recorded_measurement_lock,
            ),
            (
                &["/candidate/measurement/measurement_lock_sha256"],
                recorded_measurement_lock,
            ),
            (
                &[
                    "/candidate/source/lock_sha256",
                    "/reference/measurement/measurement_lock_sha256",
                    "/candidate/measurement/measurement_lock_sha256",
                ],
                recorded_measurement_lock,
            ),
        ];
        let mut checked = 0_u32;
        for lane in ["default", "development"] {
            for (pointers, expected) in cases {
                let mut report = captured_source_context_fixture(lane, recorded_measurement_lock);
                assert_captured_source_context(&report, lane, recorded_measurement_lock);
                for pointer in pointers {
                    let leaf = report.pointer_mut(pointer).expect("before-state lock leaf");
                    assert_eq!(leaf.as_str(), Some(expected), "{lane}: {pointer}");
                    *leaf = json!("0".repeat(64_usize));
                }
                if pointers.len() == 3_usize {
                    assert_eq!(
                        report.pointer("/reference/measurement"),
                        report.pointer("/candidate/measurement")
                    );
                }
                let rejection = catch_unwind(|| {
                    assert_captured_source_context(&report, lane, recorded_measurement_lock);
                })
                .expect_err("changed recorded context must refuse");
                drop(rejection);
                checked = checked.checked_add(1_u32).unwrap();
            }
        }
        assert_eq!(checked, 10_u32);
        assert!(boot_gates().is_none());
    }

    fn captured_growth(lane: &str) -> Value {
        assert!(boot_gates().is_none());
        // Frozen proof checkpoint 4a863c6, run 45021426271, job 137204983421.
        // Both feature lanes retain their exact hosted bytes and provenance.
        let (filename, length, sha256): (&str, usize, &str) = match lane {
            "default" => (
                "hosted-growth-4a863c6-default.json",
                1_624_271_usize,
                "71d1be5f96ef8ac6cb3f756cb13ff26efd06b57dd3f6a44a7e23c9beddba2a51",
            ),
            "development" => (
                "hosted-growth-4a863c6-development.json",
                1_624_548_usize,
                "87da24f9e0a9644384ffe1f1c9bcb2eb2df5d3e52401d9937eeba897430d1f13",
            ),
            _ => panic!("unexpected capture lane"),
        };
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../auctioneer-bidder/tests/conformance/fixtures/reference/schema-residency")
            .join(filename);
        let bytes = fs::read(path).expect("immutable hosted matched-growth capture");
        assert_eq!(bytes.len(), length);
        assert_eq!(hex::encode(Sha256::digest(&bytes)), sha256);
        let report: Value = serde_json::from_slice(&bytes).expect("hosted matched-growth capture");
        assert_eq!(
            report.get("label").and_then(Value::as_str),
            Some("same-build data-only source reconstruction relative to d26fcf7")
        );
        for (field, expected) in [
            ("common_C_I_qualified", false),
            ("physical_or_serving_qualification", false),
            ("matched_reconstruction_complete", true),
            ("frozen_sources_and_snapshots_rechecked", true),
        ] {
            assert_eq!(report.get(field).and_then(Value::as_bool), Some(expected));
        }
        // Recorded measurement context of these byte-pinned frames, not this checkout.
        assert_captured_source_context(
            &report,
            lane,
            "ae849e777f8216f983112f631168bf585c4604076381d265459249d62f88df38",
        );
        for name in ["reference", "candidate"] {
            let side = report.get(name).expect("captured source side");
            assert_captured_projection(side);
            assert_captured_observations(side);
            assert_captured_work_units(side);
        }
        assert_captured_hypothetical(captured_hypothetical(&report));
        assert!(boot_gates().is_none());
        report
    }

    fn assert_captured_c_i_delta(
        baseline: &Derived,
        evaluated: &Derived,
        resident: u64,
        initialization: u64,
    ) {
        assert_accounting_delta(
            "captured hypothetical C/I only",
            baseline,
            evaluated,
            [
                0_u64,
                0_u64,
                0_u64,
                0_u64,
                0_u64,
                0_u64,
                0_u64,
                0_u64,
                0_u64,
                resident,
                initialization,
            ],
            [0_u64; 6_usize],
            resident.checked_add(initialization).unwrap(),
        );
        assert_eq!(evaluated.catalog_resident_term, resident);
        assert_eq!(evaluated.catalog_initialization_term, initialization);
        assert_eq!(
            evaluated.schema_validation_term,
            baseline.schema_validation_term
        );
        assert_eq!(
            evaluated.catalog_context_term,
            baseline.catalog_context_term
        );
    }

    fn captured_c_i_checks(report: &Value) -> u32 {
        let hypothetical = captured_hypothetical(report);
        let resident = captured_u64(hypothetical, "retained_requested_bytes");
        let initialization = captured_u64(hypothetical, "construction_only_requested_bytes");
        let unchanged = CatalogResourceCosts::UNCHANGED;
        assert_eq!(
            unchanged,
            CatalogResourceCosts {
                resident_bytes: 0_u64,
                initialization_bytes: 0_u64,
                schema_validation_per_auction_bytes: 0_u64,
                context_per_auction_bytes: 0_u64,
            }
        );
        let mut checked = 0_u32;
        for auctions in [1_u32, 3_u32, 7_u32] {
            for auxiliary in [0_u32, 1_u32, 5_u32] {
                let mut raw = envelope::limits();
                raw.max_concurrent_auctions = auctions;
                raw.aux_route_permits = auxiliary;
                let inputs = resolve_budget_inputs(&raw).unwrap();
                let baseline = derive_raw(
                    &inputs.limits,
                    &target(),
                    construction_costs(),
                    inputs.baked_config_bytes,
                    unchanged,
                );
                // Only hypothetical C/I enters this pure equation, never
                // linked/inline attribution or observed allocator samples.
                for (resident_term, initialization_term) in [
                    (resident, 0_u64),
                    (0_u64, initialization),
                    (resident, initialization),
                ] {
                    let costs = CatalogResourceCosts {
                        resident_bytes: resident_term,
                        initialization_bytes: initialization_term,
                        ..unchanged
                    };
                    let evaluated = derive_raw(
                        &inputs.limits,
                        &target(),
                        construction_costs(),
                        inputs.baked_config_bytes,
                        costs,
                    );
                    assert_captured_c_i_delta(
                        &baseline,
                        &evaluated,
                        resident_term,
                        initialization_term,
                    );
                    assert!(boot_gates().is_none());
                    checked = checked.checked_add(1_u32).unwrap();
                }
            }
        }
        assert_eq!(checked, 27_u32);
        assert_eq!(CatalogResourceCosts::UNCHANGED, unchanged);
        assert!(boot_gates().is_none());
        checked
    }

    #[test]
    fn captured_hypothetical_c_i_add_once_independently_of_auction_and_auxiliary_scale() {
        let mut checked = 0_u32;
        for report in [captured_growth("default"), captured_growth("development")] {
            checked = checked.checked_add(captured_c_i_checks(&report)).unwrap();
        }
        assert_eq!(checked, 54_u32);
        assert!(boot_gates().is_none());
    }

    fn assert_captured_no_receipt(report: &Value) {
        let hypothetical = captured_hypothetical(report);
        // Synthetic nonzero tags only reach the receipt-presence check. They
        // are not a parsed or authenticated identity of the captured build.
        let control_build = CatalogBuildIdentity {
            target_abi: hypothetical.get("abi").and_then(Value::as_str).unwrap(),
            features_sha256: [1_u8; 32_usize],
            toolchain_sha256: [2_u8; 32_usize],
            lock_sha256: [3_u8; 32_usize],
        };
        let no_receipt = CatalogRequirement {
            source_sha256: [4_u8; 32_usize],
            qualification_sha256: [0_u8; 32_usize],
        };
        let requirements = CatalogRequirements {
            build: control_build,
            metadata: no_receipt,
            validators: no_receipt,
            registry: no_receipt,
            context: no_receipt,
        };
        assert_eq!(
            aggregate_catalog_costs(&requirements, &[]),
            Err(CatalogCostError::InvalidRequirement(CatalogOwner::Metadata))
        );
        // Aggregation checks scalar identities, not cryptographic provenance.
        // No qualified contribution or boot installation is constructed.
        assert_eq!(
            report
                .pointer("/candidate/source_recipe_projection/authenticated_qualification_receipt")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert!(boot_gates().is_none());
    }

    #[test]
    fn captured_source_recipe_and_samples_cannot_supply_a_zero_qualification_receipt() {
        for report in [captured_growth("default"), captured_growth("development")] {
            assert_captured_no_receipt(&report);
        }
        assert!(boot_gates().is_none());
    }

    fn common_catalog_costs() -> CatalogResourceCosts {
        CatalogResourceCosts {
            resident_bytes: 1009,
            initialization_bytes: 2017,
            schema_validation_per_auction_bytes: 3011,
            context_per_auction_bytes: 4001,
        }
    }

    fn isolate_vector(terms: IsolateTerms) -> [u64; 11] {
        let IsolateTerms {
            baseline_bytes,
            gvl_bytes,
            auctions_bytes,
            auction_egress_bytes,
            auxiliary_bytes,
            terminal_egress_bytes,
            probe_output_bytes,
            pre_admission_parser_bytes,
            body_streams_bytes,
            catalog_resident_bytes,
            catalog_initialization_bytes,
        } = terms;
        [
            baseline_bytes,
            gvl_bytes,
            auctions_bytes,
            auction_egress_bytes,
            auxiliary_bytes,
            terminal_egress_bytes,
            probe_output_bytes,
            pre_admission_parser_bytes,
            body_streams_bytes,
            catalog_resident_bytes,
            catalog_initialization_bytes,
        ]
    }

    fn auction_vector(charges: AuctionCharges) -> [u64; 6] {
        let AuctionCharges {
            config,
            ingress,
            privacy,
            projection,
            outbound,
            response,
        } = charges;
        [config, ingress, privacy, projection, outbound, response]
    }

    fn assert_accounting_delta(
        label: &str,
        baseline: &Derived,
        candidate: &Derived,
        isolate_delta: [u64; 11],
        auction_delta: [u64; 6],
        total_delta: u64,
    ) {
        let mut expected_isolate = isolate_vector(baseline.isolate_terms);
        for (bytes, delta) in expected_isolate.iter_mut().zip(isolate_delta) {
            *bytes = bytes.checked_add(delta).unwrap();
        }
        assert_eq!(
            isolate_vector(candidate.isolate_terms),
            expected_isolate,
            "{label}: B/G/Q/E/A/T/P/H/S/C/I"
        );
        let mut expected_auction = auction_vector(baseline.auction_charges);
        for (bytes, delta) in expected_auction.iter_mut().zip(auction_delta) {
            *bytes = bytes.checked_add(delta).unwrap();
        }
        assert_eq!(
            auction_vector(candidate.auction_charges),
            expected_auction,
            "{label}: config/ingress/privacy/projection/outbound/response"
        );
        assert_eq!(
            candidate.total_bytes,
            baseline.total_bytes.checked_add(total_delta).unwrap(),
            "{label}: isolate total"
        );
        assert_eq!(
            candidate.per_auction_peak,
            baseline
                .per_auction_peak
                .checked_add(auction_delta.into_iter().sum::<u64>())
                .unwrap(),
            "{label}: auction peak"
        );
        assert_eq!(candidate.per_stream_peak, baseline.per_stream_peak);
        assert_eq!(
            candidate.isolate_terms.checked_total(),
            Ok(candidate.total_bytes)
        );
        assert_eq!(
            candidate.auction_charges.total().unwrap(),
            candidate.per_auction_peak
        );
    }

    enum OwnerInput {
        Baseline,
        GvlCache,
        Construction,
        Egress,
        Auxiliary,
        Terminal,
        Transmit,
        PlatformCeiling,
        BodyStreams,
        CatalogResident,
        CatalogInitialization,
        H1Parser,
    }

    // Independent literal deltas in B/G/Q/E/A/T/P/H/S/C/I order, then total.
    const OWNER_CONTROLS: [(&str, OwnerInput, [u64; 11], u64); 12] = [
        (
            "baseline",
            OwnerInput::Baseline,
            [11, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            11,
        ),
        (
            "GVL cache",
            OwnerInput::GvlCache,
            [0, 512_000, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            512_000,
        ),
        (
            "construction",
            OwnerInput::Construction,
            [0, 0, 26, 0, 0, 0, 0, 0, 0, 0, 0],
            26,
        ),
        (
            "egress",
            OwnerInput::Egress,
            [0, 0, 0, 33_558_528, 0, 0, 0, 0, 0, 0, 0],
            33_558_528,
        ),
        (
            "auxiliary",
            OwnerInput::Auxiliary,
            [0, 512_000, 0, 0, 3_221_233_664, 0, 0, 0, 0, 0, 0],
            3_221_745_664,
        ),
        (
            "terminal",
            OwnerInput::Terminal,
            [0, 0, 0, 0, 0, 4608, 0, 0, 0, 0, 0],
            4608,
        ),
        (
            "transmit",
            OwnerInput::Transmit,
            [0, 0, 0, 51, 170, 1156, 68, 0, 0, 0, 0],
            1445,
        ),
        (
            "platform ceiling",
            OwnerInput::PlatformCeiling,
            [0, 0, 0, 0, 0, 0, 0, 514_560, 0, 0, 0],
            514_560,
        ),
        (
            "body streams",
            OwnerInput::BodyStreams,
            [0, 0, 0, 0, 0, 0, 0, 0, 65_475_552, 0, 0],
            65_475_552,
        ),
        (
            "catalog resident",
            OwnerInput::CatalogResident,
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 23, 0],
            23,
        ),
        (
            "catalog initialization",
            OwnerInput::CatalogInitialization,
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 29],
            29,
        ),
        (
            "H1 parser",
            OwnerInput::H1Parser,
            [0, 0, 0, 0, 0, 0, 0, 1_376_640, 0, 0, 0],
            1_376_640,
        ),
    ];

    enum CostInput {
        Baked,
        BackendCap,
        BlobCap,
        NodeCap,
        CatalogResident,
        CatalogInitialization,
        Validation,
        Context,
    }

    // Input, config receipt delta, projection receipt delta, then C/I deltas.
    const COST_CONTROLS: [(&str, CostInput, u64, u64, u64, u64); 8] = [
        ("baked", CostInput::Baked, 101 * 128, 0, 0, 0),
        ("backend cap", CostInput::BackendCap, 47 * 129, 0, 0, 0),
        ("blob cap", CostInput::BlobCap, 53 * 128, 0, 0, 0),
        ("node cap", CostInput::NodeCap, 41 * 623, 0, 0, 0),
        ("catalog resident", CostInput::CatalogResident, 0, 0, 23, 0),
        (
            "catalog initialization",
            CostInput::CatalogInitialization,
            0,
            0,
            0,
            29,
        ),
        ("validation", CostInput::Validation, 0, 31, 0, 0),
        ("context", CostInput::Context, 0, 37, 0, 0),
    ];

    #[test]
    fn independent_owner_inputs_preserve_complete_isolate_and_auction_vectors() {
        assert!(boot_gates().is_none());
        let inputs = resolve_budget_inputs(&envelope::limits()).unwrap();
        let baseline = derive_raw(
            &inputs.limits,
            &target(),
            construction_costs(),
            inputs.baked_config_bytes,
            common_catalog_costs(),
        );
        assert!(
            isolate_vector(baseline.isolate_terms)
                .into_iter()
                .all(|bytes| bytes > 0)
        );
        assert_eq!(baseline.h2_slot_peak, 102_912);
        let mut checked = 0_u32;
        for (label, input, isolate_delta, total_delta) in OWNER_CONTROLS {
            let mut limits = inputs.limits;
            let mut platform = target();
            let mut construction = construction_costs();
            let mut costs = common_catalog_costs();
            let mut auction_delta = [0; 6];
            match input {
                OwnerInput::Baseline => limits.baseline_bytes += 11,
                OwnerInput::GvlCache => limits.gvl_cache_entries += 1,
                OwnerInput::Construction => {
                    construction.fixed_per_auction_bytes += 13;
                    auction_delta = [0, 0, 13, 0, 0, 0];
                }
                OwnerInput::Egress => limits.egress_permits += 1,
                OwnerInput::Auxiliary => limits.aux_route_permits = 6,
                OwnerInput::Terminal => limits.terminal_response_permits += 1,
                OwnerInput::Transmit => limits.transmit_state_bytes += 17,
                OwnerInput::PlatformCeiling => platform.platform_concurrent_ceiling = 389,
                OwnerInput::BodyStreams => limits.max_active_body_streams = 5,
                OwnerInput::CatalogResident => costs.resident_bytes += 23,
                OwnerInput::CatalogInitialization => costs.initialization_bytes += 29,
                OwnerInput::H1Parser => platform.h1_max_buf_size_bytes = 81_921,
            }
            let candidate = derive_raw(
                &limits,
                &platform,
                construction,
                inputs.baked_config_bytes,
                costs,
            );
            if matches!(input, OwnerInput::H1Parser) {
                assert_eq!(candidate.h1_slot_peak, 106_497);
                assert_eq!(candidate.h2_slot_peak, 102_912);
            }
            assert_accounting_delta(
                label,
                &baseline,
                &candidate,
                isolate_delta,
                auction_delta,
                total_delta,
            );
            assert_eq!(candidate.construction_costs, construction);
            assert!(boot_gates().is_none());
            checked += 1;
        }
        assert_eq!(checked, 12);
        assert!(boot_gates().is_none());
    }

    #[test]
    fn config_and_catalog_costs_follow_independent_auction_and_auxiliary_populations() {
        assert!(boot_gates().is_none());
        let inputs = resolve_budget_inputs(&envelope::limits()).unwrap();
        let mut checked = 0_u32;
        for auctions in [1_u32, 3, 7] {
            for auxiliary in [0_u32, 1, 5] {
                let raw = ResolvedIsolateLimits {
                    max_concurrent_auctions: auctions,
                    aux_route_permits: auxiliary,
                    ..inputs.limits
                };
                let baseline = derive_raw(
                    &raw,
                    &target(),
                    construction_costs(),
                    inputs.baked_config_bytes,
                    common_catalog_costs(),
                );
                for (
                    label,
                    input,
                    config_delta,
                    projection_delta,
                    resident_delta,
                    initialization_delta,
                ) in COST_CONTROLS
                {
                    let mut limits = raw;
                    let mut baked_bytes = inputs.baked_config_bytes;
                    let mut costs = common_catalog_costs();
                    match input {
                        CostInput::Baked => baked_bytes += 101,
                        CostInput::BackendCap => limits.max_runtime_config_bytes += 47,
                        CostInput::BlobCap => limits.max_runtime_config_blob_bytes += 53,
                        CostInput::NodeCap => limits.max_config_json_nodes += 41,
                        CostInput::CatalogResident => costs.resident_bytes += 23,
                        CostInput::CatalogInitialization => costs.initialization_bytes += 29,
                        CostInput::Validation => costs.schema_validation_per_auction_bytes += 31,
                        CostInput::Context => costs.context_per_auction_bytes += 37,
                    }
                    // Synthetic baked bytes go only to the pure runtime call.
                    let candidate =
                        derive_raw(&limits, &target(), construction_costs(), baked_bytes, costs);
                    let auction_delta = (config_delta + projection_delta) * u64::from(auctions);
                    assert_accounting_delta(
                        &format!("{label}: N={auctions}, aux={auxiliary}"),
                        &baseline,
                        &candidate,
                        [
                            0,
                            0,
                            auction_delta,
                            0,
                            0,
                            0,
                            0,
                            0,
                            0,
                            resident_delta,
                            initialization_delta,
                        ],
                        [config_delta, 0, 0, projection_delta, 0, 0],
                        auction_delta + resident_delta + initialization_delta,
                    );
                    assert_eq!(candidate.catalog_resident_term, 1009 + resident_delta);
                    assert_eq!(
                        candidate.catalog_initialization_term,
                        2017 + initialization_delta
                    );
                    assert_eq!(
                        candidate.schema_validation_term,
                        costs.schema_validation_per_auction_bytes
                    );
                    assert_eq!(
                        candidate.catalog_context_term,
                        costs.context_per_auction_bytes
                    );
                    assert_eq!(candidate.construction_costs, baseline.construction_costs);
                    let resolved = resolve_budget_inputs(&limits).unwrap();
                    assert_eq!(resolved.limits, limits);
                    assert_eq!(resolved.baked_config_bytes, inputs.baked_config_bytes);
                    assert!(boot_gates().is_none());
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 72);
        assert!(boot_gates().is_none());
    }

    #[test]
    fn dominant_auction_work_still_adds_initialization_once_not_maximum() {
        assert!(boot_gates().is_none());
        let mut raw = envelope::limits();
        raw.max_concurrent_auctions = 3;
        raw.aux_route_permits = 0;
        let costs = CatalogResourceCosts {
            schema_validation_per_auction_bytes: 2_000_000_003,
            ..CatalogResourceCosts::UNCHANGED
        };
        let baseline = derive(&raw, costs).unwrap();
        let initialized = derive(
            &raw,
            CatalogResourceCosts {
                initialization_bytes: 2017,
                ..costs
            },
        )
        .unwrap();
        let auction_bytes = initialized.isolate_terms.auctions_bytes;
        assert!(auction_bytes > initialized.total_bytes - auction_bytes);
        assert_eq!(initialized.total_bytes - baseline.total_bytes, 2017);
        assert_eq!(initialized.catalog_initialization_term, 2017);
        assert_eq!(initialized.isolate_terms.catalog_initialization_bytes, 2017);
        assert_eq!(initialized.per_auction_peak, baseline.per_auction_peak);
        assert_eq!(auction_bytes, baseline.isolate_terms.auctions_bytes);
        assert_eq!(
            initialized.isolate_terms.checked_total(),
            Ok(initialized.total_bytes)
        );
        assert!(boot_gates().is_none());
    }

    #[test]
    fn auxiliary_zero_minimum_and_unequal_probe_populations_are_independent() {
        assert!(boot_gates().is_none());
        // One declared slot is 768 MiB * 2 handoffs + 4096 transmit bytes.
        // Probe config keeps one slot at zero aux; probe output always has four.
        for (permits, work_slots) in [(0_u32, 1_u64), (1, 2), (3, 6), (7, 14)] {
            let mut raw = envelope::limits();
            raw.aux_route_permits = permits;
            let baseline = derive(&raw, CatalogResourceCosts::UNCHANGED).unwrap();
            assert_eq!(baseline.aux_term, work_slots * 1_610_616_832);
            assert_eq!(baseline.probe_output_term, 90_112);
            let mut changed = raw;
            changed.transmit_state_bytes += 17;
            let transmitted = derive(&changed, CatalogResourceCosts::UNCHANGED).unwrap();
            assert_eq!(transmitted.aux_term - baseline.aux_term, work_slots * 17);
            assert_eq!(
                transmitted.probe_output_term - baseline.probe_output_term,
                68
            );
            assert_eq!(transmitted.per_auction_peak, baseline.per_auction_peak);
            assert_eq!(transmitted.per_stream_peak, baseline.per_stream_peak);
        }
        assert!(boot_gates().is_none());
    }

    #[test]
    fn disabled_auxiliary_still_requires_the_owner_minimum_for_probe_config() {
        assert!(boot_gates().is_none());
        let mut raw = envelope::limits();
        raw.aux_route_permits = 0;
        let inputs = resolve_budget_inputs(&raw).unwrap();
        let floor =
            derive_aux_work_for_profile(&inputs.limits, &target(), inputs.baked_config_bytes)
                .unwrap()
                .peak_bytes();
        raw.aux_route_peak_bytes = floor;
        let exact = derive(&raw, CatalogResourceCosts::UNCHANGED).unwrap();
        assert_eq!(exact.aux_term, floor * 2 + 4096);
        raw.aux_route_peak_bytes -= 1;
        assert!(matches!(
            derive(&raw, CatalogResourceCosts::UNCHANGED),
            Err(BudgetError::AuxRoutePeakBelowDerived { required, .. }
                | BudgetError::AuxRoutePeakBelowExtractionFloor { required, .. }) if required == floor
        ));
        assert!(boot_gates().is_none());
    }

    #[test]
    fn call_count_and_stream_width_move_separate_owner_terms() {
        assert!(boot_gates().is_none());
        let mut raw = envelope::limits();
        raw.max_outbound_requests = 2;
        raw.max_active_body_streams = 1;
        let baseline = derive(&raw, CatalogResourceCosts::UNCHANGED).unwrap();
        let mut calls = raw;
        calls.max_outbound_requests = 5;
        let longer = derive(&calls, CatalogResourceCosts::UNCHANGED).unwrap();
        let outbound_per_call = 0x0008_0000
            + u64::from(auctioneer_bidder::outbound_meta_bytes(
                raw.max_impressions_per_auction,
            ))
            + 7680;
        assert_eq!(
            longer.auction_charges.outbound - baseline.auction_charges.outbound,
            3 * outbound_per_call
        );
        // Each additional response funds 256 KiB retained + 12 KiB headers,
        // parse/assembly work + 64K nodes, and a 24,640-byte debug row.
        assert_eq!(
            longer.auction_charges.response - baseline.auction_charges.response,
            3 * 122_720_320
        );
        assert_eq!(longer.request_side_term, baseline.request_side_term);
        assert_eq!(longer.per_stream_peak, baseline.per_stream_peak);
        assert_eq!(
            longer.isolate_terms.body_streams_bytes,
            baseline.isolate_terms.body_streams_bytes
        );
        assert_eq!(
            longer.total_bytes - baseline.total_bytes,
            2 * (longer.per_auction_peak - baseline.per_auction_peak)
        );

        let mut streams = raw;
        streams.max_active_body_streams = 7;
        let wider = derive(&streams, CatalogResourceCosts::UNCHANGED).unwrap();
        assert_eq!(wider.auction_charges, baseline.auction_charges);
        assert_eq!(wider.per_stream_peak, baseline.per_stream_peak);
        assert_eq!(
            wider.isolate_terms.body_streams_bytes - baseline.isolate_terms.body_streams_bytes,
            6 * baseline.per_stream_peak
        );
        assert_eq!(
            wider.total_bytes - baseline.total_bytes,
            6 * baseline.per_stream_peak
        );
        assert!(boot_gates().is_none());
    }

    #[test]
    fn checked_ceiling_accepts_below_and_at_but_refuses_above() {
        assert!(boot_gates().is_none());
        let inputs = resolve_budget_inputs(&envelope::limits()).unwrap();
        // The strict owner API has no baked-config argument. Keep that input
        // zero in both owner calls rather than constructing another checker.
        let costs = CatalogResourceCosts {
            initialization_bytes: 2017,
            ..CatalogResourceCosts::UNCHANGED
        };
        let derived = derive_isolate_budget_with_baked_config(
            &inputs.limits,
            &target(),
            auctioneer_bidder::outbound_meta_bytes(inputs.limits.max_impressions_per_auction),
            construction_costs(),
            0,
            costs,
        )
        .unwrap();
        for (ceiling, margin) in [
            (derived.total_bytes + 1, 1),
            (derived.total_bytes, 0),
            (derived.total_bytes - 1, 0),
        ] {
            let mut limits = inputs.limits;
            limits.isolate_total_bytes = ceiling;
            let retained = derive_isolate_budget_with_baked_config(
                &limits,
                &target(),
                auctioneer_bidder::outbound_meta_bytes(limits.max_impressions_per_auction),
                construction_costs(),
                0,
                costs,
            )
            .unwrap();
            assert_eq!(retained.total_bytes, derived.total_bytes);
            assert_eq!(retained.margin_bytes, margin);
            let checked = validate_isolate_limits(
                &limits,
                &target(),
                auctioneer_bidder::outbound_meta_bytes(limits.max_impressions_per_auction),
                construction_costs(),
                costs,
            );
            if ceiling < derived.total_bytes {
                assert_eq!(
                    checked,
                    Err(BudgetError::BudgetOverCeiling {
                        required: derived.total_bytes,
                        ceiling
                    })
                );
            } else {
                assert_eq!(checked, Ok(retained));
            }
        }
        assert!(boot_gates().is_none());
    }

    #[test]
    fn checked_overflow_refuses_independent_catalog_and_population_controls() {
        assert!(boot_gates().is_none());
        for costs in [
            CatalogResourceCosts {
                resident_bytes: u64::MAX,
                ..CatalogResourceCosts::UNCHANGED
            },
            CatalogResourceCosts {
                initialization_bytes: u64::MAX,
                ..CatalogResourceCosts::UNCHANGED
            },
            CatalogResourceCosts {
                schema_validation_per_auction_bytes: u64::MAX,
                ..CatalogResourceCosts::UNCHANGED
            },
            CatalogResourceCosts {
                context_per_auction_bytes: u64::MAX,
                ..CatalogResourceCosts::UNCHANGED
            },
        ] {
            assert_eq!(
                derive(&envelope::limits(), costs),
                Err(BudgetError::BudgetArithmeticOverflow)
            );
        }
        for (field, raw) in [
            (
                "baseline",
                ResolvedIsolateLimits {
                    baseline_bytes: u64::MAX,
                    ..envelope::limits()
                },
            ),
            (
                "auxiliary",
                ResolvedIsolateLimits {
                    aux_route_peak_bytes: u64::MAX,
                    ..envelope::limits()
                },
            ),
            (
                "body_streams",
                ResolvedIsolateLimits {
                    guest_decoder_bound_bytes: u64::MAX,
                    ..envelope::limits()
                },
            ),
        ] {
            assert_eq!(
                derive(&raw, CatalogResourceCosts::UNCHANGED),
                Err(BudgetError::BudgetArithmeticOverflow),
                "{field}"
            );
        }
        assert!(boot_gates().is_none());
    }

    #[test]
    fn default_resolution_is_independent_for_each_config_cap_and_never_installs() {
        assert!(boot_gates().is_none());
        // Independent SDK-default literals; do not compare two users of the
        // same default constant and thereby hide a change to that default.
        for mask in 0_u8..8 {
            let mut raw = envelope::limits();
            raw.max_runtime_config_bytes = if mask & 1 == 0 { 7919 } else { 0 };
            raw.max_runtime_config_blob_bytes = if mask & 2 == 0 { 3571 } else { 0 };
            raw.max_config_json_nodes = if mask & 4 == 0 { 127 } else { 0 };
            let effective = resolve_budget_inputs(&raw).unwrap();
            assert_eq!(
                effective.limits.max_runtime_config_bytes,
                if mask & 1 == 0 { 7919 } else { 0x0100_0000 }
            );
            assert_eq!(
                effective.limits.max_runtime_config_blob_bytes,
                if mask & 2 == 0 { 3571 } else { 0x0080_0000 }
            );
            assert_eq!(
                effective.limits.max_config_json_nodes,
                if mask & 4 == 0 { 127 } else { 8192 }
            );
            let captured = serde_json::to_vec(&(
                auctioneer_config_data::host_default(),
                auctioneer_config_data::account_default(),
            ))
            .unwrap();
            assert_eq!(
                effective.baked_config_bytes,
                u64::try_from(captured.len()).unwrap()
            );
            assert!(!captured.is_empty());
            assert_eq!(
                effective.limits.max_outbound_requests,
                raw.max_outbound_requests
            );
            assert!(boot_gates().is_none());
        }
    }

    #[test]
    fn catalog_residency_is_once_while_validation_and_context_follow_auctions() {
        assert!(boot_gates().is_none());
        for auctions in [1_u32, 3, 7] {
            let mut raw = envelope::limits();
            raw.max_concurrent_auctions = auctions;
            let inputs = resolve_budget_inputs(&raw).unwrap();
            let derive = |costs| {
                derive_isolate_budget_with_baked_config(
                    &inputs.limits,
                    &target(),
                    auctioneer_bidder::outbound_meta_bytes(raw.max_impressions_per_auction),
                    construction_costs(),
                    inputs.baked_config_bytes,
                    costs,
                )
                .unwrap()
            };
            let baseline = derive(CatalogResourceCosts::UNCHANGED);
            for (costs, fixed, per_auction) in [
                (
                    CatalogResourceCosts {
                        resident_bytes: 1009,
                        ..CatalogResourceCosts::UNCHANGED
                    },
                    1009_u64,
                    0_u64,
                ),
                (
                    CatalogResourceCosts {
                        initialization_bytes: 2017,
                        ..CatalogResourceCosts::UNCHANGED
                    },
                    2017,
                    0,
                ),
                (
                    CatalogResourceCosts {
                        schema_validation_per_auction_bytes: 3011,
                        ..CatalogResourceCosts::UNCHANGED
                    },
                    0,
                    3011,
                ),
                (
                    CatalogResourceCosts {
                        context_per_auction_bytes: 4001,
                        ..CatalogResourceCosts::UNCHANGED
                    },
                    0,
                    4001,
                ),
                (
                    CatalogResourceCosts {
                        resident_bytes: 1009,
                        initialization_bytes: 2017,
                        schema_validation_per_auction_bytes: 3011,
                        context_per_auction_bytes: 4001,
                    },
                    3026,
                    7012,
                ),
            ] {
                let candidate = derive(costs);
                let expected = fixed + u64::from(auctions) * per_auction;
                assert_eq!(candidate.total_bytes - baseline.total_bytes, expected);
                assert_eq!(
                    candidate.isolate_terms.checked_total().unwrap(),
                    candidate.total_bytes
                );
                assert_eq!(
                    candidate.auction_charges.total().unwrap(),
                    candidate.per_auction_peak
                );
                assert_eq!(
                    candidate.schema_validation_term,
                    costs.schema_validation_per_auction_bytes
                );
                assert_eq!(
                    candidate.catalog_context_term,
                    costs.context_per_auction_bytes
                );
                assert!(boot_gates().is_none());
            }
        }
    }
}
