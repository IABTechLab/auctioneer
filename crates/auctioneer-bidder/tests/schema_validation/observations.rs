//! Selected native cache observations, never source or qualification authority.
//!
//! Child measurements are compared with component-only ceilings. They do not
//! authenticate a validation capture or close work, frames, cleanup, or target
//! obligations.

use crate::schema_build::{CacheLayout, CacheLayouts, Catalog, workload_data_for};
use crate::schema_bundle_heap::{CacheBundleHeap, format_peak};
use crate::schema_meta_heap::MetaCacheHeap;
use crate::schema_qualification::SchemaInventory;
use crate::schema_reference::{Corpus, captured};
use crate::schema_resource_layout::Identity;
use crate::schema_selection_source::SelectedCohort;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

/// Compare linked native measurements with component-only cache ceilings.
/// The returned observations confer no source or qualification authority.
pub(crate) fn selected_cache_observations(
    cohort: &SelectedCohort,
    catalog: &Catalog,
    identity: Identity,
) -> Value {
    use regex_automata::meta::Cache;
    let layouts = catalog.native_cache_layouts().unwrap();
    assert_eq!(layouts.schema_version, 2);
    assert_eq!(layouts.word_bytes, size_of::<usize>());
    assert_eq!(layouts.cache_bytes, size_of::<Cache>());
    assert_eq!(layouts.cache_alignment, align_of::<Cache>());
    assert_eq!(layouts.rows.len(), cohort.schemas.len());
    let selected_schemas = cohort.manifest["selection"]["schemas"].as_array().unwrap();
    assert_eq!(selected_schemas.len(), cohort.schemas.len());
    let observed = catalog.native_allocations("explicit-caches").unwrap();
    let observations = observed["schemas"].as_array().unwrap();
    assert_eq!(observations.len(), cohort.schemas.len());
    let corpus = captured().unwrap();
    let authored = workload_data_for(&cohort.root, &cohort.schemas).unwrap();
    let mut rows = Vec::new();
    let mut maximum_bundle = 0_usize;
    let mut maximum_observed = 0_usize;
    let mut helpers = 0_usize;
    let mut hostname_owners = 0_usize;
    let mut captured_cases = 0_usize;
    let mut authored_cases = 0_usize;
    let mut validation_calls = 0_usize;
    for (((schema, binding), layout), observation) in cohort
        .schemas
        .iter()
        .zip(selected_schemas)
        .zip(&layouts.rows)
        .zip(observations)
    {
        let name = &schema.witness.name;
        assert_eq!(binding["witness"]["name"], *name);
        assert_eq!(&layout.name, name);
        assert_eq!(observation["name"], *name);
        let (bundle, pattern_rows) = component_cache_bundle(schema, binding, layout, &layouts);
        helpers = helpers.checked_add(pattern_rows.len()).unwrap();
        hostname_owners = hostname_owners
            .checked_add(usize::from(bundle.format_bytes > 0))
            .unwrap();
        maximum_bundle = maximum_bundle.max(bundle.peak);
        let inputs = selected_cache_inputs(name, &corpus, authored.get(name));
        captured_cases = captured_cases.checked_add(inputs.captured).unwrap();
        authored_cases = authored_cases.checked_add(inputs.authored).unwrap();
        let (observed_peak, calls) =
            assert_cache_workloads(observation, &inputs, bundle.peak, authored.get(name));
        validation_calls = validation_calls.checked_add(calls).unwrap();
        if !pattern_rows.is_empty() {
            assert!(observed_peak > 0, "non-vacuous selected cache observation");
            assert!(
                inputs
                    .expected
                    .iter()
                    .any(|(_, valid)| *valid == Some(true))
            );
            assert!(
                inputs
                    .expected
                    .iter()
                    .any(|(_, valid)| *valid == Some(false))
            );
        }
        maximum_observed = maximum_observed.max(observed_peak);
        rows.push(json!({"name": name, "patterns": pattern_rows,
            "pattern_heap_bytes": bundle.pattern_bytes, "wrapper_inline_bytes": bundle.inline_bytes,
            "callback_storage_bytes": bundle.format_bytes, "storage_peak_bytes": bundle.peak,
            "captured_parseable_cases": inputs.captured, "authored_cases": inputs.authored,
            "observed_requested_heap_peak_bytes": observed_peak}));
    }
    assert_eq!(helpers, 4);
    assert_eq!(hostname_owners, 1);
    assert_eq!(captured_cases, 225);
    assert!(authored_cases > 0);
    assert_hostname_cache_observations(&observed);
    let identity_row = serde_json::to_value(identity).unwrap();
    json!({"schema_version": 1_u8, "kind": "native_cache_observations",
        "status": "ObservationOnly", "application_or_resource_qualification": false,
        "resource_qualification": false, "physical_fit": false, "serving_permission": false,
        "identity": identity_row,
        "layout_profile_correspondence_with_child": "not_established",
        "scope": "one fixed cache per active schema owner; serial earliest searches without reset, clone or rebinding",
        "max_input_bytes": 0x0001_0000_u64, "schemas": rows, "regex_helpers": helpers, "hostname_schemas": hostname_owners,
        "maximum_bundle_storage_bytes": maximum_bundle, "maximum_observed_requested_heap_bytes": maximum_observed,
        "captured_parseable_cases": captured_cases, "authored_cases": authored_cases, "observed_validation_calls": validation_calls,
        "probe_enum_inline_bytes": observed["owner_inline_bytes"],
        "probe_enum_storage": "outside measurement windows; not the production owner representation",
        "derivation_sources_sha256": {
            "cache": hex::encode(Sha256::digest(include_bytes!("../schema_meta_heap/mod.rs"))),
            "bundle": hex::encode(Sha256::digest(include_bytes!("../schema_bundle_heap/mod.rs")))},
        "observations": observed,
        "excludes": ["constructor_peak_and_work", "compiled_programs_and_other_schema_state", "complete_call_frames",
            "work_bound", "allocator_and_physical_memory", "guest_layouts", "serving_profile_qualification"]})
}

