//! A bounded owner for captured parameter-test data, not a source parser.

#![cfg(test)]

use crate::schema_profile;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use sha2::{Digest as _, Sha256};
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::LazyLock;

fn revision() -> &'static str {
    if crate::SCHEMA_SOURCE_CONFORMANCE {
        "d7f8515b86258688304b0d9b6668c6a0e258bc9e"
    } else {
        "1111111111111111111111111111111111111111"
    }
}

static CAPTURE: LazyLock<Vec<u8>> = LazyLock::new(|| {
    if crate::SCHEMA_SOURCE_CONFORMANCE {
        include_bytes!("../conformance/fixtures/reference/schema-reference/parameters.json")
            .to_vec()
    } else {
        include_bytes!("../support/fake_bidder/capture.json").to_vec()
    }
});

fn capture_digest() -> &'static str {
    if crate::SCHEMA_SOURCE_CONFORMANCE {
        "6efc0eaea62063f86d391df46265e1df2fb06789cfe2e746f53c3c9e4c41863e"
    } else {
        "507ef83f93b2faa8d30380204701c12c90dc395bae6077b6d9667939d4d51d28"
    }
}

const MAX_CAPTURE_BYTES: usize = 64 * 1024;

#[derive(Deserialize)]
pub(crate) struct SourcePin {
    pub schema: String,
    pub path: String,
    pub blob: String,
    pub sha256: String,
    pub bytes: usize,
    pub valid_cases: usize,
    pub invalid_cases: usize,
    pub valid_span: Range<usize>,
    pub invalid_span: Range<usize>,
    pub verdict_counts: [usize; 3],
    pub previous: bool,
    pub original: bool,
    pub duplicates: Vec<(Group, usize, usize)>,
    pub lexical: Vec<(usize, String, String)>,
}

static SOURCES: LazyLock<Vec<SourcePin>> = LazyLock::new(|| {
    if crate::SCHEMA_SOURCE_CONFORMANCE {
        let inputs = schema_profile::inputs();
        let pins = schema_profile::declarations(&inputs.snapshot, "parameters.pin.toml");
        assert_eq!(pins.len(), 14);
        pins.into_values()
            .map(|value| serde_json::from_value(value).unwrap())
            .collect()
    } else {
        vec![
            serde_json::from_str(include_str!("../support/fake_bidder/parameters.pin.json"))
                .unwrap(),
        ]
    }
});

pub(crate) fn pins() -> &'static [SourcePin] {
    &SOURCES
}

fn family_counts() -> impl Iterator<Item = (&'static str, usize, usize)> {
    SOURCES
        .iter()
        .map(|pin| (pin.schema.as_str(), pin.valid_cases, pin.invalid_cases))
}

pub(crate) fn verdict_counts() -> BTreeMap<&'static str, [usize; 3]> {
    SOURCES
        .iter()
        .map(|pin| (pin.schema.as_str(), pin.verdict_counts))
        .collect()
}

