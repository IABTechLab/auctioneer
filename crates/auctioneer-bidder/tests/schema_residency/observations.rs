//! Fresh reconstructed ownership windows, never physical qualification.

use serde_json::Value;

pub(super) fn observe(_programs: &[(String, String)]) -> Result<Value, String> {
    Err("constructor observation integration incomplete".into())
}

#[cfg(test)]
mod tests {
    use super::observe;
    use serde_json::json;

    #[test]
    fn reconstructed_owners_retain_distinct_equal_programs_and_release_them() {
        let programs = vec![("fake_first".into(), "^[0-9]+$".into()),
            ("fake_second".into(), "^[0-9]+$".into())];
        let report = observe(&programs).unwrap();
        let rows = report["programs"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["prefix_retained_bytes"], 0_i64);
        assert_eq!(rows[1]["prefix_retained_bytes"], rows[0]["cold"]["retained_bytes"]);
        assert_eq!(rows[0]["cold"], rows[1]["cold"]);
        assert_eq!(report["repeat"], json!({"allocated_bytes":0_u64,"peak_live_bytes":0_i64,
            "retained_bytes":0_i64,"allocation_calls":0_u64}));
        assert_eq!(report["late_size_limit_failure"]["retained_bytes"], 0_i64);
        assert_eq!(report["physical_or_generated_singleton_qualification"], false);
    }
}