fn component_cache_bundle(
    schema: &SchemaInventory,
    binding: &Value,
    layout: &CacheLayout,
    layouts: &CacheLayouts,
) -> (CacheBundleHeap, Vec<Value>) {
    let patterns: Vec<(String, String)> =
        serde_json::from_value(binding["generated_helpers"].clone()).unwrap();
    assert_eq!(
        layout.inline_bytes,
        patterns.len().checked_mul(layouts.cache_bytes).unwrap()
    );
    assert_eq!(
        layout.alignment,
        if patterns.is_empty() {
            1
        } else {
            layouts.cache_alignment
        }
    );
    let mut rows = Vec::new();
    let mut peaks = Vec::new();
    for (helper, pattern) in patterns {
        let heap = MetaCacheHeap::from_pattern(&pattern, 0x0001_0000)
            .expect("selected serial native cache owner");
        peaks.push(heap.peak);
        rows.push(json!({"helper": helper,
            "pattern_sha256": hex::encode(Sha256::digest(pattern.as_bytes())),
            "eager_bytes": heap.eager_bytes, "forward_bytes": heap.forward_bytes,
            "reverse_bytes": heap.reverse_bytes, "optional_prefix_peak_bytes": heap.prefix_peak_bytes,
            "fallback_bytes": heap.fallback_bytes, "growth_overlap_bytes": heap.growth_overlap,
            "requested_heap_peak_bytes": heap.peak}));
    }
    let callback = format_peak(schema.audit.formats.iter().map(|row| row.value.as_str()))
        .expect("selected format cache owner");
    (
        CacheBundleHeap::compose(&peaks, layout.inline_bytes, callback).unwrap(),
        rows,
    )
}

struct CacheInputs {
    expected: Vec<(String, Option<bool>)>,
    input_bytes: Vec<Option<usize>>,
    captured: usize,
    authored: usize,
}

