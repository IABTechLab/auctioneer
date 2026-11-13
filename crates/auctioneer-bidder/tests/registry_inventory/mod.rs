//! Registry membership and canonical-witness checks over caller-supplied fixture inputs.

#![cfg(test)]

use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Manifest {
    pub key: String,
    pub factory: bool,
    pub required_feature: Option<String>,
    #[serde(default)]
    pub native_only: bool,
    #[serde(default)]
    pub definitions: BTreeMap<String, toml::Table>,
    pub provenance: toml::Value,
}

pub(crate) fn feature_active(feature: Option<&str>) -> bool {
    match feature {
        None => true,
        Some("insecure-bidders") => cfg!(feature = "insecure-bidders"),
        Some(unknown) => panic!("unknown required feature: {unknown}"),
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Membership {
    pub factory_declarations: Vec<(String, Option<String>)>,
    pub factories: Vec<String>,
    pub metadata_declarations: Vec<(String, String, Option<String>)>,
    pub public_names: Vec<String>,
    pub bindings: Vec<(String, String)>,
}

impl Membership {
    pub(crate) fn from_manifests(manifests: &[Manifest]) -> Self {
        assert!(!manifests.is_empty());
        let mut expected = Self::default();
        let mut names = BTreeSet::new();
        for manifest in manifests {
            let active = feature_active(manifest.required_feature.as_deref());
            if manifest.factory {
                expected
                    .factory_declarations
                    .push((manifest.key.clone(), manifest.required_feature.clone()));
                if active {
                    expected.factories.push(manifest.key.clone());
                }
            } else {
                assert!(
                    manifest.definitions.is_empty(),
                    "nonfactory {}",
                    manifest.key
                );
            }
            for (name, definition) in &manifest.definitions {
                assert!(names.insert(name.clone()), "duplicate public name {name}");
                if let Some(binding) = definition.get("implementation") {
                    assert_eq!(binding.as_str(), Some(manifest.key.as_str()));
                }
                if let Some(feature) = definition.get("required_feature") {
                    assert_eq!(feature.as_str(), manifest.required_feature.as_deref());
                }
                expected.metadata_declarations.push((
                    name.clone(),
                    manifest.key.clone(),
                    manifest.required_feature.clone(),
                ));
                if active && (!manifest.native_only || !cfg!(target_arch = "wasm32")) {
                    expected.public_names.push(name.clone());
                    expected.bindings.push((name.clone(), manifest.key.clone()));
                }
            }
        }
        expected.factory_declarations.sort();
        expected.factories.sort();
        expected.metadata_declarations.sort();
        expected.public_names.sort();
        expected.bindings.sort();
        assert!(!expected.factory_declarations.is_empty());
        assert!(!expected.factories.is_empty());
        assert!(!expected.metadata_declarations.is_empty());
        assert!(!expected.public_names.is_empty());
        assert!(!expected.bindings.is_empty());
        expected
    }

    pub(crate) fn check(&self, actual: &Self) -> Result<(), &'static str> {
        if self.factory_declarations != actual.factory_declarations {
            return Err("factory declarations");
        }
        if self.factories != actual.factories {
            return Err("active factories");
        }
        if self.metadata_declarations != actual.metadata_declarations {
            return Err("metadata declarations");
        }
        if self.public_names != actual.public_names {
            return Err("active public names");
        }
        if self.bindings != actual.bindings {
            return Err("active bindings");
        }
        Ok(())
    }
}

#[derive(Deserialize)]
pub(crate) struct SourceInventory {
    pub repository: String,
    pub revision: String,
    pub implementations: Vec<SourceImplementation>,
    pub schema: Vec<SourceSchema>,
}

#[derive(Deserialize)]
pub(crate) struct SourceImplementation {
    pub source_directory: String,
    pub internal_key: String,
}

#[derive(Deserialize)]
pub(crate) struct SourceSchema {
    pub path: String,
    pub blob: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum SchemaError {
    Ownership,
    Source,
    Witness,
    Retained,
}

fn object_id(candidate: Option<&str>, length: usize) -> bool {
    candidate.is_some_and(|value| {
        value.len() == length
            && value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

fn canonical_source<'a>(
    manifest: &'a Manifest,
    inventory: &'a SourceInventory,
) -> Result<(&'a str, String, Option<&'a SourceSchema>), SchemaError> {
    let trees = manifest
        .provenance
        .get("trees")
        .and_then(toml::Value::as_array)
        .ok_or(SchemaError::Ownership)?;
    let implementations: Vec<_> = trees
        .iter()
        .filter(|tree| tree.get("category").and_then(toml::Value::as_str) == Some("implementation"))
        .collect();
    if implementations.len() != 1 {
        return Err(SchemaError::Ownership);
    }
    let tree = implementations[0];
    let source_key = tree
        .get("source")
        .and_then(toml::Value::as_str)
        .ok_or(SchemaError::Source)?;
    let source = manifest
        .provenance
        .get("sources")
        .and_then(|sources| sources.get(source_key))
        .ok_or(SchemaError::Source)?;
    let directory = tree
        .get("source_path")
        .and_then(toml::Value::as_str)
        .ok_or(SchemaError::Ownership)?;
    let owner = inventory
        .implementations
        .iter()
        .find(|row| row.internal_key == manifest.key);
    let canonical_directory = owner.map_or(directory, |row| row.source_directory.as_str());
    let basename = canonical_directory
        .strip_prefix("adapters/")
        .filter(|name| !name.is_empty() && !name.contains('/'))
        .ok_or(SchemaError::Ownership)?;
    let path = format!("static/bidder-params/{basename}.json");
    if !object_id(source.get("revision").and_then(toml::Value::as_str), 40)
        || source
            .get("repository")
            .and_then(toml::Value::as_str)
            .is_none_or(str::is_empty)
    {
        return Err(SchemaError::Source);
    }
    let pin = if owner.is_some() {
        if directory != canonical_directory {
            return Err(SchemaError::Ownership);
        }
        if source["repository"].as_str() != Some(inventory.repository.as_str())
            || source["revision"].as_str() != Some(inventory.revision.as_str())
        {
            return Err(SchemaError::Source);
        }
        Some(
            inventory
                .schema
                .iter()
                .find(|row| row.path == path)
                .ok_or(SchemaError::Ownership)?,
        )
    } else {
        // Supplemental/fork identities are frozen in their adjacent provenance;
        // source-specific integrity tests verify those immutable source objects.
        None
    };
    Ok((source_key, path, pin))
}

pub(crate) fn check_canonical_schema(
    manifest: &Manifest,
    bytes: &[u8],
    inventory: &SourceInventory,
) -> Result<(), SchemaError> {
    let (source, path, reference_pin) = canonical_source(manifest, inventory)?;
    let artifacts = manifest
        .provenance
        .get("artifacts")
        .and_then(toml::Value::as_array)
        .ok_or(SchemaError::Ownership)?;
    let canonical: Vec<_> = artifacts
        .iter()
        .filter(|row| {
            row.get("category").and_then(toml::Value::as_str) == Some("schema")
                && row.get("source_path").and_then(toml::Value::as_str) == Some(path.as_str())
        })
        .collect();
    let retained_owners = artifacts
        .iter()
        .filter(|row| {
            row.get("retained")
                .and_then(|retained| retained.get("source_path"))
                .and_then(toml::Value::as_str)
                == Some("parameters.json")
        })
        .count();
    if canonical.len() != 1 || retained_owners != 1 {
        return Err(SchemaError::Ownership);
    }
    let schema = canonical[0];
    let retained = schema.get("retained").ok_or(SchemaError::Ownership)?;
    if retained.get("source_path").and_then(toml::Value::as_str) != Some("parameters.json") {
        return Err(SchemaError::Ownership);
    }
    if schema.get("source").and_then(toml::Value::as_str) != Some(source) {
        return Err(SchemaError::Source);
    }
    let actual_length = u64::try_from(bytes.len()).map_err(|_error| SchemaError::Witness)?;
    let digest = hex::encode(Sha256::digest(bytes));
    let source_length = schema
        .get("source_bytes")
        .and_then(toml::Value::as_integer)
        .and_then(|declared_length| u64::try_from(declared_length).ok());
    if !object_id(schema.get("source_blob").and_then(toml::Value::as_str), 40)
        || source_length != Some(actual_length)
        || schema.get("source_sha256").and_then(toml::Value::as_str) != Some(digest.as_str())
    {
        return Err(SchemaError::Witness);
    }
    if let Some(pin) = reference_pin
        && (schema["source_blob"].as_str() != Some(pin.blob.as_str())
            || actual_length != pin.bytes
            || digest != pin.sha256)
    {
        return Err(SchemaError::Witness);
    }
    if retained
        .get("source_bytes")
        .and_then(toml::Value::as_integer)
        .and_then(|retained_length| u64::try_from(retained_length).ok())
        != Some(actual_length)
        || retained.get("source_sha256").and_then(toml::Value::as_str) != Some(digest.as_str())
    {
        return Err(SchemaError::Retained);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Manifest, Membership};

    #[test]
    fn native_only_filters_public_views_without_filtering_factories_or_declarations() {
        let manifests: Vec<Manifest> = [
            "key = 'fake_portable'\nfactory = true\n[definitions.fake_portable]\n[provenance]\norigin = 'local'\n",
            "key = 'fake_native'\nfactory = true\nnative_only = true\n[definitions.fake_native]\n[provenance]\norigin = 'local'\n",
        ].into_iter().map(|source| toml::from_str(source).unwrap()).collect();
        let membership = Membership::from_manifests(&manifests);
        assert_eq!(membership.factory_declarations.len(), 2);
        assert_eq!(membership.factories, ["fake_native", "fake_portable"]);
        assert_eq!(membership.metadata_declarations.len(), 2);
        assert!(
            membership
                .metadata_declarations
                .iter()
                .any(|(name, _, _)| name == "fake_native")
        );
        assert_eq!(
            membership
                .public_names
                .iter()
                .any(|name| name == "fake_native"),
            !cfg!(target_arch = "wasm32")
        );
        assert_eq!(
            membership
                .bindings
                .iter()
                .any(|(name, _)| name == "fake_native"),
            !cfg!(target_arch = "wasm32")
        );
        assert!(
            membership
                .public_names
                .iter()
                .any(|name| name == "fake_portable")
        );
    }
}