pub(crate) fn totals() -> [usize; 3] {
    SOURCES.iter().fold([0_usize; 3], |mut total, pin| {
        for (sum, count) in total.iter_mut().zip(pin.verdict_counts) {
            *sum = sum.checked_add(count).unwrap();
        }
        total
    })
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Corpus {
    pub schema_version: u16,
    pub revision: String,
    pub sources: Vec<Source>,
    pub cases: Vec<Case>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Source {
    schema: String,
    path: String,
    blob: String,
    sha256: String,
    bytes: usize,
    valid_cases: usize,
    invalid_cases: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Group {
    Valid,
    Invalid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Case {
    pub schema: String,
    pub group: Group,
    pub index: usize,
    pub input: String,
    pub valid: bool,
    pub source_offset: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CaptureError {
    Limit,
    Integrity,
    Input,
    Version,
    Revision,
    Source,
    Cases,
    Witness,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn captured() -> Result<Corpus, CaptureError> {
    load(&CAPTURE)
}

fn load(bytes: &[u8]) -> Result<Corpus, CaptureError> {
    if bytes.len() > MAX_CAPTURE_BYTES {
        return Err(CaptureError::Limit);
    }
    if digest(bytes) != capture_digest() {
        return Err(CaptureError::Integrity);
    }
    decode(bytes)
}

fn decode(bytes: &[u8]) -> Result<Corpus, CaptureError> {
    let shape: BTreeMap<String, &RawValue> =
        serde_json::from_slice(bytes).map_err(|_error| CaptureError::Input)?;
    for field in ["sources", "cases"] {
        let array = shape.get(field).ok_or(CaptureError::Input)?;
        let _objects: Vec<BTreeMap<String, &RawValue>> =
            serde_json::from_str(array.get()).map_err(|_error| CaptureError::Input)?;
    }
    // Shape preflight must not normalize away repeated fields: the DTO decoder
    // still consumes the original bytes and owns duplicate/unknown-key refusal.
    let corpus = serde_json::from_slice(bytes).map_err(|_error| CaptureError::Input)?;
    validate(&corpus)?;
    Ok(corpus)
}

fn validate(corpus: &Corpus) -> Result<(), CaptureError> {
    if corpus.schema_version != 1 {
        return Err(CaptureError::Version);
    }
    if corpus.revision != revision() {
        return Err(CaptureError::Revision);
    }
    if corpus.sources.len() != SOURCES.len() {
        return Err(CaptureError::Source);
    }
    for (source, pin) in corpus.sources.iter().zip(SOURCES.iter()) {
        if source.schema != pin.schema
            || source.path != pin.path
            || source.blob != pin.blob
            || source.sha256 != pin.sha256
            || source.bytes != pin.bytes
            || source.valid_cases != pin.valid_cases
            || source.invalid_cases != pin.invalid_cases
        {
            return Err(CaptureError::Source);
        }
    }
    let mut cases = corpus.cases.iter();
    for pin in SOURCES.iter() {
        for (group, count, span) in [
            (Group::Valid, pin.valid_cases, &pin.valid_span),
            (Group::Invalid, pin.invalid_cases, &pin.invalid_span),
        ] {
            let mut previous_end = span.start;
            for index in 0..count {
                let row = cases.next().ok_or(CaptureError::Cases)?;
                let end = row
                    .source_offset
                    .checked_add(row.input.len())
                    .ok_or(CaptureError::Cases)?;
                if row.schema != pin.schema
                    || row.group != group
                    || row.index != index
                    || row.valid != (group == Group::Valid)
                    || row.source_offset <= previous_end
                    || end >= span.end
                    || row.input.contains('`')
                {
                    return Err(CaptureError::Cases);
                }
                previous_end = end;
            }
        }
    }
    if cases.next().is_some() {
        return Err(CaptureError::Cases);
    }
    Ok(())
}

pub(crate) fn verify_source_blob(
    corpus: &Corpus,
    schema: &str,
    bytes: &[u8],
) -> Result<(), CaptureError> {
    validate(corpus)?;
    let pin = SOURCES
        .iter()
        .find(|pin| pin.schema == schema)
        .ok_or(CaptureError::Source)?;
    if bytes.len() != pin.bytes || digest(bytes) != pin.sha256 {
        return Err(CaptureError::Witness);
    }
    for (group, span) in [
        (Group::Valid, &pin.valid_span),
        (Group::Invalid, &pin.invalid_span),
    ] {
        verify_table(
            bytes,
            span.clone(),
            corpus
                .cases
                .iter()
                .filter(|row| row.schema == schema && row.group == group),
        )?;
    }
    Ok(())
}

fn verify_table<'a>(
    bytes: &[u8],
    span: Range<usize>,
    cases: impl Iterator<Item = &'a Case>,
) -> Result<(), CaptureError> {
    if span.start >= span.end || span.end > bytes.len() {
        return Err(CaptureError::Witness);
    }
    let mut cursor = span.start;
    let mut seen = false;
    for row in cases {
        let open = row
            .source_offset
            .checked_sub(1)
            .ok_or(CaptureError::Witness)?;
        let end = row
            .source_offset
            .checked_add(row.input.len())
            .ok_or(CaptureError::Witness)?;
        let comma = end.checked_add(1).ok_or(CaptureError::Witness)?;
        let next = comma.checked_add(1).ok_or(CaptureError::Witness)?;
        // Verify known literal-table byte witnesses, never parse source code or
        // regenerate expectations. No unaccounted member may hide in a gap.
        if next > span.end
            || bytes
                .get(cursor..open)
                .is_none_or(|gap| !literal_table_gap(gap))
            || bytes.get(open) != Some(&b'`')
            || bytes.get(row.source_offset..end) != Some(row.input.as_bytes())
            || bytes.get(end) != Some(&b'`')
            || bytes.get(comma) != Some(&b',')
        {
            return Err(CaptureError::Witness);
        }
        cursor = next;
        seen = true;
    }
    if !seen
        || bytes
            .get(cursor..span.end)
            .is_none_or(|tail| !literal_table_gap(tail))
    {
        return Err(CaptureError::Witness);
    }
    Ok(())
}

fn literal_table_gap(bytes: &[u8]) -> bool {
    if !bytes.is_ascii() {
        return false;
    }
    // Only separators in the inspected, hash-pinned literal tables: whitespace
    // and newline-terminated // comments. Never skip code or another member.
    let mut iterator = bytes.iter().copied();
    while let Some(byte) = iterator.next() {
        if byte.is_ascii_whitespace() {
            continue;
        }
        if byte != b'/' || iterator.next() != Some(b'/') {
            return false;
        }
        if !iterator.by_ref().any(|comment_byte| comment_byte == b'\n') {
            return false;
        }
    }
    true
}

pub(crate) fn source_objects() -> impl Iterator<Item = (&'static str, String)> {
    SOURCES.iter().map(|source| {
        (
            source.schema.as_str(),
            format!("{}:{}", revision(), source.path),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{
        CAPTURE, CaptureError, Case, Group, MAX_CAPTURE_BYTES, SOURCES, captured, decode, digest,
        load, source_objects, verify_source_blob, verify_table,
    };
    use serde_json::{Value, error::Category, json};
    use std::iter::once;

    fn changed(change: impl FnOnce(&mut Value)) -> Vec<u8> {
        let mut value: Value = serde_json::from_slice(&CAPTURE).unwrap();
        change(&mut value);
        serde_json::to_vec(&value).unwrap()
    }

    #[test]
    fn captured_rows_retain_all_sources_groups_and_source_expectations() {
        let corpus = captured().unwrap();
        assert_eq!(
            corpus.cases.len(),
            if crate::SCHEMA_SOURCE_CONFORMANCE {
                300
            } else {
                6
            }
        );
        assert_eq!(
            corpus.cases.iter().filter(|case| case.valid).count(),
            if crate::SCHEMA_SOURCE_CONFORMANCE {
                106
            } else {
                3
            }
        );
        assert_eq!(
            corpus.cases.iter().filter(|case| !case.valid).count(),
            if crate::SCHEMA_SOURCE_CONFORMANCE {
                194
            } else {
                3
            }
        );
        for (schema, valid, invalid) in super::family_counts() {
            let rows = corpus.cases.iter().filter(|row| row.schema == schema);
            assert_eq!(
                rows.clone().filter(|row| row.valid).count(),
                valid,
                "{schema}"
            );
            assert_eq!(rows.filter(|row| !row.valid).count(), invalid, "{schema}");
        }
    }

    #[test]
    fn captured_source_dispatch_is_exact_and_contains_no_invented_tables() {
        let objects = source_objects().collect::<Vec<_>>();
        assert_eq!(objects.len(), SOURCES.len());
        for ((schema, object), pin) in objects.iter().zip(SOURCES.iter()) {
            assert_eq!(*schema, pin.schema);
            assert_eq!(*object, format!("{}:{}", super::revision(), pin.path));
        }
    }

    #[test]
    fn captured_literals_retain_duplicates_and_invalid_json_labels() {
        let corpus = captured().unwrap();
        let added = corpus
            .cases
            .iter()
            .filter(|row| {
                !SOURCES
                    .iter()
                    .find(|pin| pin.schema == row.schema)
                    .unwrap()
                    .previous
            })
            .count();
        assert_eq!(
            SOURCES
                .iter()
                .map(|pin| pin.duplicates.len())
                .sum::<usize>(),
            if crate::SCHEMA_SOURCE_CONFORMANCE {
                2
            } else {
                1
            }
        );
        assert_eq!(
            SOURCES.iter().map(|pin| pin.lexical.len()).sum::<usize>(),
            2
        );
        assert_eq!(
            added,
            if crate::SCHEMA_SOURCE_CONFORMANCE {
                64
            } else {
                0
            }
        );
        for pin in SOURCES.iter() {
            for &(group, first, second) in &pin.duplicates {
                let rows: Vec<_> = corpus
                    .cases
                    .iter()
                    .filter(|row| row.schema == pin.schema && row.group == group)
                    .collect();
                assert_eq!(rows[first].input, rows[second].input);
                assert_eq!(rows[first].index, first);
                assert_eq!(rows[second].index, second);
                assert!(rows[first].source_offset < rows[second].source_offset);
            }
            let rows: Vec<_> = corpus
                .cases
                .iter()
                .filter(|row| row.schema == pin.schema && row.group == Group::Invalid)
                .collect();
            for (index, input, category) in &pin.lexical {
                let expected = match category.as_str() {
                    "syntax" => Category::Syntax,
                    "eof" => Category::Eof,
                    other => panic!("unknown error category: {other}"),
                };
                assert_eq!(&rows[*index].input, input);
                assert!(!rows[*index].valid);
                assert_eq!(
                    serde_json::from_str::<Value>(&rows[*index].input)
                        .unwrap_err()
                        .classify(),
                    expected
                );
            }
        }
    }

    #[test]
    fn previous_rows_and_provenance_reconstruct_byte_identical_capture() {
        let mut previous: super::Corpus = serde_json::from_slice(&CAPTURE).unwrap();
        let retained = |name: &str| {
            SOURCES
                .iter()
                .find(|pin| pin.schema == name)
                .unwrap()
                .previous
        };
        previous.sources.retain(|source| retained(&source.schema));
        previous.cases.retain(|row| retained(&row.schema));
        if crate::SCHEMA_SOURCE_CONFORMANCE {
            assert_eq!(previous.sources.len(), 11);
            assert_eq!(previous.cases.len(), 236);
            assert_eq!(previous.cases.iter().filter(|row| row.valid).count(), 79);
            assert_eq!(previous.cases.iter().filter(|row| !row.valid).count(), 157);
        } else {
            assert_eq!(previous.sources.len(), 1);
            assert_eq!(previous.cases.len(), 6);
            assert_eq!(previous.cases.iter().filter(|row| row.valid).count(), 3);
            assert_eq!(previous.cases.iter().filter(|row| !row.valid).count(), 3);
        }
        let mut bytes = serde_json::to_vec_pretty(&previous).unwrap();
        bytes.push(b'\n');
        if crate::SCHEMA_SOURCE_CONFORMANCE {
            assert_eq!(bytes.len(), 48_194);
            assert_eq!(
                digest(&bytes),
                "d504554491c1c0f4fcd0f0ebf45da38fb0cc1f8e5ecfb8b4200e3cfdc144f8b0"
            );
        } else {
            assert_eq!(bytes.as_slice(), CAPTURE.as_slice());
        }
    }

    #[test]
    fn original_ninety_rows_and_provenance_are_unchanged() {
        let original: Value = serde_json::from_slice(&CAPTURE).unwrap();
        let sources = original["sources"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| {
                SOURCES
                    .iter()
                    .find(|pin| pin.schema == row["schema"].as_str().unwrap())
                    .unwrap()
                    .original
            })
            .map(|row| {
                json!([
                    row["schema"],
                    row["path"],
                    row["blob"],
                    row["sha256"],
                    row["bytes"],
                    row["valid_cases"],
                    row["invalid_cases"]
                ])
            })
            .collect::<Vec<_>>();
        let cases = original["cases"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| {
                SOURCES
                    .iter()
                    .find(|pin| pin.schema == row["schema"].as_str().unwrap())
                    .unwrap()
                    .original
            })
            .map(|row| {
                json!([
                    row["schema"],
                    row["group"],
                    row["index"],
                    row["input"],
                    row["valid"],
                    row["source_offset"]
                ])
            })
            .collect::<Vec<_>>();
        assert_eq!(
            sources.len(),
            if crate::SCHEMA_SOURCE_CONFORMANCE {
                3
            } else {
                1
            }
        );
        assert_eq!(
            cases.len(),
            if crate::SCHEMA_SOURCE_CONFORMANCE {
                90
            } else {
                6
            }
        );
        if crate::SCHEMA_SOURCE_CONFORMANCE {
            // Snapshot measured from the original capture, before adding families.
            assert_eq!(
                digest(&serde_json::to_vec(&json!({"sources": sources, "cases": cases})).unwrap()),
                "4dee25a923a2db5427303270b4c4ee7b37f8aad65d6665eb8c37bc2e31415900"
            );
        }
    }

    #[test]
    fn version_and_revision_are_fixed_independently_of_content_digests() {
        for version in [0_u16, 2] {
            assert_eq!(
                decode(&changed(|value| value["schema_version"] = json!(version))).unwrap_err(),
                CaptureError::Version
            );
        }
        for revision in [
            "0".repeat(40),
            "D7F8515B86258688304B0D9B6668C6A0E258BC9E".into(),
            "d7f8515".into(),
            "d7f8515b86258688304b0d9b6668c6a0e258bc9e-unreviewed".into(),
        ] {
            assert_eq!(
                decode(&changed(|value| value["revision"] = json!(revision))).unwrap_err(),
                CaptureError::Revision
            );
        }
    }

    #[test]
    fn every_source_field_and_exact_source_membership_is_owned() {
        let corpus = captured().unwrap();
        assert_eq!(corpus.sources.len(), SOURCES.len());
        for (source, (schema, _, _)) in super::family_counts()
            .collect::<Vec<_>>()
            .iter()
            .enumerate()
        {
            assert_eq!(corpus.sources[source].schema, *schema);
            for (field, value) in [
                ("schema", json!("unknown")),
                ("path", json!("other")),
                ("blob", json!("0".repeat(40))),
                ("sha256", json!("0".repeat(64))),
                ("bytes", json!(1_usize)),
                ("valid_cases", json!(0_usize)),
                ("invalid_cases", json!(0_usize)),
            ] {
                assert_eq!(
                    decode(&changed(|capture| capture["sources"][source][field] = value))
                        .unwrap_err(),
                    CaptureError::Source,
                    "{source}: {field}"
                );
            }
        }
        for duplicate in [false, true] {
            assert_eq!(
                decode(&changed(|capture| {
                    let sources = capture["sources"].as_array_mut().unwrap();
                    if duplicate {
                        sources.push(sources[0].clone());
                    } else {
                        sources.remove(0);
                    }
                }))
                .unwrap_err(),
                CaptureError::Source
            );
        }
    }

    #[test]
    fn each_case_requires_its_schema_group_index_boolean_and_offset() {
        assert_eq!(
            captured().unwrap().cases.len(),
            if crate::SCHEMA_SOURCE_CONFORMANCE {
                300
            } else {
                6
            }
        );
        for index in 0..captured().unwrap().cases.len() {
            for (field, value) in [
                ("schema", json!("unknown")),
                ("group", json!("other")),
                ("index", json!(99_usize)),
                ("source_offset", json!(0_usize)),
            ] {
                let error = if field == "group" {
                    CaptureError::Input
                } else {
                    CaptureError::Cases
                };
                assert_eq!(
                    decode(&changed(|capture| capture["cases"][index][field] = value)).unwrap_err(),
                    error,
                    "{index}: {field}"
                );
            }
            assert_eq!(
                decode(&changed(|capture| {
                    let valid = capture["cases"][index]["valid"].as_bool().unwrap();
                    capture["cases"][index]["valid"] = json!(!valid);
                }))
                .unwrap_err(),
                CaptureError::Cases
            );
            for field in [
                "schema",
                "group",
                "index",
                "input",
                "valid",
                "source_offset",
            ] {
                assert_eq!(
                    decode(&changed(|capture| {
                        capture["cases"][index]
                            .as_object_mut()
                            .unwrap()
                            .remove(field);
                    }))
                    .unwrap_err(),
                    CaptureError::Input,
                    "missing {index}: {field}"
                );
            }
        }
    }

    #[test]
    fn each_family_group_requires_exact_rows_and_external_expected_membership() {
        let corpus = captured().unwrap();
        for (schema, valid, invalid) in super::family_counts() {
            for (group, count, other) in [
                (Group::Valid, valid, "invalid"),
                (Group::Invalid, invalid, "valid"),
            ] {
                let positions = corpus
                    .cases
                    .iter()
                    .enumerate()
                    .filter(|(_, row)| row.schema == schema && row.group == group)
                    .map(|(position, _)| position)
                    .collect::<Vec<_>>();
                assert_eq!(positions.len(), count, "{schema} {group:?}");
                let position = positions[0];
                for operation in ["remove", "duplicate", "group", "group-and-boolean"] {
                    assert_eq!(
                        decode(&changed(|capture| {
                            let rows = capture["cases"].as_array_mut().unwrap();
                            match operation {
                                "remove" => {
                                    rows.remove(position);
                                }
                                "duplicate" => rows.insert(position, rows[position].clone()),
                                "group" => rows[position]["group"] = json!(other),
                                "group-and-boolean" => {
                                    rows[position]["group"] = json!(other);
                                    rows[position]["valid"] = json!(group != Group::Valid);
                                }
                                _ => panic!("unknown membership mutation"),
                            }
                        }))
                        .unwrap_err(),
                        CaptureError::Cases,
                        "{schema} {group:?} {operation}"
                    );
                }
            }
        }
    }

    #[test]
    fn removed_duplicated_reordered_or_renamed_case_groups_cannot_silently_skip() {
        for operation in ["remove", "duplicate", "reverse", "rename"] {
            assert_eq!(
                decode(&changed(|capture| {
                    let cases = capture["cases"].as_array_mut().unwrap();
                    match operation {
                        "remove" => {
                            cases.remove(0);
                        }
                        "duplicate" => cases.push(cases[0].clone()),
                        "reverse" => cases.reverse(),
                        "rename" => cases[0]["group"] = json!("accepted"),
                        _ => panic!("unknown test operation"),
                    }
                }))
                .unwrap_err(),
                if operation == "rename" {
                    CaptureError::Input
                } else {
                    CaptureError::Cases
                }
            );
        }
    }

    #[test]
    fn unknown_fields_are_rejected_at_each_owned_object() {
        for pointer in once(String::new())
            .chain((0_usize..SOURCES.len()).map(|index| format!("/sources/{index}")))
            .chain(
                (0_usize..captured().unwrap().cases.len()).map(|index| format!("/cases/{index}")),
            )
        {
            assert_eq!(
                decode(&changed(|capture| {
                    capture
                        .pointer_mut(&pointer)
                        .unwrap()
                        .as_object_mut()
                        .unwrap()
                        .insert("extra".into(), json!(0_i32));
                }))
                .unwrap_err(),
                CaptureError::Input
            );
        }
    }

    #[test]
    fn positional_arrays_do_not_replace_owned_object_shapes() {
        for (shape, index) in once(("root", 0))
            .chain((0..SOURCES.len()).map(|index| ("source", index)))
            .chain((0..captured().unwrap().cases.len()).map(|index| ("case", index)))
        {
            let bytes = changed(|capture| match shape {
                "root" => {
                    *capture = json!([
                        capture["schema_version"],
                        capture["revision"],
                        capture["sources"],
                        capture["cases"]
                    ]);
                }
                "source" => {
                    let row = &capture["sources"][index];
                    capture["sources"][index] = json!([
                        row["schema"],
                        row["path"],
                        row["blob"],
                        row["sha256"],
                        row["bytes"],
                        row["valid_cases"],
                        row["invalid_cases"]
                    ]);
                }
                "case" => {
                    let row = &capture["cases"][index];
                    capture["cases"][index] = json!([
                        row["schema"],
                        row["group"],
                        row["index"],
                        row["input"],
                        row["valid"],
                        row["source_offset"]
                    ]);
                }
                _ => panic!("unknown test shape"),
            });
            assert_eq!(
                decode(&bytes).unwrap_err(),
                CaptureError::Input,
                "{shape} {index}"
            );
        }
    }

    #[test]
    fn shape_preflight_preserves_duplicate_field_refusal() {
        use serde::ser::{Serialize, SerializeMap as _, Serializer};
        use serde_json::value::{RawValue, to_raw_value};
        use std::collections::BTreeMap;

        struct RepeatedField<'a>(&'a serde_json::Map<String, Value>, &'a str);
        impl Serialize for RepeatedField<'_> {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let mut map = serializer.serialize_map(None)?;
                for (key, value) in self.0 {
                    map.serialize_entry(key, value)?;
                    if key == self.1 {
                        map.serialize_entry(key, value)?;
                    }
                }
                map.end()
            }
        }

        for (location, index) in once(("root", 0))
            .chain((0..SOURCES.len()).map(|index| ("sources", index)))
            .chain((0..captured().unwrap().cases.len()).map(|index| ("cases", index)))
        {
            let original: Value = serde_json::from_slice(&CAPTURE).unwrap();
            let bytes = if location == "root" {
                serde_json::to_vec(&RepeatedField(original.as_object().unwrap(), "revision"))
                    .unwrap()
            } else {
                let mut root: BTreeMap<String, Box<RawValue>> =
                    serde_json::from_slice(&CAPTURE).unwrap();
                let mut rows: Vec<Box<RawValue>> =
                    serde_json::from_str(root[location].get()).unwrap();
                rows[index] = to_raw_value(&RepeatedField(
                    original[location][index].as_object().unwrap(),
                    "schema",
                ))
                .unwrap();
                root.insert(location.into(), to_raw_value(&rows).unwrap());
                serde_json::to_vec(&root).unwrap()
            };
            assert_eq!(
                decode(&bytes).unwrap_err(),
                CaptureError::Input,
                "{location} {index}"
            );
        }
    }

    #[test]
    fn input_cap_precedes_digest_work_and_any_unreviewed_byte_change_refuses() {
        assert_eq!(
            CAPTURE.len(),
            if crate::SCHEMA_SOURCE_CONFORMANCE {
                62_945
            } else {
                include_bytes!("../support/fake_bidder/capture.json").len()
            }
        );
        assert_eq!(MAX_CAPTURE_BYTES, 64 * 1024);
        let mut padded = CAPTURE.to_vec();
        padded.resize(MAX_CAPTURE_BYTES, b' ');
        decode(&padded).unwrap();
        assert_eq!(load(&padded).unwrap_err(), CaptureError::Integrity);
        padded.push(b' ');
        assert_eq!(load(&padded).unwrap_err(), CaptureError::Limit);
        assert_eq!(
            load(&vec![0; MAX_CAPTURE_BYTES.checked_add(1).unwrap()]).unwrap_err(),
            CaptureError::Limit
        );
        let bytes = changed(|capture| capture["cases"][0]["input"] = json!("changed"));
        assert_eq!(load(&bytes).unwrap_err(), CaptureError::Integrity);
    }

    #[test]
    fn every_captured_family_group_has_nonvacuous_byte_witness_controls() {
        let corpus = captured().unwrap();
        for (schema, valid, invalid) in super::family_counts() {
            let pin = SOURCES.iter().find(|pin| pin.schema == schema).unwrap();
            for (group, count, span) in [
                (Group::Valid, valid, &pin.valid_span),
                (Group::Invalid, invalid, &pin.invalid_span),
            ] {
                let rows = corpus
                    .cases
                    .iter()
                    .filter(|row| row.schema == schema && row.group == group)
                    .cloned()
                    .collect::<Vec<_>>();
                assert_eq!(rows.len(), count, "{schema} {group:?}");
                let mut bytes = vec![b' '; pin.bytes];
                for row in &rows {
                    let start = row.source_offset;
                    let end = start + row.input.len();
                    bytes[start - 1] = b'`';
                    bytes[start..end].copy_from_slice(row.input.as_bytes());
                    bytes[end] = b'`';
                    bytes[end + 1] = b',';
                }
                assert_eq!(verify_table(&bytes, span.clone(), rows.iter()), Ok(()));
                let target = rows.iter().position(|row| !row.input.is_empty()).unwrap();
                for change in ["input", "offset", "remove", "duplicate"] {
                    let mut changed = rows.clone();
                    match change {
                        "input" => changed[target].input.replace_range(0..1, "#"),
                        "offset" => changed[target].source_offset += 1,
                        "remove" => {
                            changed.remove(target);
                        }
                        "duplicate" => changed.insert(target, changed[target].clone()),
                        _ => panic!("unknown witness mutation"),
                    }
                    assert_eq!(
                        verify_table(&bytes, span.clone(), changed.iter()),
                        Err(CaptureError::Witness),
                        "{schema} {group:?} {change}"
                    );
                }
                // Correct length alone is not evidence for a complete pinned blob.
                assert_eq!(
                    verify_source_blob(&corpus, schema, &bytes),
                    Err(CaptureError::Witness)
                );
                assert_eq!(
                    verify_source_blob(&corpus, schema, &bytes[..bytes.len() - 1]),
                    Err(CaptureError::Witness)
                );
            }
        }
        for schema in [
            "fake-absent",
            "fake-other",
            "fake-missing",
            "fake-case",
            "unknown",
        ] {
            assert_eq!(
                verify_source_blob(&corpus, schema, &[]),
                Err(CaptureError::Source)
            );
        }
    }

    #[test]
    fn byte_witnesses_bind_literal_contents_offsets_order_and_complete_table() {
        let bytes = b" \n`alpha`,\n`omega`,\n";
        let rows = [
            Case {
                schema: "test".into(),
                group: Group::Valid,
                index: 0,
                input: "alpha".into(),
                valid: true,
                source_offset: 3,
            },
            Case {
                schema: "test".into(),
                group: Group::Valid,
                index: 1,
                input: "omega".into(),
                valid: true,
                source_offset: 12,
            },
        ];
        assert_eq!(verify_table(bytes, 0..bytes.len(), rows.iter()), Ok(()));
        for change in ["input", "offset", "missing", "order", "duplicate", "range"] {
            let mut changed = rows.to_vec();
            let mut span = 0..bytes.len();
            match change {
                "input" => changed[0].input = "wrong".into(),
                "offset" => changed[0].source_offset = 4,
                "missing" => {
                    changed.remove(0);
                }
                "order" => changed.reverse(),
                "duplicate" => changed.push(changed[0].clone()),
                "range" => span.end = 9,
                _ => panic!("unknown test mutation"),
            }
            assert_eq!(
                verify_table(bytes, span, changed.iter()),
                Err(CaptureError::Witness),
                "{change}"
            );
        }
        assert_eq!(
            verify_table(bytes, 0..bytes.len(), [].iter()),
            Err(CaptureError::Witness)
        );
    }

    #[test]
    fn literal_comment_gaps_and_tail_preserve_complete_table_witnesses() {
        let bytes = b" \n`alpha`, // between\n`omega`, // tail\n";
        let rows = [
            Case {
                schema: "test".into(),
                group: Group::Valid,
                index: 0,
                input: "alpha".into(),
                valid: true,
                source_offset: 3,
            },
            Case {
                schema: "test".into(),
                group: Group::Valid,
                index: 1,
                input: "omega".into(),
                valid: true,
                source_offset: 23,
            },
        ];
        assert_eq!(verify_table(bytes, 0..bytes.len(), rows.iter()), Ok(()));
        for insertion in [
            "`hidden`,\n",
            "build(),\n",
            "/* block comment */\n",
            "\"quoted\",\n",
            "// another comment\n`hidden`,\n",
        ] {
            let mut changed = bytes.to_vec();
            changed.splice(22..22, insertion.bytes());
            let mut shifted = rows.clone();
            shifted[1].source_offset += insertion.len();
            assert_eq!(
                verify_table(&changed, 0..changed.len(), shifted.iter()),
                Err(CaptureError::Witness),
                "gap: {insertion}"
            );
            let mut tail = bytes.to_vec();
            tail.extend_from_slice(insertion.as_bytes());
            assert_eq!(
                verify_table(&tail, 0..tail.len(), rows.iter()),
                Err(CaptureError::Witness),
                "tail: {insertion}"
            );
        }
        // A comment cannot consume a witnessed member on the same line.
        let swallowed = b" \n`alpha`, // between `omega`, // tail\n";
        assert_eq!(
            verify_table(swallowed, 0..swallowed.len(), rows.iter()),
            Err(CaptureError::Witness)
        );
    }
}