fn selected_cache_inputs(name: &str, corpus: &Corpus, authored: Option<&Value>) -> CacheInputs {
    let mut inputs = CacheInputs {
        expected: vec![("empty".to_owned(), None)],
        input_bytes: vec![Some(2_usize)],
        captured: 0,
        authored: 0,
    };
    for case in corpus.cases.iter().filter(|case| case.schema == name) {
        let Ok(value) = serde_json::from_str::<Value>(&case.input) else {
            assert!(!case.valid);
            continue;
        };
        inputs.expected.push((
            format!("captured_{:?}_{}", case.group, case.index),
            Some(case.valid),
        ));
        inputs
            .input_bytes
            .push(Some(serde_json::to_vec(&value).unwrap().len()));
        inputs.captured = inputs.captured.checked_add(1).unwrap();
    }
    if let Some(data) = authored {
        for row in data["rows"].as_array().unwrap() {
            inputs.expected.push((
                format!("authored_{}", row["id"].as_str().unwrap()),
                Some(row["valid"].as_bool().unwrap()),
            ));
            // Transforms stay owned by the existing child fixture runtime.
            inputs.input_bytes.push(None);
            inputs.authored = inputs.authored.checked_add(1).unwrap();
        }
    }
    inputs
}

fn assert_cache_workloads(
    observation: &Value,
    inputs: &CacheInputs,
    bound: usize,
    authored: Option<&Value>,
) -> (usize, usize) {
    let populations = observation["populations"].as_array().unwrap();
    assert_eq!(populations.len(), 3);
    let mut observed_peak = 0_usize;
    let mut calls = 0_usize;
    for (population, count) in populations.iter().zip([1_usize, 2, 4]) {
        observed_peak = observed_peak.max(assert_cache_population(
            population,
            count,
            bound,
            &inputs.expected,
        ));
        let searches = population["searches"].as_array().unwrap();
        for (index, (search, expected_bytes)) in
            searches.iter().zip(&inputs.input_bytes).enumerate()
        {
            if let Some(bytes) = expected_bytes {
                assert_eq!(search["input_bytes"], *bytes);
            }
            assert_eq!(
                search["phase"],
                if index == 0 {
                    "first-search"
                } else {
                    "changed-input"
                }
            );
            assert!(search["input_bytes"].as_u64().unwrap() <= 0x0001_0000);
            assert!(search["input_nodes"].as_u64().unwrap() <= 4096);
            assert!(search["input_depth"].as_u64().unwrap() <= 32);
        }
        if let Some(data) = authored {
            for (declaration, field, maximum) in [
                ("maximum_string", "input_bytes", 0x0001_0000_u64),
                ("maximum_collections", "input_nodes", 4096_u64),
            ] {
                if data[declaration].as_bool() == Some(true) {
                    assert_eq!(
                        searches
                            .iter()
                            .map(|row| row[field].as_u64().unwrap())
                            .max(),
                        Some(maximum)
                    );
                }
            }
        }
        calls = calls
            .checked_add(
                inputs
                    .expected
                    .len()
                    .checked_mul(count)
                    .unwrap()
                    .checked_mul(2)
                    .unwrap(),
            )
            .unwrap();
    }
    (observed_peak, calls)
}

fn assert_hostname_cache_observations(observed: &Value) {
    let callback = format_peak(["hostname"]).unwrap();
    let hostname = observed["hostname"]["searches"].as_array().unwrap();
    assert_eq!(hostname.len(), 5);
    for row in hostname {
        let valid = row["expected"].as_bool().unwrap();
        for phase in ["cold", "warm"] {
            assert_eq!(row[phase]["valid"], valid);
            let (retained, peak) = assert_cache_phase(&row[phase]["metrics"], 0, callback);
            assert_eq!(retained, 0);
            if row["input_bytes"].as_u64().unwrap() <= 255 {
                assert!(peak > 0, "callback cache is exercised");
            } else {
                assert_eq!(
                    row[phase]["metrics"],
                    json!({"allocated_bytes": 0_u64, "allocation_calls": 0_u64,
                    "peak_live_bytes": 0_i64, "retained_bytes": 0_i64})
                );
            }
        }
    }
}

