//! Independently constructed synthetic importer inputs; never copies a catalog.

#![cfg(test)]

use super::{Inventory, InventoryError, inventory_with_pins};
use serde_json::json;
use sha2::{Digest as _, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::{fs, io};
use tempfile::TempDir;

pub const REVISION: &str = "1111111111111111111111111111111111111111";
pub const REPOSITORY: &str = "https://example.invalid/fake.git";
pub const SCHEMAS: usize = 7;
pub const OWNERS: usize = 5;

type Schema = fn() -> serde_json::Value;

const ROWS: [(&str, &str, Schema); SCHEMAS] = [
    (
        "fake_base",
        "fake_base",
        || json!({"$schema":"http://json-schema.org/draft-04/schema#","type":"object","required":["letters","digits"],"properties":{"letters":{"type":"string","pattern":"^[a-z]+$"},"digits":{"type":"string","pattern":"^[0-9]+$"}}}),
    ),
    (
        "fake_alias",
        "fake_base",
        || json!({"$schema":"http://json-schema.org/draft-04/schema#","type":"integer","minimum":1_i32}),
    ),
    (
        "fake_secondary",
        "fake_secondary",
        || json!({"$schema":"http://json-schema.org/draft-04/schema#","type":"boolean"}),
    ),
    (
        "fake_secondary_alias",
        "fake_secondary",
        || json!({"$schema":"http://json-schema.org/draft-04/schema#","type":"null"}),
    ),
    (
        "fake_numeric",
        "fake_numeric",
        || json!({"$schema":"http://json-schema.org/draft-04/schema#","type":"number","maximum":9_i32}),
    ),
    (
        "fake_Camel",
        "fake_camel",
        || json!({"$schema":"http://json-schema.org/draft-04/schema#","type":"string","format":"uuid"}),
    ),
    (
        "fake_format",
        "fake_format",
        || json!({"$schema":"https://json-schema.org/draft/2020-12/schema","if":{"type":"string"},"then":{"format":"hostname"}}),
    ),
];

pub fn schema_bytes() -> u64 {
    ROWS.iter()
        .map(|(_, _, schema)| u64::try_from(schema().to_string().len()).unwrap())
        .sum()
}

pub fn largest_bytes() -> u64 {
    u64::try_from((ROWS[0].2)().to_string().len()).unwrap()
}

pub fn pins() -> (String, String) {
    let implementations: Vec<_> = ROWS.iter().filter(|(name, key, _)| *name == *key || *name == "fake_Camel").map(|(name, key, _)| json!({"source_directory": format!("adapters/{name}"), "internal_key": key})).collect();
    assert_eq!(implementations.len(), OWNERS);
    let schemas: Vec<_> = ROWS.iter().enumerate().map(|(index, (name, _, build_schema))| {
        let schema = build_schema().to_string();
        json!({"name": name, "path": format!("static/bidder-params/{name}.json"), "blob": format!("{:040x}", index.checked_add(1).unwrap()), "bytes": schema.len(), "sha256": format!("{:x}", Sha256::digest(schema.as_bytes()))})
    }).collect();
    let source = json!({"schema_version": 2_i32, "kind": "reference_inventory_only", "revision": REVISION, "counts": {"schemas": SCHEMAS, "schema_bytes": schema_bytes(), "largest_schema_bytes": largest_bytes()}, "implementations": implementations, "schema": schemas});
    // Public source identities may differ from their filesystem implementation key.
    let bindings: serde_json::Map<_, _> = ROWS
        .iter()
        .map(|(name, key, _)| {
            (
                (*name).to_owned(),
                json!(format!(
                    "adapters/{}",
                    if *name == "fake_Camel" { *name } else { *key }
                )),
            )
        })
        .collect();
    (
        toml::to_string(&source).unwrap(),
        toml::to_string(
            &json!({"schema_version": 1_i32, "revision": REVISION, "bindings": bindings}),
        )
        .unwrap(),
    )
}

pub fn inventory(bidders: &Path, source: &Path) -> Result<Inventory, InventoryError> {
    let (pinned_source, pinned_bindings) = pins();
    inventory_with_pins(
        bidders,
        source,
        &pinned_source,
        &pinned_bindings,
        REPOSITORY,
        &supplemental_pins(),
    )
}

fn supplemental_pins() -> String {
    let schema = (ROWS[2].2)().to_string();
    let sha256 = format!("{:x}", Sha256::digest(schema.as_bytes()));
    toml::to_string(&json!({"schema_version":1_i32,"kind":"supplemental_schema_reference_only","require_all":false,"schema":[{
        "key":"fake_supplemental","name":"fake_supplemental","repository":REPOSITORY,"revision":"2222222222222222222222222222222222222222",
        "source_path":"static/bidder-params/fake_supplemental.json","source_blob":format!("{:040x}", 3_usize),"source_bytes":schema.len(),"source_sha256":sha256,
        "retained":{"source_path":"parameters.json","source_bytes":schema.len(),"source_sha256":sha256}
    }]})).unwrap()
}

pub struct Fixture {
    pub directory: TempDir,
    pub bidders: PathBuf,
    pub source: PathBuf,
}

impl Fixture {
    pub fn new() -> io::Result<Self> {
        let directory = tempfile::tempdir()?;
        let bidders = directory.path().join("bidders");
        let source = directory.path().join("source-inventory.toml");
        let (pinned_source, _) = pins();
        fs::write(&source, pinned_source)?;
        let mut manifests = BTreeMap::new();
        for (index, (name, key, build_schema)) in ROWS.iter().enumerate() {
            let schema = build_schema().to_string();
            let retained = if name == key || *name == "fake_Camel" {
                "parameters.json".to_owned()
            } else {
                format!("schemas/{name}.schema.json")
            };
            let path = bidders.join(key).join(&retained);
            fs::create_dir_all(path.parent().unwrap())?;
            fs::write(path, &schema)?;
            let artifacts = manifests.entry(*key).or_insert_with(Vec::new);
            artifacts.push(json!({"category": "schema", "source": "upstream", "source_path": format!("static/bidder-params/{name}.json"), "source_blob": format!("{:040x}", index.checked_add(1).unwrap()), "source_bytes": schema.len(), "source_sha256": format!("{:x}", Sha256::digest(schema.as_bytes())), "retained": {"source_path": retained, "source_bytes": schema.len(), "source_sha256": format!("{:x}", Sha256::digest(schema.as_bytes()))}}));
        }
        for (key, artifacts) in manifests {
            let manifest = json!({"key": key, "factory": false, "provenance": {"origin": "reference", "sources": {"upstream": {"repository": REPOSITORY, "revision": REVISION}}, "artifacts": artifacts}});
            fs::write(
                bidders.join(key).join("bidder.toml"),
                toml::to_string(&manifest).map_err(io::Error::other)?,
            )?;
        }
        let workload = json!({"schema":"fake_base", "order":0_usize, "maximum_string":true, "string_refusals":true, "pattern_initialization":true, "retained_constructor":true, "rows":[
            {"id":"short","input":{"letters":"abc","digits":"123"},"valid":true,"transforms":[]},
            {"id":"max-string","input":{"letters":"a","digits":"1"},"valid":true,"transforms":[{"op":"max_string","pointer":"/letters","character":"a","invalid":false}]},
            {"id":"max-string-invalid-tail","input":{"letters":"a","digits":"1"},"valid":false,"transforms":[{"op":"max_string","pointer":"/letters","character":"a","invalid":true}]},
            {"id":"unicode","input":{"letters":"\u{0661}","digits":"1"},"valid":false,"transforms":[]}
        ]});
        fs::create_dir_all(bidders.join("fake_base/tests/schema"))?;
        fs::write(
            bidders.join("fake_base/tests/schema/workload.toml"),
            toml::to_string(&workload).map_err(io::Error::other)?,
        )?;
        fs::write(
            bidders.join("fake_base/tests/schema/helpers.toml"),
            "schema = 'fake_base'\nhelpers = [{index=0,prefixes=2,onepass=true,alternative=false},{index=1,prefixes=2,onepass=true,alternative=false}]\n",
        )?;
        fs::write(
            bidders.join("fake_base/tests/schema/raw-controls.toml"),
            "schema = 'fake_base'\nvalid_json = '{\"letters\":\"abc\",\"digits\":\"123\"}'\ninvalid_json = '{\"letters\":null,\"digits\":\"123\"}'\n",
        )?;
        fs::write(
            bidders.join("fake_base/tests/schema/literal-controls.toml"),
            "schema = 'fake_base'\ncases = [{input_json='{\"letters\":\"abc\",\"digits\":\"123\"}',valid=true},{input_json='{\"letters\":\"A\",\"digits\":\"123\"}',valid=false}]\n",
        )?;
        let formats = json!({"schema":"fake_Camel","cases":[
            {"input_json":json!("not-a-uuid").to_string(),"valid":false,"formats_disabled_valid":true},
            {"input_json":json!("123e4567-e89b-12d3-a456-426614174000").to_string(),"valid":true,"formats_disabled_valid":true},
            {"input_json":json!(null).to_string(),"valid":false,"formats_disabled_valid":false}
        ]});
        fs::create_dir_all(bidders.join("fake_camel/tests/schema"))?;
        fs::write(
            bidders.join("fake_camel/tests/schema/literal-controls.toml"),
            toml::to_string(&formats).map_err(io::Error::other)?,
        )?;
        let longest_host = [63_usize; 4].map(|length| "a".repeat(length)).join(".");
        let hostname = json!({"schema":"fake_format", "order":1_usize, "rows":[
            {"id":"short","input":"a.example","valid":true,"transforms":[]},
            {"id":"long-host","input":longest_host,"valid":true,"transforms":[]},
            {"id":"invalid-host-tail","input":"a.example!","valid":false,"transforms":[]},
            {"id":"unicode-host","input":"\u{0661}.co","valid":false,"transforms":[]},
            {"id":"overlong-label","input":format!("{}.co", "a".repeat(64)),"valid":false,"transforms":[]}
        ]});
        fs::create_dir_all(bidders.join("fake_format/tests/schema"))?;
        let drafts = json!({"schema":"fake_format","cases":[
            {"input_json":json!("a_b.example").to_string(),"valid":false,"draft4_valid":true},
            {"input_json":json!("a.example").to_string(),"valid":true,"draft4_valid":true},
            {"input_json":json!(null).to_string(),"valid":true,"draft4_valid":true}
        ]});
        fs::write(
            bidders.join("fake_format/tests/schema/literal-controls.toml"),
            toml::to_string(&drafts).map_err(io::Error::other)?,
        )?;
        fs::write(
            bidders.join("fake_format/tests/schema/workload.toml"),
            toml::to_string(&hostname).map_err(io::Error::other)?,
        )?;
        Ok(Self {
            directory,
            bidders,
            source,
        })
    }
}
