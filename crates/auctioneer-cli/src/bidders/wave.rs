//! Bounded design declarations, not execution evidence or review approval.

use super::Error;
use super::source::REVISION;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::str::from_utf8;

const MAX_BYTES: usize = 0x0001_0000;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Wave {
    pub schema_version: u32,
    pub kind: String,
    pub id: String,
    pub revision: String,
    pub review_status: String,
    pub shared_prerequisites: Vec<String>,
    pub bidders: Vec<Bidder>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Bidder {
    pub name: String,
    pub base: String,
    pub implementation: String,
    pub metadata_blob: String,
    pub schema_blob: String,
    pub source_files: Vec<SourceFile>,
    pub routing: String,
    pub settings: String,
    pub sync: String,
    pub operations: Vec<Operation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceFile {
    pub path: String,
    pub blob: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Operation {
    pub id: String,
    pub driver: String,
    pub assertion: String,
    pub shared_prerequisites: Vec<String>,
}

impl Wave {
    pub(super) fn operation_count(&self) -> usize {
        self.bidders
            .iter()
            .map(|bidder| bidder.operations.len())
            .sum()
    }
}

pub(super) fn parse(bytes: &[u8]) -> Result<Wave, Error> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::WaveInput);
    }
    let text = from_utf8(bytes).map_err(|_error| Error::WaveInput)?;
    admit_toml(text)?;
    let wave: Wave = toml::from_str(text).map_err(|_error| Error::WaveInput)?;
    validate(&wave)?;
    Ok(wave)
}

fn admit_toml(text: &str) -> Result<(), Error> {
    if text.starts_with('\u{feff}') {
        return Err(Error::WaveInput);
    }
    // Admission only: the maintained parser still validates TOML grammar.
    // Fixed header paths and value nesting also bound the constructed tree.
    let mut frames = [0_u8; 8];
    let mut depth = 0_usize;
    for raw_line in text.lines() {
        let line = raw_line.trim_start_matches([' ', '\t']);
        if depth == 0 && line.starts_with('[') {
            let header = line
                .split_once('#')
                .map_or(line, |(prefix, _)| prefix)
                .trim_end();
            if !matches!(
                header,
                "[[bidders]]" | "[[bidders.operations]]" | "[[bidders.source_files]]"
            ) {
                return Err(Error::WaveInput);
            }
            continue;
        }
        admit_line(line, &mut frames, &mut depth)?;
    }
    if depth != 0 {
        return Err(Error::WaveInput);
    }
    Ok(())
}