fn assert_cache_phase(metrics: &Value, before: usize, bound: usize) -> (usize, usize) {
    let allocated = metrics["allocated_bytes"].as_u64().unwrap();
    let calls = metrics["allocation_calls"].as_u64().unwrap();
    let incremental_peak = usize::try_from(metrics["peak_live_bytes"].as_i64().unwrap()).unwrap();
    let delta = metrics["retained_bytes"].as_i64().unwrap();
    assert_eq!(allocated == 0, calls == 0);
    assert!(allocated >= u64::try_from(incremental_peak).unwrap());
    assert!(i64::try_from(incremental_peak).unwrap() >= delta);
    let retained =
        usize::try_from(i64::try_from(before).unwrap().checked_add(delta).unwrap()).unwrap();
    let peak = before.checked_add(incremental_peak).unwrap();
    assert!(retained <= bound, "retained {retained} exceeds {bound}");
    assert!(peak <= bound, "phase peak {peak} exceeds {bound}");
    (retained, peak)
}

fn assert_cache_population(
    row: &Value,
    owners: usize,
    bound: usize,
    expected: &[(String, Option<bool>)],
) -> usize {
    assert!(owners > 0 && !expected.is_empty());
    assert_eq!(row["owners"], owners);
    let construction = row["construction"].as_array().unwrap();
    assert_eq!(construction.len(), owners);
    let mut retained = Vec::with_capacity(owners);
    let mut maximum = 0_usize;
    for metrics in construction {
        let (live, peak) = assert_cache_phase(metrics, 0, bound);
        retained.push(live);
        maximum = maximum.max(peak);
    }
    let searches = row["searches"].as_array().unwrap();
    assert_eq!(searches.len(), expected.len());
    for (search, (id, verdict)) in searches.iter().zip(expected) {
        assert_eq!(search["id"], *id);
        assert_eq!(search["expected"], json!(verdict));
        for phase in ["cold", "warm"] {
            let results = search[phase].as_array().unwrap();
            assert_eq!(results.len(), owners);
            for (result, before) in results.iter().zip(&mut retained) {
                let actual = result["valid"].as_bool().unwrap();
                if let Some(required) = verdict {
                    assert_eq!(actual, *required, "{id} {phase}");
                }
                let (live, peak) = assert_cache_phase(&result["metrics"], *before, bound);
                *before = live;
                maximum = maximum.max(peak);
            }
        }
    }
    let release = row["release"].as_array().unwrap();
    assert_eq!(release.len(), owners);
    // The probe pops owners in reverse construction order.
    for (metrics, before) in release.iter().zip(retained.into_iter().rev()) {
        assert_eq!(metrics["allocated_bytes"], 0_u64);
        assert_eq!(metrics["allocation_calls"], 0_u64);
        let (live, peak) = assert_cache_phase(metrics, before, bound);
        assert_eq!(live, 0);
        maximum = maximum.max(peak);
    }
    maximum
}

#[cfg(test)]
mod tests {
    use super::{assert_cache_phase, assert_cache_population};
    use serde_json::{Value, json};

