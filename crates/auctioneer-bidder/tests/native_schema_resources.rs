//! Isolated native process: one test owns cold schema `LazyLock`s and the counting allocator.
//! Do not merge this into another target or prewarm validators in setup. Input construction,
//! fixture parsing, snapshots and reporting stay outside allocation windows. These checks
//! measure the frozen native reservations; they do not certify worst-case or guest readiness.

#![cfg(all(not(target_arch = "wasm32"), not(feature = "schema-artifact-staging")))]

#[cfg(test)]
mod tests {
    use auctioneer_bidder::schema::{
        NATIVE_CATALOG_INITIALIZATION_BYTES, NATIVE_CATALOG_RESIDENT_BYTES,
        NATIVE_SCHEMA_SCRATCH_BYTES, NativeSchemaError, controlled_native_costs,
        initialize_native_schemas, native_schema_available, native_schema_required,
        native_schema_work_budget, validate_native_parameters,
    };
    use auctioneer_bidder::test_support::serving_catalog::{self, ServingBidder};
    use auctioneer_runtime::{CatalogResourceCosts, test_support::alloc_track};
    use serde_json::{Map, Value, json};
    use sha2::{Digest as _, Sha256};
    use std::{
        collections::BTreeSet,
        fs,
        hint::black_box,
        io::{Write as _, stderr},
        path::Path,
        str,
    };

    #[global_allocator]
    static ALLOCATOR: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

    #[derive(Debug, PartialEq, Eq)]
    struct Sample {
        result: Result<(), NativeSchemaError>,
        peak: u64,
        allocated: u64,
        work: u64,
    }

    #[derive(Default)]
    struct Census {
        evaluations: usize,
        source_cases: usize,
        authored_cases: usize,
        lexical_refusals: usize,
        stress_cases: usize,
        max_peak: u64,
        max_allocated: u64,
    }

    #[derive(Default)]
    struct Coverage {
        census: Census,
        legacy_census: Census,
        legacy_bindings: BTreeSet<&'static str>,
        native_bindings: BTreeSet<&'static str>,
        native_owners: BTreeSet<&'static str>,
        source_owners: BTreeSet<&'static str>,
        no_source_capture: BTreeSet<&'static str>,
        patterns: BTreeSet<String>,
        hostname_fields: usize,
        aliases: usize,
    }

