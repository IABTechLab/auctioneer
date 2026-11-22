//! Fresh reconstructed ownership windows, not generated-singleton or physical evidence.

use crate::schema_meter::{Metrics, measure};
use regex_automata::{MatchKind, meta::Regex, util::syntax};
use serde_json::{Value, json};
use std::{hint::black_box, sync::LazyLock};

fn construct(pattern: &str) -> Option<Regex> {
    Regex::builder()
        .configure(
            Regex::config()
                .match_kind(MatchKind::LeftmostFirst)
                .utf8_empty(true)
                .nfa_size_limit(Some(0x0001_0000))
                .hybrid_cache_capacity(0x0001_0000)
                .pool_capacity(1),
        )
        .syntax(syntax::Config::default().utf8(true))
        .build(pattern)
        .ok()
}

/// Every entry is an independent singleton. The caller has already deduplicated
/// only source-proven aliases and included the one shared hostname owner.
pub(super) fn observe(programs: &[(String, String)]) -> Result<Value, String> {
    if programs.is_empty() {
        return Err("empty reconstructed program closure".into());
    }
    let (positive, control) = measure(|| black_box(vec![black_box(1_u8); 37]));
    if control
        .json()
        .get("allocated_bytes")
        .and_then(Value::as_u64)
        != Some(37_u64)
    {
        return Err("allocation control did not execute".into());
    }
    drop(positive);
    let mut cells = Vec::with_capacity(programs.len());
    let mut cold = Metrics::default();
    let mut repeat = Metrics::default();
    let mut rows = Vec::new();
    for (owner, pattern) in programs {
        let cell = LazyLock::new(|| construct(pattern));
        let (cold_success, first) = measure(|| black_box(LazyLock::force(&cell).is_some()));
        if !cold_success {
            return Err("reconstructed admitted program failed".into());
        }
        let prefix = cold
            .json()
            .get("retained_bytes")
            .and_then(Value::as_i64)
            .ok_or("invalid retained prefix")?;
        let (repeat_success, again) = measure(|| black_box(LazyLock::force(&cell).is_some()));
        if !repeat_success
            || again.json()
                != json!({"allocated_bytes":0_u64,"peak_live_bytes":0_i64,
            "retained_bytes":0_i64,"allocation_calls":0_u64})
        {
            return Err("repeated singleton construction allocated".into());
        }
        let first_json = first.json();
        if first_json
            .get("allocated_bytes")
            .and_then(Value::as_u64)
            .is_none_or(|value| value == 0)
            || first_json
                .get("retained_bytes")
                .and_then(Value::as_i64)
                .is_none_or(|value| value <= 0)
        {
            return Err("reconstructed program observation is empty".into());
        }
        rows.push(json!({"owner":owner,"prefix_retained_bytes":prefix,
            "cold":first_json,"repeat":again.json()}));
        cold.append(first);
        repeat.append(again);
        cells.push(cell);
    }
    // Recoverable size-limit failure is a separate control, not a claim that an
    // admitted fixed program fails or allocation failure can be recovered.
    let failure = LazyLock::new(|| construct("^[ab]{65536}$"));
    let (succeeded, failed) = measure(|| black_box(LazyLock::force(&failure).is_some()));
    let (cached, failed_repeat) = measure(|| black_box(LazyLock::force(&failure).is_some()));
    let failed_json = failed.json();
    if succeeded
        || cached
        || failed_json.get("retained_bytes").and_then(Value::as_i64) != Some(0_i64)
        || failed_json
            .get("allocated_bytes")
            .and_then(Value::as_u64)
            .is_none_or(|value| value == 0)
        || failed_repeat.json()
            != json!({"allocated_bytes":0_u64,"peak_live_bytes":0_i64,
            "retained_bytes":0_i64,"allocation_calls":0_u64})
    {
        return Err("late failure control did not reach bounded construction".into());
    }
    let cold_json = cold.json();
    let mut combined = cold;
    combined.append(failed);
    let mut cleanup = Metrics::default();
    // The harness Vec was allocated outside the window. Drop each actual
    // singleton in its own window rather than counting that Vec as program heap.
    while let Some(cell) = cells.pop() {
        let ((), released) = measure(|| drop(black_box(cell)));
        cleanup.append(released);
    }
    let cleanup_json = cleanup.json();
    if cold_json
        .get("retained_bytes")
        .and_then(Value::as_i64)
        .and_then(|value| {
            value.checked_add(cleanup_json.get("retained_bytes").and_then(Value::as_i64)?)
        })
        != Some(0)
    {
        return Err("reconstructed program release mismatch".into());
    }
    Ok(
        json!({"kind":"source-equivalent fresh singleton reconstruction",
        "programs":rows,"cold":cold_json,"repeat":repeat.json(),"cleanup":cleanup_json,
        "late_size_limit_failure":failed_json,"cached_failure":failed_repeat.json(),
        "prefix_plus_failure":combined.json(),"physical_or_generated_singleton_qualification":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::observe;
    use serde_json::json;

    #[test]
    fn reconstructed_owners_retain_distinct_equal_programs_and_release_them() {
        let programs = vec![
            ("fake_first".into(), "^[0-9]+$".into()),
            ("fake_second".into(), "^[0-9]+$".into()),
        ];
        let report = observe(&programs).unwrap();
        let rows = report["programs"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["prefix_retained_bytes"], 0_i64);
        assert_eq!(
            rows[1]["prefix_retained_bytes"],
            rows[0]["cold"]["retained_bytes"]
        );
        assert_eq!(rows[0]["cold"], rows[1]["cold"]);
        assert_eq!(
            report["repeat"],
            json!({"allocated_bytes":0_u64,"peak_live_bytes":0_i64,
            "retained_bytes":0_i64,"allocation_calls":0_u64})
        );
        assert_eq!(report["late_size_limit_failure"]["retained_bytes"], 0_i64);
        assert_eq!(
            report["physical_or_generated_singleton_qualification"],
            false
        );
    }
}