    #[test]
    fn fake_cache_phase_accounts_for_prior_storage_and_transient_growth_independently() {
        use std::panic::catch_unwind;
        let growth = json!({"allocated_bytes": 23_u64, "allocation_calls": 1_u64,
            "peak_live_bytes": 23_i64, "retained_bytes": 7_i64});
        assert_eq!(assert_cache_phase(&growth, 11, 34), (18, 34));
        assert_eq!(assert_cache_phase(&growth, 0, 23), (7, 23));
        assert_eq!(assert_cache_phase(&growth, 11, 35), (18, 34));
        catch_unwind(|| assert_cache_phase(&growth, 11, 33)).unwrap_err();
        let release = json!({"allocated_bytes": 0_u64, "allocation_calls": 0_u64,
            "peak_live_bytes": 0_i64, "retained_bytes": -18_i64});
        assert_eq!(assert_cache_phase(&release, 18, 18), (0, 18));
        catch_unwind(|| assert_cache_phase(&release, 17, 18)).unwrap_err();
        let zero = json!({"allocated_bytes": 0_u64, "allocation_calls": 0_u64,
            "peak_live_bytes": 0_i64, "retained_bytes": 0_i64});
        assert_eq!(assert_cache_phase(&zero, 0, 0), (0, 0));
        let mut cases = Vec::new();
        for (field, changed) in [
            ("allocated_bytes", json!(22_u64)),
            ("allocation_calls", json!(0_u64)),
            ("peak_live_bytes", json!(-1_i64)),
            ("retained_bytes", json!(24_i64)),
        ] {
            let mut changed_metrics = growth.clone();
            changed_metrics[field] = changed;
            cases.push(changed_metrics);
        }
        for changed in cases {
            catch_unwind(|| assert_cache_phase(&changed, 11, 34)).unwrap_err();
        }
        catch_unwind(|| assert_cache_phase(&growth, usize::MAX, usize::MAX)).unwrap_err();
    }

    #[test]
    fn fake_cache_population_covers_every_owner_input_verdict_and_release() {
        use std::panic::catch_unwind;
        let metrics = |allocated: u64, peak: i64, retained: i64| {
            json!({"allocated_bytes": allocated, "allocation_calls": u64::from(allocated > 0),
                "peak_live_bytes": peak, "retained_bytes": retained})
        };
        let zero = metrics(0, 0, 0);
        let search = |valid, metric: &Value| json!({"valid": valid, "metrics": metric});
        let row = json!({"owners": 2_u64,
            "construction": [metrics(5, 5, 5), metrics(7, 7, 7)],
            "searches": [
                {"id": "empty", "expected": null,
                    "cold": [search(false, &metrics(7, 7, 3)), search(false, &metrics(2, 2, 2))],
                    "warm": [search(false, &zero), search(false, &zero)]},
                {"id": "fake_case", "expected": true,
                    "cold": [search(true, &metrics(7, 7, 0)), search(true, &metrics(6, 6, 0))],
                    "warm": [search(true, &zero), search(true, &zero)]}],
            "release": [metrics(0, 0, -9), metrics(0, 0, -8)]});
        let expected = [
            ("empty".to_owned(), None),
            ("fake_case".to_owned(), Some(true)),
        ];
        assert_eq!(assert_cache_population(&row, 2, 15, &expected), 15);
        catch_unwind(|| assert_cache_population(&row, 2, 14, &expected)).unwrap_err();
        let mut mutations = Vec::new();
        for (pointer, value) in [
            ("/owners", json!(1_u64)),
            ("/construction", json!([metrics(5, 5, 5)])),
            ("/searches/0/id", json!("fake_skipped")),
            ("/searches/1/expected", json!(false)),
            ("/searches/1/cold/0/valid", json!(false)),
            ("/searches/1/warm/1/valid", json!(false)),
            ("/searches/0/warm", json!([])),
            ("/release/0/retained_bytes", json!(-8_i64)),
            ("/release/0/allocated_bytes", json!(1_u64)),
            ("/release", json!([])),
        ] {
            let mut changed = row.clone();
            *changed.pointer_mut(pointer).unwrap() = value;
            mutations.push(changed);
        }
        let mut missing = row.clone();
        missing["searches"].as_array_mut().unwrap().pop();
        mutations.push(missing);
        let mut reordered = row.clone();
        reordered["release"].as_array_mut().unwrap().reverse();
        mutations.push(reordered);
        for changed in mutations {
            catch_unwind(|| assert_cache_population(&changed, 2, 15, &expected)).unwrap_err();
        }
        catch_unwind(|| assert_cache_population(&row, 0, 15, &expected)).unwrap_err();
        catch_unwind(|| assert_cache_population(&row, 2, 15, &[])).unwrap_err();
    }
}