    fn shape(value: &Value) -> (u64, u32) {
        match value {
            Value::Array(items) => {
                let mut nodes = 1_u64;
                let mut depth = 0_u32;
                for item in items {
                    let (count, nested) = shape(item);
                    nodes = nodes.checked_add(count).unwrap();
                    depth = depth.max(nested);
                }
                (nodes, depth.checked_add(1).unwrap())
            }
            Value::Object(fields) => {
                let mut nodes = 1_u64;
                let mut depth = 0_u32;
                for item in fields.values() {
                    let (count, nested) = shape(item);
                    nodes = nodes
                        .checked_add(count)
                        .and_then(|sum| sum.checked_add(1))
                        .unwrap();
                    depth = depth.max(nested);
                }
                (nodes, depth.checked_add(1).unwrap())
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => (1, 0),
        }
    }

    fn evaluate(
        name: &str,
        label: &str,
        value: &Value,
        valid: Option<bool>,
        census: &mut Census,
    ) -> Sample {
        let snapshot = serde_json::to_vec(value).unwrap();
        let bytes = u64::try_from(snapshot.len()).unwrap();
        let (nodes, depth) = shape(value);
        let admitted = bytes <= 0x0001_0000 && nodes <= 4_096 && depth <= 32;
        let expected_work = if admitted {
            bytes
                .checked_mul(16)
                .and_then(|sum| sum.checked_add(nodes.checked_mul(128)?))
                .and_then(|sum| sum.checked_add(1_024))
                .unwrap()
        } else {
            0
        };
        let mut budget = native_schema_work_budget(0x0008_0000);
        let remaining = budget.remaining();
        assert_eq!(remaining, 0x0400_0000);
        let allocated = alloc_track::allocated();
        let baseline = alloc_track::reset_peak();
        let result =
            validate_native_parameters(black_box(name), black_box(value), &mut budget, || false);
        let sample = Sample {
            result,
            peak: alloc_track::peak_since(baseline),
            allocated: alloc_track::allocated().checked_sub(allocated).unwrap(),
            work: remaining.checked_sub(budget.remaining()).unwrap(),
        };
        let live = alloc_track::live();
        assert_eq!(live, baseline, "{name}/{label}: per-call cache retained");
        assert!(sample.peak <= 0x0008_0000, "{name}/{label}: {sample:?}");
        assert_eq!(sample.work, expected_work, "{name}/{label}: work debit");
        if !admitted || valid == Some(false) {
            assert_eq!(
                result,
                Err(NativeSchemaError::InvalidParameters),
                "{name}/{label}"
            );
        } else if valid == Some(true) {
            assert_eq!(
                result,
                Ok(()),
                "{name}/{label}: committed valid input rejected"
            );
        } else {
            assert!(
                matches!(result, Ok(()) | Err(NativeSchemaError::InvalidParameters)),
                "{name}/{label}: {result:?}"
            );
        }
        assert_eq!(
            serde_json::to_vec(value).unwrap(),
            snapshot,
            "{name}/{label}: borrowed input changed"
        );
        census.evaluations = census.evaluations.checked_add(1).unwrap();
        census.max_peak = census.max_peak.max(sample.peak);
        census.max_allocated = census.max_allocated.max(sample.allocated);
        sample
    }

    fn exercise(
        name: &str,
        label: &str,
        value: &Value,
        valid: Option<bool>,
        conflict: &Value,
        census: &mut Census,
    ) {
        let first = evaluate(name, label, value, valid, census);
        let _conflict = evaluate(
            name,
            "fake_conflicting_shape",
            conflict,
            Some(false),
            census,
        );
        let repeated = evaluate(name, label, value, valid, census);
        assert_eq!(
            first, repeated,
            "{name}/{label}: fresh cache/history-dependent charge or allocation"
        );
    }

    fn captured(manifest: &toml::Value, owner: &Path, relative: &str) -> Vec<u8> {
        let bytes = fs::read(owner.join(relative)).unwrap();
        let retained = manifest["provenance"]["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|artifact| artifact.get("retained"))
            .find(|retained| retained["source_path"].as_str() == Some(relative))
            .expect("captured schema/parameters must have a pinned retained witness");
        assert_eq!(
            u64::try_from(bytes.len()).unwrap(),
            u64::try_from(retained["source_bytes"].as_integer().unwrap()).unwrap()
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            retained["source_sha256"].as_str().unwrap(),
            "{relative}: captured source digest"
        );
        bytes
    }

    fn source_cases(path: &Path, bytes: &[u8], census: &mut Census) -> Vec<(String, Value, bool)> {
        let document: Value = serde_json::from_slice(bytes).unwrap();
        let mut cases = Vec::new();
        let mut append = |label: String, input: &str, valid: bool| {
            census.source_cases = census.source_cases.checked_add(1).unwrap();
            if let Ok(value) = serde_json::from_str(input) {
                cases.push((label, value, valid));
            } else {
                assert!(
                    !valid,
                    "{}:{label}: source valid case is not JSON",
                    path.display()
                );
                census.lexical_refusals = census.lexical_refusals.checked_add(1).unwrap();
            }
        };
        if let Some(rows) = document.get("cases").and_then(Value::as_array) {
            assert!(!rows.is_empty());
            for (index, row) in rows.iter().enumerate() {
                append(
                    format!("source_{index}"),
                    row["input"].as_str().unwrap(),
                    row["valid"].as_bool().unwrap(),
                );
            }
        } else {
            for (group, valid) in [("valid", true), ("invalid", false)] {
                let rows = document[group].as_array().unwrap();
                assert!(!rows.is_empty());
                for (index, input) in rows.iter().enumerate() {
                    append(
                        format!("source_{group}_{index}"),
                        input.as_str().unwrap(),
                        valid,
                    );
                }
            }
        }
        assert!(cases.iter().any(|(_, _, valid)| *valid));
        assert!(cases.iter().any(|(_, _, valid)| !valid));
        cases
    }

    fn authored_cases(path: &Path, schema_bytes: &[u8]) -> Vec<(String, Value, bool)> {
        let document: toml::Value = toml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(
            document["schema_sha256"].as_str().unwrap(),
            format!("{:x}", Sha256::digest(schema_bytes)),
            "{}: authored expectation schema pin",
            path.display()
        );
        let rows = document["rows"].as_array().unwrap();
        assert!(
            !rows.is_empty(),
            "{}: authored controls cannot be empty",
            path.display()
        );
        rows.iter()
            .map(|row| {
                (
                    format!("authored_{}", row["id"].as_str().unwrap()),
                    serde_json::from_str(row["input_json"].as_str().unwrap()).unwrap(),
                    row["valid"].as_bool().unwrap(),
                )
            })
            .collect()
    }

    fn with_field(base: &Value, field: &str, value: Value) -> Value {
        let mut output = base.clone();
        output
            .as_object_mut()
            .unwrap()
            .insert(field.to_owned(), value);
        output
    }

    fn shape_stress(base: &Value) -> Vec<Value> {
        let mut cases = vec![
            json!(null),
            json!(false),
            json!(7_i64),
            json!("fake_scalar"),
            json!([]),
        ];
        for width in [0, 1, 64, 1_024, 2_048, 4_096] {
            let object: Map<_, _> = (0..width)
                .map(|index| (format!("fake_{index}"), json!(null)))
                .collect();
            cases.push(with_field(base, "fake_stress", Value::Object(object)));
            cases.push(with_field(
                base,
                "fake_stress",
                Value::Array(vec![json!(null); width]),
            ));
        }
        for depth in [1_u32, 8, 30, 31, 32, 33] {
            let nested = (0..depth).fold(json!(null), |value, _| json!([value]));
            cases.push(with_field(base, "fake_stress", nested));
        }
        for length in [
            0_usize,
            1,
            64,
            4_096,
            0x8000,
            65_000,
            0x0001_0000,
            0x0001_0001,
        ] {
            cases.push(with_field(base, "fake_stress", json!("x".repeat(length))));
        }
        for length in [1, 1_024, 10_920, 10_923] {
            cases.push(with_field(
                base,
                "fake_stress",
                json!("\u{0000}".repeat(length)),
            ));
        }
        cases
    }

    fn pattern_stress(pattern: &str) -> Vec<(String, bool)> {
        match pattern {
            "^[0-9]+$" | r"^\d+$" | r"^[0-9]+(\.[0-9]+)?$" => {
                let mut cases = Vec::new();
                for length in [1, 63, 255, 4_096, 0x8000, 65_000] {
                    let digits = "7".repeat(length);
                    cases.push((digits.clone(), true));
                    cases.push((format!("{digits}x"), false));
                }
                if pattern == r"^[0-9]+(\.[0-9]+)?$" {
                    cases.extend([
                        ("0.5".into(), true),
                        ("1.".into(), false),
                        ("1.5x".into(), false),
                    ]);
                }
                cases
            }
            r"\.[a-zA-Z]{2,3}$" => {
                let at = [
                    "a".repeat(63),
                    "b".repeat(63),
                    "c".repeat(63),
                    "d".repeat(60),
                    "co".into(),
                ]
                .join(".");
                assert_eq!(at.len(), 255);
                let over = [
                    "a".repeat(63),
                    "b".repeat(63),
                    "c".repeat(63),
                    "d".repeat(61),
                    "co".into(),
                ]
                .join(".");
                assert_eq!(over.len(), 256);
                vec![
                    ("a.co".into(), true),
                    (format!("{}.co", "a".repeat(63)), true),
                    (format!("{}.co", "a".repeat(64)), false),
                    (at.clone(), true),
                    (over, false),
                    ("-a.co".into(), false),
                    ("a..co".into(), false),
                    ("a.co.".into(), false),
                    ("a.c".into(), false),
                    ("a.company".into(), false),
                    (format!("{}.co", "a".repeat(0x8000)), false),
                ]
            }
            _ => panic!("new native pattern requires independent stress expectations: {pattern}"),
        }
    }

    fn assert_cold_allocator_control() {
        assert_eq!(NATIVE_CATALOG_RESIDENT_BYTES, 0x0100_0000);
        assert_eq!(NATIVE_CATALOG_INITIALIZATION_BYTES, 0x0080_0000);
        assert_eq!(NATIVE_SCHEMA_SCRATCH_BYTES, 0x0008_0000);
        let allocated = alloc_track::allocated();
        let baseline = alloc_track::reset_peak();
        let positive = black_box(vec![0_u8; 4_096]);
        assert_eq!(alloc_track::peak_since(baseline), 4_096);
        assert_eq!(
            alloc_track::allocated().checked_sub(allocated).unwrap(),
            4_096
        );
        assert_eq!(alloc_track::live().checked_sub(baseline).unwrap(), 4_096);
        drop(black_box(positive));
        assert_eq!(alloc_track::live(), baseline);
    }

    fn initialize_cold_schemas() {
        let costs = controlled_native_costs(CatalogResourceCosts::UNCHANGED);
        assert_eq!(costs.resident_bytes, 0x0100_0000);
        assert_eq!(costs.initialization_bytes, 0x0080_0000);
        assert_eq!(costs.schema_validation_per_auction_bytes, 0x0008_0000);
        for unfunded in [
            CatalogResourceCosts {
                resident_bytes: 0x00ff_ffff,
                ..costs
            },
            CatalogResourceCosts {
                initialization_bytes: 0x007f_ffff,
                ..costs
            },
            CatalogResourceCosts {
                schema_validation_per_auction_bytes: 0x0007_ffff,
                ..costs
            },
        ] {
            assert!(!initialize_native_schemas(unfunded));
        }
        let allocated = alloc_track::allocated();
        let baseline = alloc_track::reset_peak();
        let initialized = initialize_native_schemas(costs);
        let initialization_peak = alloc_track::peak_since(baseline);
        let resident = u64::try_from(alloc_track::live().checked_sub(baseline).unwrap()).unwrap();
        let initialization_temporary = initialization_peak.checked_sub(resident).unwrap();
        let initialization_allocated = alloc_track::allocated().checked_sub(allocated).unwrap();
        assert!(initialized, "native schema initialization refused");
        assert!(
            initialization_allocated > 0,
            "cold initialization must exercise real LazyLocks"
        );
        assert!(
            initialization_peak <= 0x0180_0000,
            "cold native I+C peak: {initialization_peak}"
        );
        assert!(resident <= 0x0100_0000, "native C live bytes: {resident}");
        assert!(
            initialization_temporary <= 0x0080_0000,
            "native I temporary bytes: {initialization_temporary}"
        );
        let cached_allocated = alloc_track::allocated();
        let live = alloc_track::live();
        assert!(initialize_native_schemas(costs));
        assert_eq!(
            alloc_track::allocated(),
            cached_allocated,
            "cached initialization must not allocate"
        );
        assert_eq!(alloc_track::live(), live);
        writeln!(
            stderr().lock(),
            "native schemas cold: resident={resident} temporary={initialization_temporary} peak_I_plus_C={initialization_peak} allocated={initialization_allocated}"
        )
        .unwrap();
    }

    #[test]
    fn cold_native_schema_reservations_cover_discovered_bindings_and_fresh_evaluations() {
        assert_cold_allocator_control();
        initialize_cold_schemas();
        let rows = serving_catalog::published();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bidders");
        let mut coverage = Coverage::default();
        let invalid_root = json!(null);
        for row in &rows {
            let owner = root.join(row.implementation);
            let manifest_bytes = fs::read(owner.join("bidder.toml")).unwrap();
            let manifest: toml::Value =
                toml::from_str(str::from_utf8(&manifest_bytes).unwrap()).unwrap();
            let native = manifest
                .get("native_only")
                .and_then(toml::Value::as_bool)
                .unwrap_or(false);
            assert_eq!(
                native_schema_required(row.name()),
                native,
                "{}: publication/schema dispatch mismatch",
                row.name()
            );
            if native {
                exercise_native(
                    row,
                    &rows,
                    &owner,
                    &manifest,
                    &manifest_bytes,
                    &invalid_root,
                    &mut coverage,
                );
            } else {
                exercise_legacy(row, &owner, &manifest, &invalid_root, &mut coverage);
            }
        }
        assert_coverage(&coverage);
        report_coverage(&coverage);
    }

    fn exercise_legacy(
        row: &ServingBidder,
        owner: &Path,
        manifest: &toml::Value,
        invalid_root: &Value,
        coverage: &mut Coverage,
    ) {
        if !native_schema_available(row.name()) {
            return;
        }
        assert!(coverage.legacy_bindings.insert(row.name()));
        let schema_bytes = captured(manifest, owner, "parameters.json");
        let controls = authored_cases(&owner.join("tests/schema/semantics.toml"), &schema_bytes);
        let (_, base, _) = controls
            .iter()
            .find(|(_, _, valid)| *valid)
            .expect("available legacy schema has an independently reviewed valid input");
        for (label, value, valid) in &controls {
            coverage.legacy_census.authored_cases = coverage
                .legacy_census
                .authored_cases
                .checked_add(1)
                .unwrap();
            exercise(
                row.name(),
                label,
                value,
                Some(*valid),
                invalid_root,
                &mut coverage.legacy_census,
            );
        }
        let source_path = owner.join("fixtures/parameters.json");
        if source_path.is_file() {
            let bytes = captured(manifest, owner, "fixtures/parameters.json");
            for (label, value, valid) in
                source_cases(&source_path, &bytes, &mut coverage.legacy_census)
            {
                exercise(
                    row.name(),
                    &label,
                    &value,
                    Some(valid),
                    invalid_root,
                    &mut coverage.legacy_census,
                );
            }
        }
        for (index, value) in shape_stress(base).iter().enumerate() {
            coverage.legacy_census.stress_cases =
                coverage.legacy_census.stress_cases.checked_add(1).unwrap();
            let valid = if value.is_object() { None } else { Some(false) };
            exercise(
                row.name(),
                &format!("fake_legacy_shape_{index}"),
                value,
                valid,
                invalid_root,
                &mut coverage.legacy_census,
            );
        }
    }

    fn exercise_native(
        row: &ServingBidder,
        rows: &[ServingBidder],
        owner: &Path,
        manifest: &toml::Value,
        manifest_bytes: &[u8],
        invalid_root: &Value,
        coverage: &mut Coverage,
    ) {
        assert!(native_schema_available(row.name()));
        assert!(coverage.native_bindings.insert(row.name()));
        coverage.native_owners.insert(row.implementation);
        let factory: Value =
            serde_json::from_slice(&fs::read(owner.join("fixtures/factory.json")).unwrap())
                .unwrap();
        assert_eq!(
            factory["manifest_sha256"].as_str().unwrap(),
            format!("{:x}", Sha256::digest(manifest_bytes)),
            "{}: first factory case manifest pin",
            row.name()
        );
        let schema_bytes = captured(manifest, owner, "parameters.json");
        assert_eq!(row.info.default_enabled, Some(false));
        if let Some(parent) = row.info.alias_of.as_deref() {
            coverage.aliases = coverage.aliases.checked_add(1).unwrap();
            assert!(
                rows.iter()
                    .any(|base| base.name() == parent && native_schema_required(parent))
            );
        }
        exercise(
            row.name(),
            "factory_first_case",
            &row.params,
            Some(true),
            invalid_root,
            &mut coverage.census,
        );
        let source_path = owner.join("fixtures/parameters.json");
        exercise_native_source(row, owner, &source_path, manifest, invalid_root, coverage);
        for (label, value, valid) in
            authored_cases(&owner.join("tests/schema/semantics.toml"), &schema_bytes)
        {
            coverage.census.authored_cases = coverage.census.authored_cases.checked_add(1).unwrap();
            exercise(
                row.name(),
                &label,
                &value,
                Some(valid),
                invalid_root,
                &mut coverage.census,
            );
        }
        for (index, value) in shape_stress(&row.params).iter().enumerate() {
            coverage.census.stress_cases = coverage.census.stress_cases.checked_add(1).unwrap();
            let valid = if value.is_object() { None } else { Some(false) };
            exercise(
                row.name(),
                &format!("fake_shape_{index}"),
                value,
                valid,
                invalid_root,
                &mut coverage.census,
            );
        }
        let schema: Value = serde_json::from_slice(&schema_bytes).unwrap();
        exercise_native_patterns(row, &schema, coverage);
        assert_native_budget_refusals(row);
    }

    fn exercise_native_source(
        row: &ServingBidder,
        owner: &Path,
        source_path: &Path,
        manifest: &toml::Value,
        invalid_root: &Value,
        coverage: &mut Coverage,
    ) {
        if source_path.is_file() {
            coverage.source_owners.insert(row.implementation);
            let bytes = captured(manifest, owner, "fixtures/parameters.json");
            for (label, value, valid) in source_cases(source_path, &bytes, &mut coverage.census) {
                exercise(
                    row.name(),
                    &label,
                    &value,
                    Some(valid),
                    invalid_root,
                    &mut coverage.census,
                );
            }
        } else {
            coverage.no_source_capture.insert(row.implementation);
        }
    }

    fn exercise_native_patterns(row: &ServingBidder, schema: &Value, coverage: &mut Coverage) {
        assert_eq!(schema["type"], "object");
        for (field, property) in schema["properties"].as_object().unwrap() {
            let Some(pattern) = property.get("pattern").and_then(Value::as_str) else {
                continue;
            };
            coverage.patterns.insert(pattern.to_owned());
            if property.get("format").and_then(Value::as_str) == Some("hostname") {
                coverage.hostname_fields = coverage.hostname_fields.checked_add(1).unwrap();
            }
            let pattern_conflict = with_field(&row.params, field, json!("!"));
            for (index, (text, valid)) in pattern_stress(pattern).into_iter().enumerate() {
                coverage.census.stress_cases = coverage.census.stress_cases.checked_add(1).unwrap();
                let input = with_field(&row.params, field, json!(text));
                exercise(
                    row.name(),
                    &format!("fake_pattern_{field}_{index}"),
                    &input,
                    Some(valid),
                    &pattern_conflict,
                    &mut coverage.census,
                );
            }
        }
    }

    fn assert_native_budget_refusals(row: &ServingBidder) {
        let mut unfunded = native_schema_work_budget(0x0007_ffff);
        let remaining = unfunded.remaining();
        assert_eq!(
            validate_native_parameters(row.name(), &row.params, &mut unfunded, || false),
            Err(NativeSchemaError::Resource)
        );
        assert_eq!(unfunded.remaining(), remaining);
        let mut expired = native_schema_work_budget(0x0008_0000);
        assert_eq!(
            validate_native_parameters(row.name(), &row.params, &mut expired, || true),
            Err(NativeSchemaError::Deadline)
        );
        assert_eq!(expired.remaining(), 0x0400_0000);
    }

    fn assert_coverage(coverage: &Coverage) {
        assert_eq!(
            coverage.native_bindings.len(),
            14,
            "13 new selected identities plus canonical alias parent"
        );
        assert_eq!(coverage.native_owners.len(), 13);
        assert_eq!(coverage.aliases, 1);
        assert_eq!(
            coverage.patterns.len(),
            4,
            "all four shipped native patterns must be stressed"
        );
        assert_eq!(
            coverage.hostname_fields, 1,
            "shared hostname helper must be reached"
        );
        assert_eq!(coverage.source_owners.len(), 6);
        assert_eq!(coverage.no_source_capture.len(), 7);
        assert_eq!(coverage.census.source_cases, 149);
        assert_eq!(coverage.census.lexical_refusals, 5);
        assert_eq!(coverage.census.authored_cases, 120);
        assert_eq!(coverage.census.stress_cases, 564);
        assert_eq!(coverage.census.evaluations, 2_526);
        // The development feature adds one legacy schema, eight authored
        // cases and 35 shape cases, each evaluated three times.
        let (legacy_bindings, authored, stress, evaluations) = if cfg!(feature = "insecure-bidders")
        {
            (3, 33, 105, 489)
        } else {
            (2, 25, 70, 360)
        };
        assert_eq!(
            coverage.legacy_bindings.len(),
            legacy_bindings,
            "selected legacy schemas are available but not mandatory"
        );
        assert_eq!(coverage.legacy_census.authored_cases, authored);
        assert_eq!(coverage.legacy_census.source_cases, 26);
        assert_eq!(coverage.legacy_census.lexical_refusals, 1);
        assert_eq!(coverage.legacy_census.stress_cases, stress);
        assert_eq!(coverage.legacy_census.evaluations, evaluations);
    }

    fn report_coverage(coverage: &Coverage) {
        let missing_source_captures = coverage
            .no_source_capture
            .iter()
            .copied()
            .collect::<Vec<_>>()
            .join(",");
        let mut output = stderr().lock();
        writeln!(
            output,
            "native schemas evaluated: bindings={} owners={} source_owners={} source_cases={} lexical_refusals={} authored_cases={} stress_cases={} evaluations={} max_peak={} max_allocated={} missing_source_captures=[{missing_source_captures}]",
            coverage.native_bindings.len(),
            coverage.native_owners.len(),
            coverage.source_owners.len(),
            coverage.census.source_cases,
            coverage.census.lexical_refusals,
            coverage.census.authored_cases,
            coverage.census.stress_cases,
            coverage.census.evaluations,
            coverage.census.max_peak,
            coverage.census.max_allocated
        )
        .unwrap();
        writeln!(
            output,
            "available legacy schemas evaluated: bindings={} source_cases={} lexical_refusals={} authored_cases={} stress_cases={} evaluations={} max_peak={} max_allocated={}",
            coverage.legacy_bindings.len(),
            coverage.legacy_census.source_cases,
            coverage.legacy_census.lexical_refusals,
            coverage.legacy_census.authored_cases,
            coverage.legacy_census.stress_cases,
            coverage.legacy_census.evaluations,
            coverage.legacy_census.max_peak,
            coverage.legacy_census.max_allocated
        )
        .unwrap();
    }
}
