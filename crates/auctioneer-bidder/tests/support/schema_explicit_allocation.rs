//! Phase-separated native observations, never a production resource descriptor.

use crate::schema_allocation::{Case, cases, constructors, controls, measure, shape, workloads};
use crate::{CacheFactory, CacheOwner, Validator};
use serde_json::{Value, json};
use std::hint::black_box;
use std::mem::size_of;

fn search(validate: Validator, owner: &mut CacheOwner, input: &Value) -> Value {
    let (valid, metrics) = measure(|| black_box(validate(black_box(input), black_box(owner))));
    json!({"valid": valid, "metrics": metrics.json()})
}

fn population(
    factory: CacheFactory,
    validate: Validator,
    inputs: &[Case],
    count: usize,
) -> Result<Value, &'static str> {
    // The probe's enum storage is prepared outside windows; its exact inline
    // layout is reported separately from each factory's requested heap.
    let mut owners = Vec::with_capacity(count);
    let mut construction = Vec::with_capacity(count);
    for _ in 0..count {
        let (owner, metrics) = measure(|| black_box(factory()));
        owners.push(owner.ok_or("schema cache construction failed")?);
        construction.push(metrics.json());
    }
    let mut rows = Vec::with_capacity(inputs.len());
    for (index, case) in inputs.iter().enumerate() {
        let (nodes, depth) = shape(&case.input);
        let bytes = serde_json::to_vec(&case.input).unwrap().len();
        assert!(bytes <= 65_536 && nodes <= 4096 && depth <= 32);
        let cold: Vec<_> = owners
            .iter_mut()
            .map(|owner| search(validate, owner, &case.input))
            .collect();
        let warm: Vec<_> = owners
            .iter_mut()
            .map(|owner| search(validate, owner, &case.input))
            .collect();
        rows.push(json!({
            "id": case.id, "expected": case.expected,
            "phase": if index == 0 { "first-search" } else { "changed-input" },
            "input_bytes": bytes, "input_nodes": nodes, "input_depth": depth,
            "cold": cold, "warm": warm
        }));
    }
    let mut release = Vec::with_capacity(count);
    while let Some(owner) = owners.pop() {
        let (_, metrics) = measure(|| drop(black_box(owner)));
        release.push(metrics.json());
    }
    Ok(json!({"owners": count, "construction": construction, "searches": rows, "release": release}))
}

fn hostname() -> Value {
    let longest = [63_usize; 4].map(|length| "a".repeat(length)).join(".");
    assert_eq!(longest.len(), 255);
    let inputs = [
        ("short", "a.example".to_owned(), true),
        ("max-host", longest, true),
        ("invalid-tail", format!("{}!", "a.".repeat(60)), false),
        ("overlong-label", format!("{}.co", "a".repeat(64)), false),
        ("overlong-host", "a".repeat(256), false),
    ];
    let rows: Vec<_> = inputs
        .iter()
        .map(|(id, input, expected)| {
            let invoke = || {
                let (valid, metrics) = measure(|| {
                    black_box(crate::schema_formats::reference_hostname_format(black_box(input)))
                });
                json!({"valid": valid, "metrics": metrics.json()})
            };
            let cold = invoke();
            let warm = invoke();
            json!({"id": id, "expected": expected, "input_bytes": input.len(), "cold": cold, "warm": warm})
        })
        .collect();
    json!({"scope": "separate format; fresh caller-owned cache constructed and dropped inside every search", "searches": rows})
}

pub(crate) fn report(workload: &str) -> Result<Value, &'static str> {
    if workload == "constructors" {
        return Ok(constructors());
    }
    if workload == "controls" {
        return Ok(controls());
    }
    if workload != "explicit-caches" {
        return Err("unknown explicit allocation workload");
    }
    let constructors = constructors();
    let hostname = hostname();
    let mut rows = Vec::with_capacity(crate::VALIDATORS.len());
    for (name, factory, validate) in crate::VALIDATORS {
        let mut inputs = vec![Case {
            id: "empty".into(),
            name,
            input: json!({}),
            expected: None,
        }];
        if let Some(name) = workloads().find(|candidate| candidate == name) {
            inputs.extend(cases(name));
        }
        assert!(inputs.iter().all(|case| case.name == *name));
        let mut populations = Vec::with_capacity(3);
        for count in [1, 2, 4] {
            populations.push(population(*factory, *validate, &inputs, count)?);
        }
        rows.push(json!({"name": name, "populations": populations}));
    }
    Ok(json!({
        "scope": "native requested heap; explicit probe owners, not production bounds",
        "meter": "incremental requested heap; input, report and enum-container storage excluded",
        "owner_layout": "per-schema independent enum bundles; sequential populations; not a production layout",
        "owner_inline_bytes": size_of::<CacheOwner>(),
        "constructors": constructors, "hostname": hostname, "schemas": rows
    }))
}
