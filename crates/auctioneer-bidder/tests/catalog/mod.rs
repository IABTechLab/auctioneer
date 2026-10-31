//! Independent production catalog/source inventory readers, used only by conformance.

#![cfg(test)]

use crate::registry_inventory::{Manifest, SourceInventory, SourceSchema};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const UPSTREAM: &str = "https://github.com/prebid/prebid-server.git";
const REVISION: &str = "d7f8515b86258688304b0d9b6668c6a0e258bc9e";
const SOURCE_INVENTORY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/conformance/fixtures/reference/catalog/source-inventory.toml"
));

pub(crate) fn bidder_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bidders")
}

pub(crate) fn manifests() -> Vec<Manifest> {
    let mut found = BTreeMap::new();
    for entry in fs::read_dir(bidder_root()).unwrap() {
        let directory = entry.unwrap().path();
        let path = directory.join("bidder.toml");
        if !path.is_file() {
            continue;
        }
        let manifest: Manifest = toml::from_str(&fs::read_to_string(&path).unwrap())
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!(
            directory.file_name().unwrap().to_str().unwrap(),
            manifest.key
        );
        assert!(found.insert(manifest.key.clone(), manifest).is_none());
    }
    assert!(
        !found.is_empty(),
        "the independent manifest scan must not be empty"
    );
    found.into_values().collect()
}

#[derive(Deserialize)]
struct MetadataPins {
    identity: Vec<SourceSchema>,
}

pub(crate) fn source_inventory() -> SourceInventory {
    let mut document: toml::Value = toml::from_str(SOURCE_INVENTORY).unwrap();
    document
        .as_table_mut()
        .unwrap()
        .entry("repository")
        .or_insert_with(|| UPSTREAM.into());
    let inventory: SourceInventory = document.try_into().unwrap();
    assert_eq!(inventory.repository, UPSTREAM);
    assert_eq!(inventory.revision, REVISION);
    assert!(!inventory.implementations.is_empty());
    assert!(!inventory.schema.is_empty());
    inventory
}

pub(crate) fn check_metadata_source_inventory(manifests: &[Manifest], inventory: &SourceInventory) {
    let pins: MetadataPins = toml::from_str(SOURCE_INVENTORY).unwrap();
    assert!(!pins.identity.is_empty());
    let mut actual = BTreeMap::new();
    for manifest in manifests {
        let Some(artifacts) = manifest
            .provenance
            .get("artifacts")
            .and_then(toml::Value::as_array)
        else {
            continue;
        };
        for artifact in artifacts
            .iter()
            .filter(|row| row["category"].as_str() == Some("metadata"))
        {
            assert!(artifact.get("retained").is_none());
            let source = &manifest.provenance["sources"][artifact["source"].as_str().unwrap()];
            if source["repository"].as_str() == Some(inventory.repository.as_str())
                && source["revision"].as_str() == Some(inventory.revision.as_str())
            {
                assert!(
                    actual
                        .insert(artifact["source_path"].as_str().unwrap(), artifact)
                        .is_none()
                );
            }
        }
    }
    assert_eq!(actual.len(), pins.identity.len());
    for pin in &pins.identity {
        let witness = actual[pin.path.as_str()];
        assert_eq!(
            witness["source_blob"].as_str(),
            Some(pin.blob.as_str()),
            "{}",
            pin.path
        );
        assert_eq!(
            witness["source_bytes"]
                .as_integer()
                .and_then(|n| u64::try_from(n).ok()),
            Some(pin.bytes),
            "{}",
            pin.path
        );
        assert_eq!(
            witness["source_sha256"].as_str(),
            Some(pin.sha256.as_str()),
            "{}",
            pin.path
        );
    }
}
