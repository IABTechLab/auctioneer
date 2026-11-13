//! Fixed selected-schema expectations, not an oracle or serving qualification.

use crate::schema_selection_source::{Lane, SelectedCohort, selected_cohort};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read as _;
use std::path::{Component, Path};
use std::str;

const MAX_DOCUMENT_BYTES: u64 = 64 * 1024;
const MAX_SCHEMA_BYTES: u64 = 16 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Origin {
    AuthoredSchemaDerived,
    PinnedSourceExpectation {
        revision: String,
        source_path: String,
        source_blob: String,
        source_offset: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct Row {
    pub id: String,
    pub schema: String,
    pub input_json: String,
    pub valid: bool,
    pub origin: Origin,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Counts {
    pub authored: usize,
    pub imported: usize,
    pub accepts: usize,
    pub rejects: usize,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Batch {
    pub lane: String,
    pub rows: Vec<Row>,
    pub counts: Counts,
    pub schema_sha256: BTreeMap<String, String>,
    pub data_sha256: BTreeMap<String, String>,
    pub imported_sha256: BTreeMap<String, String>,
}

struct Document {
    rows: Vec<Row>,
    expected: Counts,
    source_cases: Option<SourceCases>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Declaration {
    schema_version: u16,
    schema: String,
    schema_sha256: String,
    origin: String,
    expected: Counts,
    rows: Vec<AuthoredRow>,
    #[serde(default, deserialize_with = "present")]
    source_cases: Option<SourceCases>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredRow {
    id: String,
    schema: String,
    valid: bool,
    #[serde(default, deserialize_with = "present")]
    input: Option<Value>,
    #[serde(default, deserialize_with = "present")]
    input_json: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceCases {
    path: String,
    sha256: String,
    revision: String,
    source_path: String,
    source_blob: String,
    accepts: usize,
    rejects: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Capture {
    schema_version: u16,
    revision: String,
    origin: String,
    source_path: String,
    source_blob: String,
    offset_unit: String,
    cases: Vec<CapturedRow>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapturedRow {
    group: String,
    index: usize,
    input: String,
    valid: bool,
    source_offset: u64,
}

// Unlike Option's default decoder, an explicitly present null is not absent.
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn read_bounded(root: &Path, relative_path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    if !relative(relative_path) {
        return Err("invalid selected semantic path".to_owned());
    }
    let mut path = root.to_owned();
    let mut components = relative_path.components().peekable();
    while let Some(component) = components.next() {
        path.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() || (components.peek().is_some() && !metadata.is_dir())
        {
            return Err("invalid selected semantic path".to_owned());
        }
    }
    let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err("selected semantic file type or size limit".to_owned());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let length = u64::try_from(bytes.len()).map_err(|error| error.to_string())?;
    if length != metadata.len() || length > limit {
        return Err("selected semantic file changed or exceeded limit".to_owned());
    }
    Ok(bytes)
}

fn counts(rows: &[Row]) -> Counts {
    Counts {
        authored: rows
            .iter()
            .filter(|row| matches!(row.origin, Origin::AuthoredSchemaDerived))
            .count(),
        imported: rows
            .iter()
            .filter(|row| matches!(row.origin, Origin::PinnedSourceExpectation { .. }))
            .count(),
        accepts: rows.iter().filter(|row| row.valid).count(),
        rejects: rows.iter().filter(|row| !row.valid).count(),
    }
}

fn add_counts(left: Counts, right: Counts) -> Result<Counts, String> {
    let add = |a: usize, b: usize| a.checked_add(b).ok_or("semantic count overflow".to_owned());
    Ok(Counts {
        authored: add(left.authored, right.authored)?,
        imported: add(left.imported, right.imported)?,
        accepts: add(left.accepts, right.accepts)?,
        rejects: add(left.rejects, right.rejects)?,
    })
}

pub(crate) fn selected(lane: Lane) -> Result<Batch, String> {
    for_cohort(&selected_cohort(lane)?)
}

pub(crate) fn for_cohort(cohort: &SelectedCohort) -> Result<Batch, String> {
    let lane = cohort
        .manifest
        .get("lane")
        .and_then(Value::as_str)
        .filter(|lane| matches!(*lane, "secure" | "development"))
        .ok_or("invalid selected semantic lane")?;
    let claimed = cohort
        .manifest
        .pointer("/selection/schemas")
        .and_then(Value::as_array)
        .ok_or("missing selected semantic membership")?;
    if cohort.manifest.get("schema_version") != Some(&Value::from(1_u8))
        || cohort.schemas.is_empty()
        || claimed.len() != cohort.schemas.len()
    {
        return Err("invalid selected semantic membership".to_owned());
    }
    let mut batch = Batch {
        lane: lane.to_owned(),
        rows: Vec::new(),
        counts: Counts::default(),
        schema_sha256: BTreeMap::new(),
        data_sha256: BTreeMap::new(),
        imported_sha256: BTreeMap::new(),
    };
    let mut ids = BTreeSet::new();
    for (schema, claim) in cohort.schemas.iter().zip(claimed) {
        let witness = &schema.witness;
        if claim.get("witness")
            != Some(&serde_json::to_value(witness).map_err(|error| error.to_string())?)
            || batch.schema_sha256.contains_key(&witness.name)
        {
            return Err("selected semantic schema identity mismatch".to_owned());
        }
        let schema_path = Path::new(&witness.path);
        let schema_bytes = read_bounded(&cohort.root, schema_path, MAX_SCHEMA_BYTES)?;
        if u64::try_from(schema_bytes.len()).map_err(|error| error.to_string())? != witness.bytes
            || digest(&schema_bytes) != witness.sha256
        {
            return Err("selected semantic schema witness changed".to_owned());
        }
        let owner = Path::new(schema_path.iter().next().ok_or("missing semantic owner")?);
        let sidecar_path = owner.join("tests/schema/semantics.toml");
        let data_bytes = read_bounded(&cohort.root, &sidecar_path, MAX_DOCUMENT_BYTES)?;
        let toml_value: toml::Value =
            toml::from_str(str::from_utf8(&data_bytes).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?;
        let value = serde_json::to_value(toml_value).map_err(|error| error.to_string())?;
        let mut document = document(&value, &witness.name, &witness.sha256)
            .map_err(|error| format!("{}: {error}", sidecar_path.display()))?;
        if let Some(pin) = &document.source_cases {
            let import_path = owner.join(&pin.path);
            let import_bytes = read_bounded(&cohort.root, &import_path, MAX_DOCUMENT_BYTES)?;
            document
                .rows
                .extend(imported(&import_bytes, &witness.name, pin)?);
            let key = import_path
                .to_str()
                .ok_or("non UTF-8 semantic import path")?;
            if batch
                .imported_sha256
                .insert(key.to_owned(), digest(&import_bytes))
                .is_some()
            {
                return Err("duplicate selected semantic import".to_owned());
            }
        }
        let actual = counts(&document.rows);
        if actual != document.expected {
            return Err(format!(
                "selected semantic counts mismatch: {}",
                witness.name
            ));
        }
        for row in &document.rows {
            if !ids.insert(row.id.clone()) {
                return Err("duplicate selected semantic case id".to_owned());
            }
        }
        batch.counts = add_counts(batch.counts, actual)?;
        batch
            .schema_sha256
            .insert(witness.name.clone(), witness.sha256.clone());
        batch
            .data_sha256
            .insert(witness.name.clone(), digest(&data_bytes));
        batch.rows.extend(document.rows);
    }
    Ok(batch)
}

fn document(value: &Value, schema: &str, sha256: &str) -> Result<Document, String> {
    if !value.is_object()
        || !value.get("expected").is_some_and(Value::is_object)
        || !value
            .get("rows")
            .and_then(Value::as_array)
            .is_some_and(|rows| rows.iter().all(Value::is_object))
        || value
            .get("source_cases")
            .is_some_and(|source| !source.is_object())
    {
        return Err("invalid selected semantic document shape".to_owned());
    }
    let declaration: Declaration =
        serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
    if declaration.schema_version != 1
        || schema.is_empty()
        || declaration.schema != schema
        || !hex(sha256, 64)
        || declaration.schema_sha256 != sha256
        || declaration.origin != "authored_schema_derived"
        || declaration.rows.is_empty()
    {
        return Err("selected semantic document identity mismatch".to_owned());
    }
    let mut ids = BTreeSet::new();
    let mut rows = Vec::new();
    for row in declaration.rows {
        if row.id.trim().is_empty() || row.schema != schema || !ids.insert(row.id.clone()) {
            return Err("invalid or duplicate authored semantic identity".to_owned());
        }
        let input_json = match (row.input, row.input_json) {
            (Some(input), None) => {
                serde_json::to_string(&input).map_err(|error| error.to_string())?
            }
            (None, Some(input)) => {
                serde_json::from_str::<Value>(&input).map_err(|error| error.to_string())?;
                input
            }
            _ => return Err("semantic case needs exactly one input or input_json".to_owned()),
        };
        rows.push(Row {
            id: format!("{schema}:authored:{}", row.id),
            schema: schema.to_owned(),
            input_json,
            valid: row.valid,
            origin: Origin::AuthoredSchemaDerived,
        });
    }
    let mut projected = counts(&rows);
    if let Some(pin) = &declaration.source_cases {
        if !relative(Path::new(&pin.path))
            || !relative(Path::new(&pin.source_path))
            || !hex(&pin.sha256, 64)
            || !hex(&pin.revision, 40)
            || !hex(&pin.source_blob, 40)
        {
            return Err("invalid selected semantic source pin".to_owned());
        }
        projected = add_counts(
            projected,
            Counts {
                authored: 0,
                imported: pin
                    .accepts
                    .checked_add(pin.rejects)
                    .filter(|count| *count > 0)
                    .ok_or("invalid semantic import count")?,
                accepts: pin.accepts,
                rejects: pin.rejects,
            },
        )?;
    }
    if projected != declaration.expected {
        return Err("selected semantic expected counts mismatch".to_owned());
    }
    Ok(Document {
        rows,
        expected: declaration.expected,
        source_cases: declaration.source_cases,
    })
}

fn imported(bytes: &[u8], schema: &str, pin: &SourceCases) -> Result<Vec<Row>, String> {
    if u64::try_from(bytes.len()).map_err(|error| error.to_string())? > MAX_DOCUMENT_BYTES
        || digest(bytes) != pin.sha256
    {
        return Err("selected semantic capture digest mismatch".to_owned());
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    if !value.is_object()
        || !value
            .get("cases")
            .and_then(Value::as_array)
            .is_some_and(|cases| cases.iter().all(Value::is_object))
    {
        return Err("invalid selected semantic capture shape".to_owned());
    }
    // Decode original bytes too, so duplicate fields cannot be normalized away.
    let capture: Capture = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    if capture.schema_version != 1
        || capture.revision != pin.revision
        || capture.source_path != pin.source_path
        || capture.source_blob != pin.source_blob
        || capture.origin != "source-test-expectations-not-executed"
        || capture.offset_unit != "zero-based-raw-byte"
    {
        return Err("selected semantic capture source mismatch".to_owned());
    }
    let mut rows = Vec::new();
    let mut accepts = 0;
    let mut rejects = 0;
    let mut previous_end = None;
    for case in capture.cases {
        let next = match case.group.as_str() {
            "valid" if case.valid && rejects == 0 => &mut accepts,
            "invalid" if !case.valid => &mut rejects,
            _ => return Err("selected semantic capture group/verdict mismatch".to_owned()),
        };
        if case.index != *next || previous_end.is_some_and(|end| case.source_offset <= end) {
            return Err("selected semantic capture order or offset mismatch".to_owned());
        }
        previous_end = Some(
            case.source_offset
                .checked_add(u64::try_from(case.input.len()).map_err(|error| error.to_string())?)
                .ok_or("semantic capture offset overflow")?,
        );
        *next = next
            .checked_add(1)
            .ok_or("semantic capture count overflow")?;
        serde_json::from_str::<Value>(&case.input).map_err(|error| error.to_string())?;
        rows.push(Row {
            id: format!("{schema}:source:{}:{}", case.group, case.index),
            schema: schema.to_owned(),
            input_json: case.input,
            valid: case.valid,
            origin: Origin::PinnedSourceExpectation {
                revision: capture.revision.clone(),
                source_path: capture.source_path.clone(),
                source_blob: capture.source_blob.clone(),
                source_offset: case.source_offset,
            },
        });
    }
    if accepts != pin.accepts || rejects != pin.rejects {
        return Err("selected semantic capture counts mismatch".to_owned());
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::{
        Counts, Lane, Origin, SelectedCohort, SourceCases, digest, document, for_cohort, imported,
        read_bounded, selected, selected_cohort,
    };
    use crate::schema_qualification::{SchemaInventory, SchemaWitness, audit_schema};
    use serde_json::{Value, json};
    use std::collections::BTreeSet;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::Path;

    fn fake_document() -> Value {
        json!({"schema_version":1_u16,"schema":"fake_owner","schema_sha256":"a".repeat(64),
            "origin":"authored_schema_derived","expected":{"authored":1_usize,"imported":0_usize,"accepts":1_usize,"rejects":0_usize},
            "rows":[{"id":"fake_valid","schema":"fake_owner","valid":true,"input_json":"{}"}]})
    }

    fn fake_capture() -> Value {
        json!({"schema_version":1_u16,"revision":"a".repeat(40),
            "origin":"source-test-expectations-not-executed",
            "source_path":"adapters/fake_owner/params_test.go","source_blob":"b".repeat(40),
            "offset_unit":"zero-based-raw-byte","cases":[
                {"group":"valid","index":0_usize,"input":" { } ","valid":true,"source_offset":7_u64},
                {"group":"invalid","index":0_usize,"input":"null","valid":false,"source_offset":40_u64}]})
    }

    fn fake_pin(bytes: &[u8]) -> SourceCases {
        SourceCases {
            path: "fixtures/parameters.json".to_owned(),
            sha256: digest(bytes),
            revision: "a".repeat(40),
            source_path: "adapters/fake_owner/params_test.go".to_owned(),
            source_blob: "b".repeat(40),
            accepts: 1,
            rejects: 1,
        }
    }

    fn fake_cohort() -> (tempfile::TempDir, SelectedCohort) {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().to_owned();
        fs::create_dir_all(root.join("fake_owner/tests/schema")).unwrap();
        let value = json!({"type":"object"});
        let bytes = serde_json::to_vec(&value).unwrap();
        fs::write(root.join("fake_owner/parameters.json"), &bytes).unwrap();
        let schema = SchemaInventory {
            witness: SchemaWitness {
                name: "fake_owner".to_owned(),
                path: "fake_owner/parameters.json".to_owned(),
                source_path: "static/bidder-params/fake_owner.json".to_owned(),
                source_blob: "fake_blob".to_owned(),
                bytes: u64::try_from(bytes.len()).unwrap(),
                sha256: digest(&bytes),
            },
            audit: audit_schema(&value).unwrap(),
        };
        let mut declaration = fake_document();
        declaration["schema_sha256"] = json!(schema.witness.sha256);
        fs::write(
            root.join("fake_owner/tests/schema/semantics.toml"),
            toml::to_string(&declaration).unwrap(),
        )
        .unwrap();
        let manifest = json!({"schema_version":1_u16,"lane":"secure",
            "selection":{"schemas":[{"witness":schema.witness}]}});
        (
            directory,
            SelectedCohort {
                root,
                schemas: vec![schema],
                manifest,
            },
        )
    }

    #[test]
    fn selected_semantics_both_lanes_have_exact_selected_owners_and_fixed_import_count() {
        // Mocktioneer's eight authored cases now belong to both build lanes.
        for lane in [Lane::Secure, Lane::Development] {
            let count = 16;
            let cohort = selected_cohort(lane).unwrap();
            let batch = selected(lane).unwrap();
            let wanted: BTreeSet<_> = cohort
                .schemas
                .iter()
                .map(|schema| schema.witness.name.as_str())
                .collect();
            let actual: BTreeSet<_> = batch.rows.iter().map(|row| row.schema.as_str()).collect();
            assert_eq!(wanted.len(), count);
            assert_eq!(actual, wanted);
            assert_eq!(batch.schema_sha256.len(), count);
            assert_eq!(batch.data_sha256.len(), count);
            assert_eq!(batch.imported_sha256.len(), 1);
            let golden = Counts {
                authored: 147,
                imported: 14,
                accepts: 61,
                rejects: 100,
            };
            assert_eq!(batch.counts, golden);
            assert_eq!(
                batch.rows.len(),
                batch.counts.authored + batch.counts.imported
            );
            assert_eq!(
                batch.rows.len(),
                batch.counts.accepts + batch.counts.rejects
            );
            assert_eq!(
                batch
                    .rows
                    .iter()
                    .map(|row| row.id.as_str())
                    .collect::<BTreeSet<_>>()
                    .len(),
                batch.rows.len()
            );
            assert_eq!(
                batch
                    .rows
                    .iter()
                    .filter(|row| matches!(row.origin, Origin::PinnedSourceExpectation { .. }))
                    .count(),
                14
            );
            for schema in &cohort.schemas {
                assert_eq!(
                    batch.schema_sha256[&schema.witness.name],
                    schema.witness.sha256
                );
            }
        }
    }

    #[test]
    fn selected_semantics_document_preserves_explicit_bool_and_both_input_forms() {
        for use_json in [false, true] {
            let mut value = fake_document();
            if !use_json {
                value["rows"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("input_json");
                value["rows"][0]["input"] = json!({});
            }
            let parsed = document(&value, "fake_owner", &"a".repeat(64)).unwrap();
            assert_eq!(parsed.rows.len(), 1);
            assert!(parsed.rows[0].valid);
            assert_eq!(parsed.rows[0].origin, Origin::AuthoredSchemaDerived);
            assert_eq!(
                serde_json::from_str::<Value>(&parsed.rows[0].input_json).unwrap(),
                json!({})
            );
            assert_eq!(parsed.expected.authored, 1);
        }
    }

    #[test]
    fn selected_semantics_document_refuses_unknown_keys_and_unbound_schema_identity() {
        for pointer in ["", "/expected", "/rows/0"] {
            let mut value = fake_document();
            value
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("fake_extra".to_owned(), json!(true));
            assert!(
                document(&value, "fake_owner", &"a".repeat(64)).is_err(),
                "{pointer}"
            );
        }
        for field in ["schema", "schema_sha256"] {
            let mut value = fake_document();
            value[field] = json!("fake_wrong_identity");
            assert!(
                document(&value, "fake_owner", &"a".repeat(64)).is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn selected_semantics_document_refuses_duplicate_ids_ambiguous_inputs_and_missing_verdicts() {
        let mut duplicate = fake_document();
        duplicate["rows"]
            .as_array_mut()
            .unwrap()
            .push(fake_document()["rows"][0].clone());
        assert!(document(&duplicate, "fake_owner", &"a".repeat(64)).is_err());
        for change in [
            "both_inputs",
            "no_input",
            "no_verdict",
            "wrong_schema",
            "invalid_json",
            "wrong_count",
        ] {
            let mut value = fake_document();
            match change {
                "both_inputs" => {
                    value["rows"][0]["input"] = json!({});
                }
                "no_input" => {
                    value["rows"][0]
                        .as_object_mut()
                        .unwrap()
                        .remove("input_json");
                }
                "no_verdict" => {
                    value["rows"][0].as_object_mut().unwrap().remove("valid");
                }
                "wrong_schema" => {
                    value["rows"][0]["schema"] = json!("fake_other_owner");
                }
                "invalid_json" => {
                    value["rows"][0]["input_json"] = json!("{");
                }
                "wrong_count" => {
                    value["expected"]["authored"] = json!(2_usize);
                }
                _ => panic!("unknown fake mutation"),
            }
            assert!(
                document(&value, "fake_owner", &"a".repeat(64)).is_err(),
                "{change}"
            );
        }
    }

    #[test]
    fn fake_document_preserves_null_false_and_raw_json() {
        let mut value = fake_document();
        value["rows"][0]["valid"] = json!(false);
        value["expected"]["accepts"] = json!(0_usize);
        value["expected"]["rejects"] = json!(1_usize);
        let raw = " \n null \t";
        value["rows"][0]["input_json"] = json!(raw);
        let parsed = document(&value, "fake_owner", &"a".repeat(64)).unwrap();
        assert_eq!(parsed.rows[0].input_json, raw);
        assert!(!parsed.rows[0].valid);
        value["rows"][0]
            .as_object_mut()
            .unwrap()
            .remove("input_json");
        value["rows"][0]["input"] = Value::Null;
        let parsed_null = document(&value, "fake_owner", &"a".repeat(64)).unwrap();
        assert_eq!(parsed_null.rows[0].input_json, "null");
    }

    #[test]
    fn fake_document_refuses_nonobjects_coercions_and_missing_contract_fields() {
        for (pointer, replacement) in [
            ("", json!([])),
            ("/expected", json!([1_usize, 0_usize, 1_usize, 0_usize])),
            ("/rows/0", json!(["fake_valid", "fake_owner", true, "{}"])),
            ("/schema_version", json!(2_u16)),
            ("/origin", json!("source-test-expectations-not-executed")),
            ("/rows/0/valid", json!("true")),
            ("/rows/0/valid", Value::Null),
            ("/rows/0/id", json!(" ")),
            ("/rows/0/input_json", Value::Null),
            ("/source_cases", Value::Null),
            ("/expected/imported", json!(1_usize)),
            ("/expected/accepts", json!(2_usize)),
            ("/expected/rejects", json!(1_usize)),
        ] {
            let mut value = fake_document();
            if pointer == "/source_cases" {
                value["source_cases"] = replacement;
            } else {
                *value.pointer_mut(pointer).unwrap() = replacement;
            }
            assert!(
                document(&value, "fake_owner", &"a".repeat(64)).is_err(),
                "{pointer}"
            );
        }
        for field in [
            "schema_version",
            "schema",
            "schema_sha256",
            "origin",
            "expected",
            "rows",
        ] {
            let mut value = fake_document();
            value.as_object_mut().unwrap().remove(field);
            assert!(
                document(&value, "fake_owner", &"a".repeat(64)).is_err(),
                "{field}"
            );
        }
        let mut both = fake_document();
        both["rows"][0]["input"] = Value::Null;
        assert!(document(&both, "fake_owner", &"a".repeat(64)).is_err());
    }

    #[test]
    fn fake_import_preserves_independent_verdicts_raw_inputs_and_source_identity() {
        let bytes = serde_json::to_vec(&fake_capture()).unwrap();
        let pin = fake_pin(&bytes);
        let rows = imported(&bytes, "fake_owner", &pin).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].input_json, " { } ");
        assert_eq!(rows[1].input_json, "null");
        assert!(rows[0].valid);
        assert!(!rows[1].valid);
        assert_ne!(rows[0].id, rows[1].id);
        for (row, offset) in rows.iter().zip([7, 40]) {
            assert_eq!(row.schema, "fake_owner");
            assert_eq!(
                row.origin,
                Origin::PinnedSourceExpectation {
                    revision: pin.revision.clone(),
                    source_path: pin.source_path.clone(),
                    source_blob: pin.source_blob.clone(),
                    source_offset: offset,
                }
            );
        }
        let mut value = fake_document();
        value["source_cases"] = json!({"path":pin.path,"sha256":pin.sha256,
            "revision":pin.revision,"source_path":pin.source_path,"source_blob":pin.source_blob,
            "accepts":1_usize,"rejects":1_usize});
        value["expected"] =
            json!({"authored":1_usize,"imported":2_usize,"accepts":2_usize,"rejects":1_usize});
        let parsed = document(&value, "fake_owner", &"a".repeat(64)).unwrap();
        assert_eq!(parsed.rows.len(), 1);
        assert_eq!(parsed.expected.imported, 2);
        value["source_cases"]["fake_extra"] = json!(true);
        assert!(document(&value, "fake_owner", &"a".repeat(64)).is_err());
    }

    #[test]
    fn fake_import_refuses_shape_metadata_count_order_and_verdict_drift() {
        for (pointer, replacement) in [
            ("", json!([])),
            ("/schema_version", json!(2_u16)),
            ("/revision", json!("c".repeat(40))),
            ("/source_path", json!("adapters/fake_other/params_test.go")),
            ("/source_blob", json!("c".repeat(40))),
            ("/origin", json!("authored_schema_derived")),
            ("/offset_unit", json!("characters")),
            ("/cases", json!([])),
            ("/cases/0", json!(["valid", 0_usize, "{}", true, 7_u64])),
            ("/cases/0/group", json!("fake_unknown")),
            ("/cases/0/index", json!(1_usize)),
            ("/cases/0/input", json!("{")),
            ("/cases/0/valid", json!(false)),
            ("/cases/0/valid", json!("true")),
            ("/cases/1/source_offset", json!(8_u64)),
            ("/cases/1/source_offset", json!(u64::MAX)),
        ] {
            let mut value = fake_capture();
            *value.pointer_mut(pointer).unwrap() = replacement;
            let bytes = serde_json::to_vec(&value).unwrap();
            assert!(
                imported(&bytes, "fake_owner", &fake_pin(&bytes)).is_err(),
                "{pointer}"
            );
        }
        for pointer in ["", "/cases/0"] {
            let mut value = fake_capture();
            value
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("fake_extra".to_owned(), json!(true));
            let bytes = serde_json::to_vec(&value).unwrap();
            assert!(
                imported(&bytes, "fake_owner", &fake_pin(&bytes))
                    .unwrap_err()
                    .contains("unknown field")
            );
        }
        let bytes = serde_json::to_vec(&fake_capture()).unwrap();
        let mut pin = fake_pin(&bytes);
        pin.sha256 = "0".repeat(64);
        assert_eq!(
            imported(&bytes, "fake_owner", &pin).unwrap_err(),
            "selected semantic capture digest mismatch"
        );
        let duplicated = String::from_utf8(bytes)
            .unwrap()
            .replacen("\"valid\":true", "\"valid\":true,\"valid\":true", 1)
            .into_bytes();
        assert!(
            imported(&duplicated, "fake_owner", &fake_pin(&duplicated))
                .unwrap_err()
                .contains("duplicate field")
        );
    }

    #[test]
    fn fake_cohort_binds_raw_sidecar_digest_and_refuses_membership_or_schema_drift() {
        let (_directory, mut cohort) = fake_cohort();
        for lane in ["secure", "development"] {
            cohort.manifest["lane"] = json!(lane);
            let batch = for_cohort(&cohort).unwrap();
            assert_eq!(batch.lane, lane);
            assert_eq!(batch.rows[0].id, "fake_owner:authored:fake_valid");
            assert_eq!(
                batch.data_sha256["fake_owner"],
                digest(
                    &fs::read(cohort.root.join("fake_owner/tests/schema/semantics.toml"),).unwrap()
                )
            );
        }
        cohort.manifest["selection"]["schemas"][0]["witness"]["name"] = json!("fake_wrong");
        assert_eq!(
            for_cohort(&cohort).unwrap_err(),
            "selected semantic schema identity mismatch"
        );
        cohort.manifest["selection"]["schemas"][0]["witness"]["name"] = json!("fake_owner");
        fs::write(
            cohort.root.join("fake_owner/parameters.json"),
            b"{\"type\":\"string\"}",
        )
        .unwrap();
        assert_eq!(
            for_cohort(&cohort).unwrap_err(),
            "selected semantic schema witness changed"
        );
    }

    #[test]
    fn fake_cohort_refuses_empty_duplicate_unknown_lane_and_escaping_paths() {
        for change in ["empty", "duplicate", "lane", "path"] {
            let (_directory, mut cohort) = fake_cohort();
            match change {
                "empty" => {
                    cohort.schemas.clear();
                    cohort.manifest["selection"]["schemas"] = json!([]);
                }
                "duplicate" => {
                    cohort.schemas.push(cohort.schemas[0].clone());
                    let claim = cohort.manifest["selection"]["schemas"][0].clone();
                    cohort.manifest["selection"]["schemas"]
                        .as_array_mut()
                        .unwrap()
                        .push(claim);
                }
                "lane" => cohort.manifest["lane"] = json!("fake_unknown_lane"),
                "path" => {
                    cohort.schemas[0].witness.path = "../fake_owner/parameters.json".to_owned();
                    cohort.manifest["selection"]["schemas"][0]["witness"]["path"] =
                        json!(cohort.schemas[0].witness.path);
                }
                _ => panic!("unknown fake mutation"),
            }
            assert!(for_cohort(&cohort).is_err(), "{change}");
        }
    }

    #[test]
    fn fake_bounded_reader_refuses_limits_traversal_and_symlinks() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir_all(directory.path().join("fake_dir")).unwrap();
        fs::write(directory.path().join("fake_dir/fake_input"), b"{}").unwrap();
        assert_eq!(
            read_bounded(directory.path(), Path::new("fake_dir/fake_input"), 2).unwrap(),
            b"{}"
        );
        for (path, limit) in [
            ("fake_dir/fake_input", 1),
            ("fake_dir", 2),
            ("../fake_input", 2),
            ("/fake_input", 2),
            ("", 2),
        ] {
            assert!(
                read_bounded(directory.path(), Path::new(path), limit).is_err(),
                "{path}"
            );
        }
        symlink("fake_dir", directory.path().join("fake_link")).unwrap();
        symlink("fake_input", directory.path().join("fake_dir/fake_link")).unwrap();
        for path in ["fake_link/fake_input", "fake_dir/fake_link"] {
            assert!(
                read_bounded(directory.path(), Path::new(path), 2).is_err(),
                "{path}"
            );
        }
    }
}