fn admit_line(line: &str, frames: &mut [u8; 8], depth: &mut usize) -> Result<(), Error> {
    let mut bytes = line.bytes().peekable();
    while let Some(byte) = bytes.next() {
        match byte {
            b'#' => break,
            b'.' => return Err(Error::WaveInput),
            b'[' | b'{' => {
                *frames.get_mut(*depth).ok_or(Error::WaveInput)? = byte;
                *depth = depth.checked_add(1).ok_or(Error::WaveInput)?;
            }
            b']' | b'}' => {
                *depth = depth.checked_sub(1).ok_or(Error::WaveInput)?;
                let opener = if byte == b']' { b'[' } else { b'{' };
                if frames.get(*depth).copied() != Some(opener) {
                    return Err(Error::WaveInput);
                }
            }
            b'\'' | b'"' => {
                let mut closed = false;
                while let Some(next) = bytes.next() {
                    if next == byte {
                        closed = true;
                        break;
                    }
                    if next == b'\\' && byte == b'"' && bytes.next().is_none() {
                        return Err(Error::WaveInput);
                    }
                }
                if !closed || bytes.peek() == Some(&byte) {
                    return Err(Error::WaveInput);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate(wave: &Wave) -> Result<(), Error> {
    if wave.schema_version != 1
        || wave.kind != "priority_display_design"
        || !token(&wave.id)
        || wave.revision != REVISION
        || wave.review_status != "pending"
        || !(1..=512).contains(&wave.bidders.len())
        || wave.operation_count() > 1024
    {
        return Err(Error::WaveInput);
    }
    let mut prerequisites = BTreeSet::new();
    for prerequisite in &wave.shared_prerequisites {
        if !token(prerequisite) || !prerequisites.insert(prerequisite.as_str()) {
            return Err(Error::WaveInput);
        }
    }
    let mut names = BTreeSet::new();
    for bidder in &wave.bidders {
        if !token(&bidder.name) || !names.insert(bidder.name.to_ascii_lowercase()) {
            return Err(Error::WaveInput);
        }
        validate_bidder(bidder, &prerequisites)?;
    }
    Ok(())
}

fn validate_bidder(bidder: &Bidder, prerequisites: &BTreeSet<&str>) -> Result<(), Error> {
    if !token(&bidder.base)
        || !token(&bidder.implementation)
        || !blob(&bidder.metadata_blob)
        || !blob(&bidder.schema_blob)
        || !(1..=16).contains(&bidder.source_files.len())
        || !matches!(
            bidder.routing.as_str(),
            "https_review_pending" | "https_default_pending" | "regional_credentials_required"
        )
        || !text(&bidder.settings)
        || !matches!(
            bidder.sync.as_str(),
            "declared" | "supports_only" | "absent" | "inherited_key"
        )
        || !(1..=64).contains(&bidder.operations.len())
    {
        return Err(Error::WaveInput);
    }
    let mut previous = None;
    for file in &bidder.source_files {
        if !source_path(&file.path)
            || !blob(&file.blob)
            || previous.is_some_and(|path: &str| path >= file.path.as_str())
        {
            return Err(Error::WaveInput);
        }
        previous = Some(file.path.as_str());
    }
    let mut ids = BTreeSet::new();
    for operation in &bidder.operations {
        if !token(&operation.id)
            || !ids.insert(operation.id.as_str())
            || !driver(&operation.driver)
            || !text(&operation.assertion)
            || operation
                .shared_prerequisites
                .iter()
                .any(|reference| !prerequisites.contains(reference.as_str()))
        {
            return Err(Error::WaveInput);
        }
    }
    Ok(())
}

fn token(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn blob(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 1024
        && !value.bytes().any(|byte| byte.is_ascii_control())
}

fn driver(value: &str) -> bool {
    // Labels identify future Rust tests; they are never executable commands.
    value
        .split_once("::")
        .is_some_and(|(target, test)| token(target) && token(test))
}

fn source_path(value: &str) -> bool {
    // Canonical source-directory ownership is reconciled by the caller.
    let mut components = value.split('/');
    components.next() == Some("adapters")
        && components.next().is_some_and(token)
        && components
            .next()
            .and_then(|name| name.strip_suffix(".go"))
            .is_some_and(token)
        && components.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use toml::Value;
    use toml::map::Map;

    fn fixture() -> Value {
        let text = format!(
            r#"
schema_version = 1
kind = "priority_display_design"
id = "example-wave_1"
revision = "{REVISION}"
review_status = "pending"
shared_prerequisites = ["core_gate"]

[[bidders]]
name = "fake_example"
base = "fake_example_base"
implementation = "fake_example_impl"
metadata_blob = "0123456789abcdef0123456789abcdef01234567"
schema_blob = "abcdef0123456789abcdef0123456789abcdef01"
routing = "https_review_pending"
settings = "Review display settings."
sync = "supports_only"
source_files = [
  {{ path = "adapters/fake_exampleBase/fake_example.go", blob = "0123456789abcdef0123456789abcdef01234567" }},
  {{ path = "adapters/fake_exampleBase/fake_example_test.go", blob = "abcdef0123456789abcdef0123456789abcdef01" }}
]

[[bidders.operations]]
id = "display_check"
driver = "auctioneer-bidder::fake_example"
assertion = "Display request and response mappings need Rust tests."
shared_prerequisites = ["core_gate"]
"#
        );
        toml::from_str(&text).unwrap()
    }

    fn encode(value: &Value) -> Vec<u8> {
        toml::to_string(value).unwrap().into_bytes()
    }

    fn reject(value: &Value) {
        assert_eq!(parse(&encode(value)).err(), Some(Error::WaveInput));
    }

    fn object(value: &mut Value, layer: usize) -> &mut Map<String, Value> {
        match layer {
            0 => value.as_table_mut().unwrap(),
            1 => value["bidders"][0].as_table_mut().unwrap(),
            2 => value["bidders"][0]["source_files"][0]
                .as_table_mut()
                .unwrap(),
            3 => value["bidders"][0]["operations"][0].as_table_mut().unwrap(),
            _ => panic!("invalid fixture layer"),
        }
    }

    fn validate_value(value: &Value) -> Result<(), Error> {
        // The byte cap makes the largest structural bounds unreachable via parse.
        let encoded = String::from_utf8(encode(value)).map_err(|_error| Error::WaveInput)?;
        let wave: Wave = toml::from_str(&encoded).map_err(|_error| Error::WaveInput)?;
        validate(&wave)
    }

    fn operations(count: usize) -> Value {
        let template = fixture()["bidders"][0]["operations"][0].clone();
        Value::Array(
            (0..count)
                .map(|index| {
                    let mut operation = template.clone();
                    operation["id"] = Value::String(format!("operation_{index}"));
                    operation
                })
                .collect(),
        )
    }

    fn bidders(count: usize, operation_count: usize) -> Value {
        let mut template = fixture()["bidders"][0].clone();
        template["operations"] = operations(operation_count);
        Value::Array(
            (0..count)
                .map(|index| {
                    let mut bidder = template.clone();
                    bidder["name"] = Value::String(format!("fake_bidder_{index}"));
                    bidder
                })
                .collect(),
        )
    }

    #[test]
    fn parses_design_fields_and_counts_every_operation() {
        let mut value = fixture();
        value["bidders"] = bidders(2, 2);
        value["bidders"][1]["operations"] = operations(1);
        let wave = parse(&encode(&value)).unwrap();
        assert_eq!(wave.schema_version, 1);
        assert_eq!(wave.kind, "priority_display_design");
        assert_eq!(wave.id, "example-wave_1");
        assert_eq!(wave.revision, REVISION);
        assert_eq!(wave.review_status, "pending");
        assert_eq!(wave.shared_prerequisites, ["core_gate"]);
        assert_eq!(wave.bidders.len(), 2);
        assert_eq!(wave.operation_count(), 3);
        assert_eq!(wave.bidders[0].name, "fake_bidder_0");
        assert_eq!(wave.bidders[0].base, "fake_example_base");
        assert_eq!(wave.bidders[0].implementation, "fake_example_impl");
        assert_eq!(wave.bidders[0].metadata_blob.len(), 40);
        assert_eq!(wave.bidders[0].schema_blob.len(), 40);
        assert_eq!(wave.bidders[0].source_files.len(), 2);
        assert_eq!(wave.bidders[0].routing, "https_review_pending");
        assert_eq!(wave.bidders[0].settings, "Review display settings.");
        assert_eq!(wave.bidders[0].sync, "supports_only");
        assert_eq!(wave.bidders[0].operations[0].id, "operation_0");
        assert_eq!(
            wave.bidders[0].operations[0].driver,
            "auctioneer-bidder::fake_example"
        );
        assert_eq!(
            wave.bidders[0].operations[0].shared_prerequisites,
            ["core_gate"]
        );
    }

    #[test]
    fn source_http_routing_has_a_pending_https_default_not_a_vendor_review_gate() {
        let mut value = fixture();
        value["bidders"][0]["routing"] = "https_default_pending".into();
        let wave = parse(&encode(&value)).unwrap();
        assert_eq!(wave.bidders[0].routing, "https_default_pending");
        assert_eq!(wave.review_status, "pending");
        value["bidders"][0]["routing"] = "http_replacement_required".into();
        assert_eq!(parse(&encode(&value)).unwrap_err(), Error::WaveInput);
    }

    #[test]
    fn committed_priority_contract_passes_the_shared_parser() {
        use sha2::{Digest as _, Sha256};
        let bytes = include_bytes!(
            "../../../auctioneer-bidder/tests/conformance/fixtures/waves/priority.toml"
        );
        assert_eq!(
            hex::encode(Sha256::digest(bytes)),
            "aa9d38e3e6221c13d13b28ba18684e041f48e3c15608aa60ee11db86924a0c24"
        );
        let wave = parse(bytes).unwrap();
        assert_eq!(wave.review_status, "pending");
        assert_eq!(wave.bidders.len(), 13);
        assert_eq!(wave.operation_count(), 113);
    }

    #[test]
    fn accepts_exact_byte_cap_and_rejects_one_extra_byte() {
        let mut bytes = encode(&fixture());
        assert!(bytes.len() < 0x0001_0000);
        bytes.resize(0x0001_0000, b' ');
        assert_eq!(parse(&bytes).unwrap().operation_count(), 1);
        bytes.push(b' ');
        assert_eq!(parse(&bytes).err(), Some(Error::WaveInput));
    }

    #[test]
    fn admission_bounds_combined_containers_before_decoding() {
        for depth in [7_usize, 8, 9] {
            for layout in 0_usize..3 {
                let mut nested = "0".to_owned();
                for level in 0..depth {
                    nested = if layout == 0 || (layout == 2 && level.is_multiple_of(2)) {
                        format!("[{nested}]")
                    } else {
                        format!("{{ value = {nested} }}")
                    };
                }
                let input = format!("value = {nested}");
                assert_eq!(
                    admit_toml(&input),
                    if depth <= 8 {
                        Ok(())
                    } else {
                        Err(Error::WaveInput)
                    },
                    "depth {depth}, layout {layout}"
                );
            }
        }
        for input in [
            "value = [{ nested = [0] }]",
            "value = '[{a.b}]' # ignored. [ {",
            "value = \"escaped \\\" [{a.b}]\"",
            "[[bidders.operations]] # a.b [ {",
        ] {
            assert_eq!(admit_toml(input), Ok(()), "{input}");
        }
        for input in [
            "value = [{ nested = [0] }",
            "value = [}",
            "a.b = 1",
            "[bidders.unknown]",
            "value = 'unterminated",
        ] {
            assert_eq!(admit_toml(input), Err(Error::WaveInput), "{input}");
        }
    }

    #[test]
    fn wave_decoding_enforces_the_documented_syntax_subset() {
        let encoded = String::from_utf8(encode(&fixture())).unwrap();
        for (from, to) in [
            ("[[bidders]]", "[[\"bidders\"]]"),
            (
                "settings = \"Review display settings.\"",
                "settings = \"\"\"Review display settings.\"\"\"",
            ),
        ] {
            assert_eq!(encoded.matches(from).count(), 1);
            let alternate = encoded.replace(from, to);
            let decoded: Wave = toml::from_str(&alternate).unwrap();
            assert_eq!(validate(&decoded), Ok(()));
            assert_eq!(parse(alternate.as_bytes()).err(), Some(Error::WaveInput));
        }
    }

    #[test]
    fn a_byte_order_mark_cannot_bypass_header_admission() {
        let input = "\u{feff}[[\"bidders\"]]";
        assert_eq!(admit_toml(input), Err(Error::WaveInput));
    }

    #[test]
    fn malformed_input_has_only_the_fixed_error() {
        for bytes in [
            b"".as_slice(),
            b"not TOML: secret",
            b"\xff",
            b"schema_version = 1\nschema_version = 1",
            b"id = 'private-input-marker'",
        ] {
            assert_eq!(parse(bytes).err(), Some(Error::WaveInput));
        }
        assert_eq!(Error::WaveInput.code(), "bidder_wave_input");
        assert_eq!(
            format!("{:?}", parse(b"private-input-marker").unwrap_err()),
            "WaveInput"
        );
    }

    #[test]
    fn rejects_unknown_fields_in_every_object() {
        let mut checked = 0_usize;
        for layer in 0..4 {
            let mut value = fixture();
            object(&mut value, layer).insert("evidence_approved".to_owned(), Value::Boolean(true));
            reject(&value);
            checked += 1;
        }
        assert_eq!(checked, 4);
    }

    #[test]
    fn rejects_missing_fields_and_wrong_types_in_every_object() {
        let mut checked = 0_usize;
        for layer in 0..4 {
            let keys: Vec<_> = object(&mut fixture(), layer).keys().cloned().collect();
            assert!(!keys.is_empty());
            for key in keys {
                let mut missing = fixture();
                object(&mut missing, layer).remove(&key).unwrap();
                reject(&missing);
                let mut wrong_type = fixture();
                object(&mut wrong_type, layer).insert(key, Value::Boolean(false));
                reject(&wrong_type);
                checked += 1;
            }
        }
        assert_eq!(checked, 23);
    }

    #[test]
    fn rejects_wrong_list_member_types() {
        let mut checked = 0_usize;
        for (layer, field) in [
            (0, "shared_prerequisites"),
            (0, "bidders"),
            (1, "source_files"),
            (1, "operations"),
            (3, "shared_prerequisites"),
        ] {
            let mut value = fixture();
            object(&mut value, layer)
                .insert(field.to_owned(), Value::Array(vec![Value::Integer(1)]));
            reject(&value);
            checked += 1;
        }
        assert_eq!(checked, 5);
    }

    #[test]
    fn rejects_duplicate_toml_fields_in_every_object() {
        let text = String::from_utf8(encode(&fixture())).unwrap();
        let mut checked = 0_usize;
        for line in [
            "review_status = \"pending\"",
            "routing = \"https_review_pending\"",
            "path = \"adapters/fake_exampleBase/fake_example.go\"",
            "driver = \"auctioneer-bidder::fake_example\"",
        ] {
            assert_eq!(text.matches(line).count(), 1);
            let duplicated = text.replace(line, &format!("{line}\n{line}"));
            assert_eq!(parse(duplicated.as_bytes()).err(), Some(Error::WaveInput));
            checked += 1;
        }
        assert_eq!(checked, 4);
    }

    #[test]
    fn requires_the_exact_design_header_and_pinned_revision() {
        for version in [-1, 0, 2, 0x0001_0000_0000] {
            let mut value = fixture();
            value["schema_version"] = Value::Integer(version);
            reject(&value);
        }
        for (field, invalid) in [
            ("kind", "priority_display"),
            ("kind", "priority_display_design "),
            ("review_status", "approved"),
            ("review_status", "migrated"),
            ("review_status", "Pending"),
            ("revision", "0123456789abcdef0123456789abcdef01234567"),
            ("revision", ""),
        ] {
            let mut value = fixture();
            value[field] = Value::String(invalid.to_owned());
            reject(&value);
        }
        let mut value = fixture();
        value["revision"] = Value::String(REVISION.to_ascii_uppercase());
        reject(&value);
    }

    #[test]
    fn bounds_all_identity_tokens_and_operation_ids() {
        let mut checked = 0_usize;
        for (layer, field) in [
            (0, "id"),
            (1, "name"),
            (1, "base"),
            (1, "implementation"),
            (3, "id"),
        ] {
            // Short and leading-uppercase tokens exercise the lexical guard, not a roster.
            for valid in [
                "a".to_owned(),
                "A_9-z".to_owned(),
                format!("fake_{}", "x".repeat(59)),
            ] {
                let mut value = fixture();
                object(&mut value, layer).insert(field.to_owned(), Value::String(valid));
                parse(&encode(&value)).unwrap();
            }
            for invalid in [
                String::new(),
                "x".repeat(65),
                "two words".to_owned(),
                "a.b".to_owned(),
                "a/b".to_owned(),
                "a:b".to_owned(),
                "\u{e9}".to_owned(),
                "x\n".to_owned(),
            ] {
                let mut value = fixture();
                object(&mut value, layer).insert(field.to_owned(), Value::String(invalid));
                reject(&value);
                checked += 1;
            }
        }
        assert_eq!(checked, 40);
    }

    #[test]
    fn bidder_names_are_unique_under_ascii_casefold() {
        for duplicate in ["fake_example", "fake_EXAMPLE", "fake_ExAmPlE"] {
            let mut value = fixture();
            let mut second = value["bidders"][0].clone();
            second["name"] = Value::String(duplicate.to_owned());
            value["bidders"].as_array_mut().unwrap().push(second);
            reject(&value);
        }
    }

    #[test]
    fn bidder_count_is_between_one_and_512_without_a_roster() {
        let mut value = fixture();
        value["bidders"] = bidders(0, 1);
        reject(&value);
        value["bidders"] = bidders(14, 1);
        assert_eq!(parse(&encode(&value)).unwrap().bidders.len(), 14);
        value["bidders"] = bidders(512, 1);
        assert_eq!(validate_value(&value), Ok(()));
        value["bidders"] = bidders(513, 1);
        assert_eq!(validate_value(&value), Err(Error::WaveInput));
    }

    #[test]
    fn operations_are_nonempty_and_bounded_per_bidder() {
        let mut value = fixture();
        value["bidders"][0]["operations"] = operations(0);
        reject(&value);
        value["bidders"][0]["operations"] = operations(64);
        assert_eq!(parse(&encode(&value)).unwrap().operation_count(), 64);
        value["bidders"][0]["operations"] = operations(65);
        reject(&value);
    }

    #[test]
    fn total_operations_are_bounded_independently_of_per_bidder_counts() {
        let mut value = fixture();
        value["bidders"] = bidders(16, 64);
        assert_eq!(validate_value(&value), Ok(()));
        value["bidders"].as_array_mut().unwrap().push({
            let mut extra = fixture()["bidders"][0].clone();
            extra["name"] = Value::String("fake_extra".to_owned());
            extra
        });
        assert_eq!(validate_value(&value), Err(Error::WaveInput));
    }

    #[test]
    fn operation_ids_are_unique_only_within_each_bidder() {
        let mut value = fixture();
        value["bidders"] = bidders(2, 1);
        assert_eq!(parse(&encode(&value)).unwrap().operation_count(), 2);
        let duplicate = value["bidders"][0]["operations"][0].clone();
        value["bidders"][0]["operations"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        reject(&value);
    }

    #[test]
    fn validates_every_source_and_schema_blob_as_lowercase_sha1() {
        let mut checked = 0_usize;
        for (layer, field) in [(1, "metadata_blob"), (1, "schema_blob"), (2, "blob")] {
            for invalid in [
                String::new(),
                "a".repeat(39),
                "a".repeat(41),
                "A".repeat(40),
                "g".repeat(40),
                format!("{} ", "a".repeat(39)),
            ] {
                let mut value = fixture();
                object(&mut value, layer).insert(field.to_owned(), Value::String(invalid));
                reject(&value);
                checked += 1;
            }
        }
        assert_eq!(checked, 18);
        let mut value = fixture();
        value["bidders"][0]["source_files"][1]["blob"] = Value::String("bad".to_owned());
        reject(&value);
    }

    #[test]
    fn source_paths_are_clean_immediate_go_files_not_inferred_from_internal_key() {
        for valid in [
            "adapters/fake_Bidder/fake_Bidder.go".to_owned(),
            "adapters/fake_a/fake_a_test.go".to_owned(),
            format!(
                "adapters/fake_{}/fake_{}.go",
                "a".repeat(59),
                "b".repeat(59)
            ),
        ] {
            let mut value = fixture();
            value["bidders"][0]["source_files"]
                .as_array_mut()
                .unwrap()
                .truncate(1);
            value["bidders"][0]["source_files"][0]["path"] = Value::String(valid);
            parse(&encode(&value)).unwrap();
        }
        let mut checked = 0_usize;
        for invalid in [
            "",
            "/adapters/a/a.go",
            "adapters//a.go",
            "adapters/./a.go",
            "adapters/../a.go",
            "adapters/a/../a.go",
            "adapters/a/sub/a.go",
            "adapters/a.go",
            "other/a/a.go",
            "adapters/a/a.yaml",
            "adapters/a/a.GO",
            "adapters/a/.go",
            "adapters/a/..go",
            "adapters/a/./a.go",
            "adapters/a\\b/a.go",
            "adapters/a/a.go/",
            "adapters/a/a.go\n",
            "adapters/a b/a.go",
            "adapters/%2e%2e/a.go",
            "static/bidder-info/example.yaml",
        ] {
            let mut value = fixture();
            value["bidders"][0]["source_files"]
                .as_array_mut()
                .unwrap()
                .truncate(1);
            value["bidders"][0]["source_files"][0]["path"] = Value::String(invalid.to_owned());
            reject(&value);
            checked += 1;
        }
        assert_eq!(checked, 20);
    }

    #[test]
    fn source_files_are_nonempty_bounded_sorted_and_unique() {
        let mut value = fixture();
        let template = value["bidders"][0]["source_files"][0].clone();
        value["bidders"][0]["source_files"] = Value::Array(Vec::new());
        reject(&value);
        let files: Vec<_> = (0_usize..16)
            .map(|index| {
                let mut file = template.clone();
                file["path"] = Value::String(format!("adapters/fake_example/f{index:02}.go"));
                file
            })
            .collect();
        value["bidders"][0]["source_files"] = Value::Array(files);
        assert_eq!(
            parse(&encode(&value)).unwrap().bidders[0]
                .source_files
                .len(),
            16
        );
        let mut extra = template;
        extra["path"] = Value::String("adapters/fake_example/f16.go".to_owned());
        value["bidders"][0]["source_files"]
            .as_array_mut()
            .unwrap()
            .push(extra);
        reject(&value);
        let mut unsorted = fixture();
        unsorted["bidders"][0]["source_files"]
            .as_array_mut()
            .unwrap()
            .reverse();
        reject(&unsorted);
        let mut duplicate = fixture();
        duplicate["bidders"][0]["source_files"][1] =
            duplicate["bidders"][0]["source_files"][0].clone();
        reject(&duplicate);
    }

    #[test]
    fn routing_and_sync_are_exact_design_declarations() {
        for (field, allowed) in [
            (
                "routing",
                [
                    "https_review_pending",
                    "https_default_pending",
                    "regional_credentials_required",
                ]
                .as_slice(),
            ),
            (
                "sync",
                ["declared", "supports_only", "absent", "inherited_key"].as_slice(),
            ),
        ] {
            for valid in allowed {
                let mut value = fixture();
                value["bidders"][0][field] = Value::String((*valid).to_owned());
                parse(&encode(&value)).unwrap();
                for invalid in [format!("{valid} "), valid.to_ascii_uppercase()] {
                    value["bidders"][0][field] = Value::String(invalid);
                    reject(&value);
                }
            }
            for invalid in ["", "approved", "executed", "other"] {
                let mut value = fixture();
                value["bidders"][0][field] = Value::String(invalid.to_owned());
                reject(&value);
            }
        }
    }

    #[test]
    fn settings_and_assertions_are_nonblank_bounded_text() {
        for (layer, field) in [(1, "settings"), (3, "assertion")] {
            for valid in ["x".to_owned(), "x".repeat(1024), "\u{e9}".repeat(512)] {
                let mut value = fixture();
                object(&mut value, layer).insert(field.to_owned(), Value::String(valid));
                parse(&encode(&value)).unwrap();
            }
            for invalid in [
                String::new(),
                "   ".to_owned(),
                "x".repeat(1025),
                "\u{e9}".repeat(513),
                "x\0".to_owned(),
                "x\u{7f}".to_owned(),
            ] {
                let mut value = fixture();
                object(&mut value, layer).insert(field.to_owned(), Value::String(invalid));
                reject(&value);
            }
        }
    }

    #[test]
    fn driver_is_a_two_segment_test_label_not_a_command_or_path() {
        for valid in [
            "auctioneer-bidder::fake_bidder".to_owned(),
            "auctioneer-core::bidder_onboarding".to_owned(),
            format!("{}::{}", "x".repeat(64), "y".repeat(64)),
        ] {
            let mut value = fixture();
            value["bidders"][0]["operations"][0]["driver"] = Value::String(valid);
            parse(&encode(&value)).unwrap();
        }
        let mut checked = 0_usize;
        for invalid in [
            String::new(),
            "cargo test".to_owned(),
            "cargo::test;id".to_owned(),
            "sh::$(id)".to_owned(),
            "a::b|c".to_owned(),
            "a::b&c".to_owned(),
            "a::b`id`".to_owned(),
            "a::b\nc".to_owned(),
            "a::b c".to_owned(),
            "a::b/c".to_owned(),
            "a::b.c".to_owned(),
            "a::..".to_owned(),
            "a".to_owned(),
            "a:b".to_owned(),
            "::a".to_owned(),
            "a::".to_owned(),
            "a:::b".to_owned(),
            "a::b::c".to_owned(),
            format!("{}::b", "a".repeat(65)),
            format!("a::{}", "b".repeat(65)),
            "a::\u{e9}".to_owned(),
        ] {
            let mut value = fixture();
            value["bidders"][0]["operations"][0]["driver"] = Value::String(invalid);
            reject(&value);
            checked += 1;
        }
        assert_eq!(checked, 21);
    }

    #[test]
    fn prerequisite_references_must_be_declared_and_root_tokens_unique() {
        let mut value = fixture();
        value["shared_prerequisites"] = Value::Array(Vec::new());
        reject(&value);
        value["bidders"][0]["operations"][0]["shared_prerequisites"] = Value::Array(Vec::new());
        parse(&encode(&value)).unwrap();
        for invalid in [
            String::new(),
            "x".repeat(65),
            "two words".to_owned(),
            "\u{e9}".to_owned(),
        ] {
            let mut invalid_root = fixture();
            invalid_root["shared_prerequisites"]
                .as_array_mut()
                .unwrap()
                .push(Value::String(invalid.clone()));
            reject(&invalid_root);
            let mut invalid_reference = fixture();
            invalid_reference["bidders"][0]["operations"][0]["shared_prerequisites"] =
                Value::Array(vec![Value::String(invalid)]);
            reject(&invalid_reference);
        }
        let mut duplicate = fixture();
        duplicate["shared_prerequisites"]
            .as_array_mut()
            .unwrap()
            .push(Value::String("core_gate".to_owned()));
        reject(&duplicate);
        let mut unknown = fixture();
        unknown["bidders"][0]["operations"][0]["shared_prerequisites"] =
            Value::Array(vec![Value::String("missing".to_owned())]);
        reject(&unknown);
        unknown["bidders"][0]["operations"][0]["shared_prerequisites"] =
            Value::Array(vec![Value::String("CORE_GATE".to_owned())]);
        reject(&unknown);
    }

    #[test]
    fn prerequisite_tokens_accept_boundaries_and_remain_case_sensitive() {
        for token in ["a".to_owned(), "A_9-z".to_owned(), "x".repeat(64)] {
            let mut value = fixture();
            let prerequisite = Value::Array(vec![Value::String(token)]);
            value["shared_prerequisites"] = prerequisite.clone();
            value["bidders"][0]["operations"][0]["shared_prerequisites"] = prerequisite;
            parse(&encode(&value)).unwrap();
        }
        let mut value = fixture();
        value["shared_prerequisites"]
            .as_array_mut()
            .unwrap()
            .push(Value::String("CORE_GATE".to_owned()));
        parse(&encode(&value)).unwrap();
    }

    #[test]
    fn validates_every_bidder_and_operation_not_just_the_first() {
        let mut value = fixture();
        value["bidders"] = bidders(2, 2);
        assert_eq!(parse(&encode(&value)).unwrap().operation_count(), 4);
        value["bidders"][1]["operations"][1]["driver"] = Value::String("shell command".to_owned());
        reject(&value);
        value["bidders"][1]["operations"] = operations(2);
        value["bidders"][1]["metadata_blob"] = Value::String("bad".to_owned());
        reject(&value);
    }
}
