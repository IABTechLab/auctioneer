//! Default-mode native observations using the shared requested-heap meter.

use crate::schema_allocation::{
    Case, Metrics, cases, constructors, controls, measure, shape, summary, workloads,
};
use serde_json::{Value, json};
use std::hint::black_box;

fn passes(cases: &[Case]) -> (Vec<(bool, Metrics)>, Vec<(bool, Metrics)>) {
    let run = || {
        let mut results = Vec::with_capacity(cases.len());
        for case in cases {
            let (_, callback) = crate::VALIDATORS
                .iter()
                .find(|(name, _)| *name == case.name)
                .unwrap();
            // Only this synchronous generated boolean call is inside the window.
            let result = measure(|| black_box(callback(black_box(&case.input))));
            results.push(result);
        }
        results
    };
    (run(), run())
}

pub(crate) fn report(workload: &str) -> Result<Value, &'static str> {
    if workload == "controls" {
        return Ok(controls());
    }
    let dispatch = || {
        crate::VALIDATORS
            .iter()
            .map(|(name, _)| Case {
                id: "empty".into(),
                name,
                input: json!({}),
                expected: None,
            })
            .collect::<Vec<_>>()
    };
    if workload == "dispatch" {
        let inputs = dispatch();
        let (cold, repeat) = passes(&inputs);
        let rows: Vec<_> = inputs
            .iter()
            .zip(&cold)
            .zip(&repeat)
            .map(|((case, first), next)| {
                json!({"name":case.name,"cold":{"valid":first.0,"metrics":first.1.json()},
                "repeat":{"valid":next.0,"metrics":next.1.json()}})
            })
            .collect();
        return Ok(json!({"scope":"empty-object dispatch only; not cache warming","rows":rows}));
    }
    let constructor_report = (workload == "initialized-catalog").then(constructors);
    let sampled = workload == "sampled-catalog" || constructor_report.is_some();
    let inputs = if sampled {
        let mut inputs = dispatch();
        for name in workloads() {
            inputs.extend(cases(name));
        }
        inputs
    } else {
        cases(
            workloads()
                .find(|name| *name == workload)
                .ok_or("unknown allocation workload")?,
        )
    };
    // All input inspection/allocation finishes before the first callback.
    let shapes: Vec<_> = inputs
        .iter()
        .map(|case| {
            let (nodes, depth) = shape(&case.input);
            let bytes = serde_json::to_vec(&case.input).unwrap().len();
            assert!(bytes <= 65_536 && nodes <= 4096 && depth <= 32);
            (bytes, nodes, depth)
        })
        .collect();
    let (first, repeat) = passes(&inputs);
    let matches = first
        .iter()
        .zip(&repeat)
        .zip(&inputs)
        .all(|((first, next), case)| {
            first.0 == next.0 && case.expected.is_none_or(|expected| expected == first.0)
        });
    if let Some(constructors) = constructor_report {
        let outcomes: Vec<_> = inputs
            .iter()
            .zip(&first)
            .zip(&repeat)
            .map(|((case, first), next)| {
                json!({
                    "name":case.name,"id":case.id,"first_valid":first.0,"repeat_valid":next.0
                })
            })
            .collect();
        return Ok(
            json!({"scope":"explicit constructors then sampled validation; not complete cache bounds",
            "constructors":constructors,"callback_inputs":inputs.len(),"all_results_match":matches,
            "outcomes":outcomes,"first_pass":summary(&first).json(),"repeat":summary(&repeat).json()}),
        );
    }
    if sampled {
        return Ok(json!({"scope":"sampled paths only; not all catalog caches",
            "dispatch_callbacks":crate::VALIDATORS.len(),"callback_inputs":inputs.len(),"all_results_match":matches,
            "first_pass":summary(&first).json(),"repeat":summary(&repeat).json()}));
    }
    let rows: Vec<_> = inputs.iter().zip(&shapes).zip(&first).zip(&repeat).enumerate()
        .map(|(index, (((case, (bytes, nodes, depth)), initial), next))| {
            json!({"id":case.id,"input_bytes":bytes,"input_nodes":nodes,"input_depth":depth,
                "expected":case.expected,"phase":if index == 0 {"cold-first-use"} else {"after-short-input"},
                "first_pass":{"valid":initial.0,"metrics":initial.1.json()},
                "repeat":{"valid":next.0,"metrics":next.1.json()}})
        }).collect();
    Ok(
        json!({"scope":"named workload first use and same-input repeat","name":workload,
        "meter":"incremental requested heap per callback; baseline caches excluded; signed retention",
        "cold_first_use":first[0].1.json(),"changed_inputs":summary(&first[1..]).json(),
        "repeat":summary(&repeat).json(),"rows":rows}),
    )
}
