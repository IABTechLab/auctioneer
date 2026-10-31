use std::collections::BTreeSet;

use anyhow::{Context as _, Result, ensure};
use serde_json::{Map, Value, json};

use super::integrity::safe_relative;

const REVISION: &str = "d7f8515b86258688304b0d9b6668c6a0e258bc9e";

pub(crate) fn field<'a>(data: &'a Value, name: &str) -> Result<&'a Value> {
    data.get(name)
        .with_context(|| format!("missing field {name}"))
}

pub(crate) fn object(data: &Value) -> Result<&Map<String, Value>> {
    data.as_object().context("expected object")
}

fn array(data: &Value) -> Result<&[Value]> {
    data.as_array().map(Vec::as_slice).context("expected array")
}

fn text(data: &Value) -> Result<&str> {
    data.as_str().context("expected string")
}

fn keys(data: &Value, names: &[&str]) -> Result<()> {
    let fields = object(data)?;
    ensure!(
        fields.len() == names.len() && names.iter().all(|name| fields.contains_key(*name)),
        "object fields do not match provenance envelope"
    );
    Ok(())
}

pub(crate) fn sha256(digest: &str) -> Result<()> {
    ensure!(
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "invalid SHA-256"
    );
    Ok(())
}

pub(crate) fn source_lock(data: &Value) -> Result<()> {
    keys(data, &["revision", "source_sha256"])?;
    ensure!(
        field(data, "revision")?.as_str() == Some(REVISION),
        "source revision mismatch"
    );
    let sources = object(field(data, "source_sha256")?)?;
    ensure!(sources.len() == 35, "source fingerprint coverage mismatch");
    for (path, digest) in sources {
        safe_relative(path)?;
        sha256(text(digest)?)?;
    }
    Ok(())
}

pub(crate) fn source_cases(data: &Value, sources: &Value) -> Result<()> {
    keys(data, &["schema_version", "reference", "cases"])?;
    ensure!(
        field(data, "schema_version")?.as_u64() == Some(1),
        "case schema version mismatch"
    );
    ensure!(
        field(data, "reference")?
            == &json!({"revision":REVISION,"license":"Apache-2.0","generator_version":1_u32,"go":"go1.23.12","now":"2026-09-30T12:00:00Z"}),
        "capture metadata mismatch"
    );
    let fingerprints = object(field(sources, "source_sha256")?)?;
    let mut ids = BTreeSet::new();
    for case in array(field(data, "cases")?)? {
        keys(
            case,
            &["id", "operation", "provenance", "input", "expected"],
        )?;
        let id = text(field(case, "id")?)?;
        ensure!(
            !id.trim().is_empty() && ids.insert(id),
            "empty or duplicate case ID"
        );
        let provenance = field(case, "provenance")?;
        keys(provenance, &["kind", "paths", "notes"])?;
        ensure!(
            field(provenance, "kind")?.as_str() == Some("source_call"),
            "captured case is not source-call evidence"
        );
        ensure!(
            !text(field(provenance, "notes")?)?.trim().is_empty(),
            "missing provenance notes"
        );
        let paths = array(field(provenance, "paths")?)?;
        ensure!(!paths.is_empty(), "missing source-call provenance");
        let mut seen = BTreeSet::new();
        for path_value in paths {
            let path = text(path_value)?;
            safe_relative(path)?;
            ensure!(
                fingerprints.contains_key(path) && seen.insert(path),
                "unfingerprinted or duplicate provenance path"
            );
        }
    }
    Ok(())
}

pub(crate) fn privacy(data: &Value) -> Result<()> {
    for (key, expected) in [
        ("prebid_revision", REVISION),
        ("go", "go1.23.12"),
        ("go_gpp", "v0.2.0"),
        ("unicode", "15.0.0"),
    ] {
        ensure!(
            field(data, key)?.as_str() == Some(expected),
            "privacy capture metadata mismatch"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sources() -> Value {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../auctioneer-bidder/tests/conformance/fixtures/usersync/sources.json"
        )))
        .unwrap()
    }

    fn cookies() -> Value {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../auctioneer-bidder/tests/conformance/fixtures/usersync/cookies.json"
        )))
        .unwrap()
    }

    fn policy() -> Value {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../auctioneer-privacy/tests/data/privacy/reference.json"
        )))
        .unwrap()
    }

    #[test]
    fn original_capture_provenance_is_accepted() {
        source_lock(&sources()).unwrap();
        source_cases(&cookies(), &sources()).unwrap();
        privacy(&policy()).unwrap();
    }

    #[test]
    fn source_lock_rejects_wrong_revisions_without_changing_fingerprints() {
        let valid = sources();
        for revision in [json!("0".repeat(40)), json!(""), json!(null), json!(1_u32)] {
            let mut data = valid.clone();
            data["revision"] = revision;
            assert_eq!(data["source_sha256"], valid["source_sha256"]);
            assert_eq!(
                source_lock(&data).unwrap_err().to_string(),
                "source revision mismatch"
            );
        }
    }

    #[test]
    fn wrong_capture_metadata_and_versions_are_rejected() {
        for field in ["revision", "go", "license", "now", "generator_version"] {
            let mut data = cookies();
            data["reference"][field] = json!("wrong");
            assert_eq!(
                source_cases(&data, &sources()).unwrap_err().to_string(),
                "capture metadata mismatch",
                "{field}"
            );
        }
        let mut wrong_version = cookies();
        wrong_version["schema_version"] = json!(2_u32);
        assert_eq!(
            source_cases(&wrong_version, &sources())
                .unwrap_err()
                .to_string(),
            "case schema version mismatch"
        );
        for field in ["prebid_revision", "go", "go_gpp", "unicode"] {
            let mut data = policy();
            data[field] = json!("wrong");
            assert_eq!(
                privacy(&data).unwrap_err().to_string(),
                "privacy capture metadata mismatch",
                "{field}"
            );
        }
    }

    #[test]
    fn missing_unfingerprinted_or_design_provenance_is_not_source_evidence() {
        for provenance in [
            json!({"kind":"source_call","paths":[],"notes":"calls"}),
            json!({"kind":"source_call","paths":["unknown.go"],"notes":"calls"}),
            json!({"kind":"source_call","paths":["../usersync/cookie.go"],"notes":"calls"}),
            json!({"kind":"auctioneer_design","paths":["usersync/cookie.go"],"notes":"calls"}),
            json!({"kind":"source_call","paths":["usersync/cookie.go"],"notes":" "}),
            json!({"kind":"source_call","paths":["usersync/cookie.go","usersync/cookie.go"],"notes":"calls"}),
        ] {
            let mut data = cookies();
            data["cases"][0]["provenance"] = provenance;
            assert!(source_cases(&data, &sources()).is_err());
        }
    }

    #[test]
    fn empty_and_duplicate_case_ids_are_rejected() {
        let original = cookies();
        for id in [json!(""), json!(" "), original["cases"][1]["id"].clone()] {
            let mut data = original.clone();
            data["cases"][0]["id"] = id;
            assert_eq!(
                source_cases(&data, &sources()).unwrap_err().to_string(),
                "empty or duplicate case ID"
            );
        }
    }

    #[test]
    fn provenance_envelopes_reject_extra_and_missing_fields() {
        for (path, diagnostic) in [
            ("", "object fields do not match provenance envelope"),
            ("/reference", "capture metadata mismatch"),
            ("/cases/0", "object fields do not match provenance envelope"),
            (
                "/cases/0/provenance",
                "object fields do not match provenance envelope",
            ),
        ] {
            let mut data = cookies();
            data.pointer_mut(path).unwrap()["extra"] = json!(true);
            assert_eq!(
                source_cases(&data, &sources()).unwrap_err().to_string(),
                diagnostic
            );
        }
        let mut extra_source_field = sources();
        extra_source_field["extra"] = json!(true);
        assert!(source_lock(&extra_source_field).is_err());
        let mut missing_notes = cookies();
        missing_notes["cases"][0]["provenance"]
            .as_object_mut()
            .unwrap()
            .remove("notes")
            .unwrap();
        assert!(source_cases(&missing_notes, &sources()).is_err());
    }

    #[test]
    fn malformed_source_fingerprints_paths_and_coverage_are_rejected() {
        for digest in ["", "abc", &"g".repeat(64)] {
            let mut data = sources();
            data["source_sha256"]["LICENSE"] = json!(digest);
            assert_eq!(
                source_lock(&data).unwrap_err().to_string(),
                "invalid SHA-256"
            );
        }
        for path in [
            "../outside",
            "/outside",
            "nested/./file",
            "file\\name",
            "C:/outside",
        ] {
            let mut data = sources();
            let fingerprints = data["source_sha256"].as_object_mut().unwrap();
            let digest = fingerprints.remove("LICENSE").unwrap();
            assert!(fingerprints.insert(path.to_owned(), digest).is_none());
            assert_eq!(fingerprints.len(), 35);
            assert_eq!(
                source_lock(&data).unwrap_err().to_string(),
                "unsafe relative path"
            );
        }
        let mut data = sources();
        data["source_sha256"]
            .as_object_mut()
            .unwrap()
            .remove("LICENSE")
            .unwrap();
        assert_eq!(
            source_lock(&data).unwrap_err().to_string(),
            "source fingerprint coverage mismatch"
        );
    }

    #[test]
    fn provenance_validation_does_not_interpret_operation_fields_or_row_counts() {
        let mut data = cookies();
        data["cases"].as_array_mut().unwrap().truncate(1);
        data["cases"][0]["operation"] = json!("consumer-owned operation");
        data["cases"][0]["input"] = json!(null);
        data["cases"][0]["expected"] = json!(["consumer-owned shape"]);
        source_cases(&data, &sources()).unwrap();
        let metadata = json!({"prebid_revision":REVISION,"go":"go1.23.12","go_gpp":"v0.2.0","unicode":"15.0.0"});
        privacy(&metadata).unwrap();
    }
}
