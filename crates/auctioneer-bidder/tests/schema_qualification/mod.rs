//! Native, data-only schema audit. No compiler, instance validation, or reference retrieval.

#![cfg(not(target_arch = "wasm32"))]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::str;

use auctioneer_config::bidder_manifest;
use bidder_manifest::{Category, Manifest, Provenance};

const MAX_DEPTH: usize = 64;
const MAX_SCHEMA_BYTES: u64 = 16 << 10;
const MAX_DOCUMENT_BYTES: u64 = 1 << 20;
const MAX_NODES: usize = 8_192;
const COMMITTED_SOURCE: &str =
    include_str!("../conformance/fixtures/reference/catalog/source-inventory.toml");
const COMMITTED_BINDINGS: &str =
    include_str!("../conformance/fixtures/reference/catalog/source-bindings.toml");
const COMMITTED_SUPPLEMENTAL: &str =
    include_str!("../conformance/fixtures/reference/catalog/supplemental-schemas.toml");

pub mod fixture;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum Draft {
    Draft4,
    Draft202012,
    Unspecified,
    Other(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Occurrence {
    pub keyword: String,
    pub pointer: String,
    pub value: Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LocatedString {
    pub pointer: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum PatternKind {
    Pattern,
    PropertyName,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RegexPattern {
    pub pointer: String,
    pub value: String,
    pub kind: PatternKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum LocalTarget {
    Schema(String),
    NonSchema(String),
    Unresolved,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LocalReference {
    pub pointer: String,
    pub value: String,
    pub target: LocalTarget,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SchemaAudit {
    pub draft: Draft,
    pub schema_positions: Vec<String>,
    pub validation_keywords: Vec<Occurrence>,
    pub annotations: Vec<Occurrence>,
    pub metadata_keywords: Vec<Occurrence>,
    pub unknown_keywords: Vec<Occurrence>,
    pub formats: Vec<LocatedString>,
    pub patterns: Vec<RegexPattern>,
    pub local_references: Vec<LocalReference>,
    pub external_references: Vec<LocatedString>,
    /// Sorted cyclic strongly connected components, including containment edges.
    pub local_reference_cycles: Vec<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SchemaWitness {
    pub name: String,
    pub path: String,
    pub source_path: String,
    pub source_blob: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SchemaInventory {
    pub witness: SchemaWitness,
    pub audit: SchemaAudit,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Inventory {
    #[serde(skip)]
    pub root: PathBuf,
    pub revision: String,
    pub schema_bytes: u64,
    pub largest_schema: SchemaWitness,
    pub schemas: Vec<SchemaInventory>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryError {
    Input,
    Limit,
    Revision,
    Membership,
    Path,
    Witness,
    Counts,
    Structure,
}

#[derive(Deserialize)]
struct SourceInventory {
    schema_version: u32,
    kind: String,
    revision: String,
    counts: Counts,
    implementations: Vec<SourceImplementation>,
    schema: Vec<SourceWitness>,
}

#[derive(Deserialize)]
struct SourceImplementation {
    source_directory: String,
    internal_key: String,
}

#[derive(Deserialize)]
struct SourceBindings {
    schema_version: u32,
    revision: String,
    bindings: BTreeMap<String, String>,
}

#[derive(Deserialize, Eq, PartialEq)]
struct Counts {
    schemas: u64,
    schema_bytes: u64,
    largest_schema_bytes: u64,
}

#[derive(Deserialize)]
struct SourceWitness {
    name: String,
    path: String,
    blob: String,
    bytes: u64,
    sha256: String,
}

fn directory(path: &Path) -> Result<(), InventoryError> {
    let metadata = fs::symlink_metadata(path).map_err(|_error| InventoryError::Input)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(InventoryError::Path);
    }
    Ok(())
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, InventoryError> {
    let metadata = fs::symlink_metadata(path).map_err(|_error| InventoryError::Input)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(InventoryError::Path);
    }
    if metadata.len() > limit {
        return Err(InventoryError::Limit);
    }
    let file = fs::File::open(path).map_err(|_error| InventoryError::Input)?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_error| InventoryError::Input)?;
    let length = u64::try_from(bytes.len()).map_err(|_error| InventoryError::Limit)?;
    if length > limit {
        return Err(InventoryError::Limit);
    }
    if length != metadata.len() {
        return Err(InventoryError::Input);
    }
    Ok(bytes)
}

fn source_inventory(bytes: &[u8]) -> Result<SourceInventory, InventoryError> {
    let text = str::from_utf8(bytes).map_err(|_error| InventoryError::Input)?;
    toml::from_str(text).map_err(|_error| InventoryError::Input)
}

fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn bindings(text: &str, revision: &str) -> Result<SourceBindings, InventoryError> {
    let bindings: SourceBindings = toml::from_str(text).map_err(|_error| InventoryError::Input)?;
    if bindings.revision != revision {
        return Err(InventoryError::Revision);
    }
    if bindings.schema_version != 1 {
        return Err(InventoryError::Witness);
    }
    Ok(bindings)
}

fn implementation_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn witnesses(
    source: SourceInventory,
    bindings: &SourceBindings,
) -> Result<BTreeMap<String, SchemaWitness>, InventoryError> {
    let mut owners = BTreeMap::new();
    for row in source.implementations {
        row.source_directory
            .strip_prefix("adapters/")
            .filter(|name| safe_name(name))
            .ok_or(InventoryError::Path)?;
        if !implementation_key(&row.internal_key) {
            return Err(InventoryError::Path);
        }
        if owners
            .insert(row.source_directory, row.internal_key)
            .is_some()
        {
            return Err(InventoryError::Membership);
        }
    }
    if owners.is_empty() || owners.values().collect::<BTreeSet<_>>().len() != owners.len() {
        return Err(InventoryError::Membership);
    }
    let mut rows = BTreeMap::new();
    for row in source.schema {
        if !safe_name(&row.name) || row.path != format!("static/bidder-params/{}.json", row.name) {
            return Err(InventoryError::Path);
        }
        let source_directory = bindings
            .bindings
            .get(&row.name)
            .ok_or(InventoryError::Witness)?;
        let key = owners
            .get(source_directory)
            .ok_or(InventoryError::Witness)?;
        let base = source_directory
            .strip_prefix("adapters/")
            .ok_or(InventoryError::Path)?;
        let path = if row.name == base {
            format!("{key}/parameters.json")
        } else {
            format!("{key}/schemas/{}.schema.json", row.name)
        };
        let witness = SchemaWitness {
            path,
            name: row.name.clone(),
            source_path: row.path,
            source_blob: row.blob,
            bytes: row.bytes,
            sha256: row.sha256,
        };
        if rows.insert(row.name, witness).is_some() {
            return Err(InventoryError::Membership);
        }
    }
    if u64::try_from(rows.len()).map_err(|_error| InventoryError::Limit)? != source.counts.schemas {
        return Err(InventoryError::Membership);
    }
    Ok(rows)
}

fn retained_schema_path(path: &str) -> bool {
    path == "parameters.json"
        || path
            .strip_prefix("schemas/")
            .and_then(|name| name.strip_suffix(".schema.json"))
            .is_some_and(safe_name)
}

fn manifest_witnesses(
    manifest: Manifest,
    key: &str,
    expected: &BTreeMap<String, SchemaWitness>,
    found: &mut BTreeMap<String, SchemaWitness>,
    revision: &str,
    repository: &str,
    exclusion: Option<&Exclusion>,
) -> Result<(), InventoryError> {
    if manifest.key != key {
        return Err(InventoryError::Witness);
    }
    let Provenance::Reference {
        sources, artifacts, ..
    } = manifest.provenance
    else {
        return Ok(());
    };
    for row in artifacts {
        if row.category != Category::Schema {
            continue;
        }
        let name = row
            .source_path
            .strip_prefix("static/bidder-params/")
            .and_then(|path| path.strip_suffix(".json"))
            .filter(|name| safe_name(name))
            .ok_or(InventoryError::Path)?;
        let source = sources.get(&row.source).ok_or(InventoryError::Witness)?;
        if !expected.contains_key(name)
            && exclusion.is_some_and(|pin| {
                pin.name == name
                    && pin.repository == source.repository
                    && pin.revision == source.revision
            })
        {
            continue;
        }
        if source.revision != revision {
            return Err(InventoryError::Revision);
        }
        if source.repository != repository {
            return Err(InventoryError::Witness);
        }
        let witness = expected.get(name).ok_or(InventoryError::Witness)?;
        if found.contains_key(name) || found.len() >= 261 {
            return Err(InventoryError::Membership);
        }
        let retained = row.retained.ok_or(InventoryError::Witness)?;
        if !retained_schema_path(&retained.path) {
            return Err(InventoryError::Path);
        }
        let actual = SchemaWitness {
            name: name.to_owned(),
            path: format!("{key}/{}", retained.path),
            source_path: row.source_path,
            source_blob: row.source_blob,
            bytes: retained.bytes,
            sha256: retained.sha256,
        };
        if actual != *witness
            || row.source_sha256 != witness.sha256
            || row.source_bytes != witness.bytes
        {
            return Err(InventoryError::Witness);
        }
        found.insert(actual.name.clone(), actual);
    }
    Ok(())
}

fn schema_set(root: &Path, key: &str, found: &mut BTreeSet<String>) -> Result<(), InventoryError> {
    directory(root)?;
    for result in fs::read_dir(root).map_err(|_error| InventoryError::Input)? {
        let entry = result.map_err(|_error| InventoryError::Input)?;
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|_error| InventoryError::Input)?;
        if metadata.file_type().is_symlink() {
            return Err(InventoryError::Path);
        }
        if !metadata.is_file() || found.len() >= 261 {
            return Err(InventoryError::Membership);
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_error| InventoryError::Path)?;
        if !retained_schema_path(&format!("schemas/{name}")) {
            return Err(InventoryError::Membership);
        }
        found.insert(format!("{key}/schemas/{name}"));
    }
    Ok(())
}

fn owned_schema_set(
    root: &Path,
    key: &str,
    found: &mut BTreeSet<String>,
) -> Result<(), InventoryError> {
    for result in fs::read_dir(root).map_err(|_error| InventoryError::Input)? {
        let entry = result.map_err(|_error| InventoryError::Input)?;
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|_error| InventoryError::Input)?;
        if metadata.file_type().is_symlink() {
            return Err(InventoryError::Path);
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_error| InventoryError::Path)?;
        if name == "schemas" {
            schema_set(&entry.path(), key, found)?;
        } else if Path::new(&name)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            if name != "parameters.json" || !metadata.is_file() || found.len() >= 261 {
                return Err(InventoryError::Membership);
            }
            found.insert(format!("{key}/{name}"));
        } else {
            // Non-schema files are outside the original schema inventory.
        }
    }
    Ok(())
}

fn owned_witnesses(
    root: &Path,
    expected: &BTreeMap<String, SchemaWitness>,
    revision: &str,
    repository: &str,
    supplemental: &SupplementalReference,
) -> Result<BTreeMap<String, SchemaWitness>, InventoryError> {
    let owners: BTreeSet<_> = expected
        .values()
        .filter_map(|row| row.path.split('/').next())
        .collect();
    let mut found = BTreeMap::new();
    let mut paths = BTreeSet::new();
    let mut supplemental_owners = BTreeSet::new();
    for result in fs::read_dir(root).map_err(|_error| InventoryError::Input)? {
        let entry = result.map_err(|_error| InventoryError::Input)?;
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|_error| InventoryError::Input)?;
        if metadata.file_type().is_symlink() {
            return Err(InventoryError::Path);
        }
        if !metadata.is_dir() {
            if !metadata.is_file() {
                return Err(InventoryError::Path);
            }
            continue;
        }
        let key = entry
            .file_name()
            .into_string()
            .map_err(|_error| InventoryError::Path)?;
        if !implementation_key(&key) {
            return Err(InventoryError::Path);
        }
        let bytes = read_bounded(&entry.path().join("bidder.toml"), MAX_DOCUMENT_BYTES)?;
        let manifest: Manifest =
            toml::from_str(str::from_utf8(&bytes).map_err(|_error| InventoryError::Input)?)
                .map_err(|_error| InventoryError::Input)?;
        let exclusion_path = entry.path().join("tests/schema/inventory-exclusion.toml");
        let exclusion = if exclusion_path
            .try_exists()
            .map_err(|_error| InventoryError::Input)?
        {
            Some(
                toml::from_str::<Exclusion>(
                    str::from_utf8(&read_bounded(&exclusion_path, MAX_DOCUMENT_BYTES)?)
                        .map_err(|_error| InventoryError::Input)?,
                )
                .map_err(|_error| InventoryError::Input)?,
            )
        } else {
            None
        };
        if let Some(pin) = &exclusion {
            verify_exclusion(root, &key, &manifest, pin, supplemental)?;
            if !supplemental_owners.insert(key.clone()) {
                return Err(InventoryError::Membership);
            }
        }
        manifest_witnesses(
            manifest,
            &key,
            expected,
            &mut found,
            revision,
            repository,
            exclusion.as_ref(),
        )?;
        if owners.contains(key.as_str()) {
            owned_schema_set(&entry.path(), &key, &mut paths)?;
        }
    }
    if found.len() != expected.len()
        || paths != expected.values().map(|row| row.path.clone()).collect()
    {
        return Err(InventoryError::Membership);
    }
    if supplemental.require_all
        && supplemental_owners
            != supplemental
                .schema
                .iter()
                .map(|pin| pin.key.clone())
                .collect()
    {
        return Err(InventoryError::Membership);
    }
    Ok(found)
}

/// Verify committed witnesses before auditing every raw schema. Neither input is optional.
pub fn inventory(bidders_root: &Path, source_path: &Path) -> Result<Inventory, InventoryError> {
    if hex::encode(Sha256::digest(COMMITTED_SOURCE.as_bytes()))
        != "b22f74407fd081cd7d073b8724ba8f74e1385163b9bc30c2277342adc38bb6c6"
    {
        return Err(InventoryError::Witness);
    }
    if hex::encode(Sha256::digest(COMMITTED_BINDINGS.as_bytes()))
        != "d1a0f18d99f53cbd4f62fe74c5d1e5580f8688fce95a29b1b05ee934dbe9ff0c"
    {
        return Err(InventoryError::Witness);
    }
    if hex::encode(Sha256::digest(COMMITTED_SUPPLEMENTAL.as_bytes()))
        != "2022d18b8e5580824111a2a3781039b701c7c6df85a0456206ce2b370775ebbb"
    {
        return Err(InventoryError::Witness);
    }
    let result = inventory_with_pins(
        bidders_root,
        source_path,
        COMMITTED_SOURCE,
        COMMITTED_BINDINGS,
        "https://github.com/prebid/prebid-server.git",
        COMMITTED_SUPPLEMENTAL,
    )?;
    if result.revision != "d7f8515b86258688304b0d9b6668c6a0e258bc9e"
        || result.schemas.len() != 261
        || result.schema_bytes != 164_518
        || result.largest_schema.bytes != 3_778
        || result.largest_schema.source_blob != "37909b12066ad9239b488b9751d070212e90d61a"
    {
        return Err(InventoryError::Counts);
    }
    Ok(result)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Exclusion {
    name: String,
    repository: String,
    revision: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SupplementalReference {
    schema_version: u16,
    kind: String,
    require_all: bool,
    schema: Vec<SupplementalWitness>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SupplementalWitness {
    key: String,
    name: String,
    repository: String,
    revision: String,
    source_path: String,
    source_blob: String,
    source_bytes: u64,
    source_sha256: String,
    retained: SupplementalRetained,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SupplementalRetained {
    #[serde(rename = "source_path")]
    path: String,
    #[serde(rename = "source_bytes")]
    bytes: u64,
    #[serde(rename = "source_sha256")]
    sha256: String,
}

fn verify_exclusion(
    root: &Path,
    key: &str,
    manifest: &Manifest,
    exclusion: &Exclusion,
    reference: &SupplementalReference,
) -> Result<(), InventoryError> {
    let pin = reference
        .schema
        .iter()
        .find(|pin| pin.key == key && pin.name == exclusion.name)
        .ok_or(InventoryError::Witness)?;
    if pin.repository != exclusion.repository || pin.revision != exclusion.revision {
        return Err(InventoryError::Witness);
    }
    let Provenance::Reference {
        sources, artifacts, ..
    } = &manifest.provenance
    else {
        return Err(InventoryError::Witness);
    };
    let mut candidates = artifacts
        .iter()
        .filter(|row| row.category == Category::Schema && row.source_path == pin.source_path);
    let row = candidates.next().ok_or(InventoryError::Witness)?;
    if candidates.next().is_some() {
        return Err(InventoryError::Membership);
    }
    let source = sources.get(&row.source).ok_or(InventoryError::Witness)?;
    let retained = row.retained.as_ref().ok_or(InventoryError::Witness)?;
    if source.repository != pin.repository
        || source.revision != pin.revision
        || row.source_blob != pin.source_blob
        || row.source_bytes != pin.source_bytes
        || row.source_sha256 != pin.source_sha256
        || retained.path != pin.retained.path
        || retained.bytes != pin.retained.bytes
        || retained.sha256 != pin.retained.sha256
    {
        return Err(InventoryError::Witness);
    }
    if !retained_schema_path(&retained.path)
        || pin.source_path != format!("static/bidder-params/{}.json", pin.name)
    {
        return Err(InventoryError::Path);
    }
    let bytes = read_bounded(&root.join(key).join(&retained.path), MAX_SCHEMA_BYTES)?;
    if u64::try_from(bytes.len()).map_err(|_error| InventoryError::Limit)? != pin.retained.bytes
        || hex::encode(Sha256::digest(&bytes)) != pin.retained.sha256
    {
        return Err(InventoryError::Witness);
    }
    Ok(())
}

pub fn inventory_with_pins(
    bidders_root: &Path,
    source_path: &Path,
    pinned_source: &str,
    pinned_bindings: &str,
    repository: &str,
    pinned_supplemental: &str,
) -> Result<Inventory, InventoryError> {
    directory(bidders_root)?;
    let pinned = source_inventory(pinned_source.as_bytes())?;
    let source = source_inventory(&read_bounded(source_path, MAX_DOCUMENT_BYTES)?)?;
    if source.revision != pinned.revision {
        return Err(InventoryError::Revision);
    }
    if source.schema_version != 2 || source.kind != "reference_inventory_only" {
        return Err(InventoryError::Witness);
    }
    if source.counts != pinned.counts {
        return Err(InventoryError::Counts);
    }
    let owners = |rows: &[SourceImplementation]| {
        rows.iter()
            .map(|row| (row.source_directory.clone(), row.internal_key.clone()))
            .collect::<BTreeSet<_>>()
    };
    if source.implementations.len() != pinned.implementations.len()
        || owners(&source.implementations) != owners(&pinned.implementations)
    {
        return Err(InventoryError::Membership);
    }
    let revision = pinned.revision.clone();
    let expected_counts = pinned.counts;
    let bindings = bindings(pinned_bindings, &revision)?;
    let expected = witnesses(source_inventory(pinned_source.as_bytes())?, &bindings)?;
    if witnesses(source, &bindings)? != expected {
        return Err(InventoryError::Witness);
    }
    let supplemental: SupplementalReference =
        toml::from_str(pinned_supplemental).map_err(|_error| InventoryError::Input)?;
    if supplemental.schema_version != 1 || supplemental.kind != "supplemental_schema_reference_only"
    {
        return Err(InventoryError::Witness);
    }
    let supplemental_keys = supplemental
        .schema
        .iter()
        .map(|pin| (&pin.key, &pin.name))
        .collect::<BTreeSet<_>>();
    if supplemental_keys.len() != supplemental.schema.len() {
        return Err(InventoryError::Membership);
    }
    let actual = owned_witnesses(
        bidders_root,
        &expected,
        &revision,
        repository,
        &supplemental,
    )?;
    let mut schemas = Vec::new();
    let mut schema_bytes = 0_u64;
    for witness in actual.into_values() {
        let bytes = read_bounded(&bidders_root.join(&witness.path), MAX_SCHEMA_BYTES)?;
        let length = u64::try_from(bytes.len()).map_err(|_error| InventoryError::Limit)?;
        if length != witness.bytes || hex::encode(Sha256::digest(&bytes)) != witness.sha256 {
            return Err(InventoryError::Witness);
        }
        schema_bytes = schema_bytes
            .checked_add(length)
            .ok_or(InventoryError::Limit)?;
        if schema_bytes > expected_counts.schema_bytes {
            return Err(InventoryError::Counts);
        }
        let schema = serde_json::from_slice(&bytes).map_err(|_error| InventoryError::Input)?;
        schemas.push(SchemaInventory {
            witness,
            audit: audit_schema(&schema)?,
        });
    }
    let largest_schema = schemas
        .iter()
        .max_by_key(|row| row.witness.bytes)
        .map(|row| row.witness.clone())
        .ok_or(InventoryError::Membership)?;
    let largest_pin = expected
        .values()
        .max_by_key(|row| row.bytes)
        .ok_or(InventoryError::Membership)?;
    if schema_bytes != expected_counts.schema_bytes
        || largest_schema.bytes != expected_counts.largest_schema_bytes
        || largest_schema != *largest_pin
    {
        return Err(InventoryError::Counts);
    }
    Ok(Inventory {
        root: bidders_root.to_owned(),
        revision,
        schema_bytes,
        largest_schema,
        schemas,
    })
}

fn guard_structure(root: &Value) -> Result<(), InventoryError> {
    let mut pending = vec![(root, 0_usize)];
    let mut nodes = 0_usize;
    while let Some((value, depth)) = pending.pop() {
        nodes = nodes.checked_add(1).ok_or(InventoryError::Limit)?;
        if nodes > MAX_NODES || depth > MAX_DEPTH {
            return Err(InventoryError::Limit);
        }
        let next = depth.checked_add(1).ok_or(InventoryError::Limit)?;
        match value {
            Value::Array(items) => {
                if items.len() > MAX_NODES {
                    return Err(InventoryError::Limit);
                }
                pending.extend(items.iter().map(|item| (item, next)));
            }
            Value::Object(items) => {
                if items.len() > MAX_NODES {
                    return Err(InventoryError::Limit);
                }
                pending.extend(items.values().map(|item| (item, next)));
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    Ok(())
}

fn pointer(parent: &str, token: &str) -> String {
    format!("{parent}/{}", token.replace('~', "~0").replace('/', "~1"))
}

fn draft(schema: &Value, inherited: &Draft) -> Result<Draft, InventoryError> {
    let Some(value) = schema.get("$schema") else {
        return Ok(inherited.clone());
    };
    match value.as_str().ok_or(InventoryError::Structure)? {
        "http://json-schema.org/draft-04/schema#" | "https://json-schema.org/draft-04/schema#" => {
            Ok(Draft::Draft4)
        }
        "https://json-schema.org/draft/2020-12/schema"
        | "https://json-schema.org/draft/2020-12/schema#" => Ok(Draft::Draft202012),
        other => Ok(Draft::Other(other.to_owned())),
    }
}

fn validation_keyword(keyword: &str, modern: bool) -> bool {
    matches!(
        keyword,
        "multipleOf"
            | "maximum"
            | "exclusiveMaximum"
            | "minimum"
            | "exclusiveMinimum"
            | "maxLength"
            | "minLength"
            | "pattern"
            | "maxItems"
            | "minItems"
            | "uniqueItems"
            | "maxProperties"
            | "minProperties"
            | "required"
            | "enum"
            | "type"
            | "format"
            | "properties"
            | "patternProperties"
            | "additionalProperties"
            | "items"
            | "allOf"
            | "anyOf"
            | "oneOf"
            | "not"
    ) || (!modern && matches!(keyword, "additionalItems" | "dependencies"))
        || (modern
            && matches!(
                keyword,
                "const"
                    | "if"
                    | "then"
                    | "else"
                    | "contains"
                    | "minContains"
                    | "maxContains"
                    | "prefixItems"
                    | "propertyNames"
                    | "dependentRequired"
                    | "dependentSchemas"
                    | "unevaluatedItems"
                    | "unevaluatedProperties"
            ))
}

fn annotation_keyword(keyword: &str) -> bool {
    matches!(
        keyword,
        "title"
            | "description"
            | "default"
            | "examples"
            | "$comment"
            | "deprecated"
            | "readOnly"
            | "writeOnly"
    )
}

fn known_annotation(keyword: &str, modern: bool) -> bool {
    matches!(keyword, "title" | "description" | "default")
        || (modern && annotation_keyword(keyword))
}

fn metadata_keyword(keyword: &str, modern: bool) -> bool {
    matches!(keyword, "$schema" | "$ref")
        || (!modern && matches!(keyword, "id" | "definitions"))
        || (modern
            && matches!(
                keyword,
                "$id" | "$defs" | "$anchor" | "$dynamicAnchor" | "$dynamicRef" | "$vocabulary"
            ))
}

type Graph = BTreeMap<String, BTreeSet<String>>;

struct AuditBuilder {
    audit: SchemaAudit,
    graph: Graph,
    references: Vec<(String, LocatedString)>,
}

impl AuditBuilder {
    fn visit(
        &mut self,
        schema: &Value,
        location: &str,
        inherited: &Draft,
    ) -> Result<(), InventoryError> {
        let dialect = draft(schema, inherited)?;
        let modern = dialect == Draft::Draft202012;
        self.audit.schema_positions.push(location.to_owned());
        self.graph.entry(location.to_owned()).or_default();
        if modern && schema.is_boolean() {
            return Ok(());
        }
        let object = schema.as_object().ok_or(InventoryError::Structure)?;
        for (keyword, value) in object {
            let path = pointer(location, keyword);
            let occurrence = Occurrence {
                keyword: keyword.clone(),
                pointer: path.clone(),
                value: value.clone(),
            };
            if annotation_keyword(keyword) {
                self.audit.annotations.push(occurrence.clone());
            }
            if known_annotation(keyword, modern) {
                continue;
            }
            if validation_keyword(keyword, modern) {
                self.audit.validation_keywords.push(occurrence);
            } else if metadata_keyword(keyword, modern) {
                self.audit.metadata_keywords.push(occurrence);
            } else {
                self.audit.unknown_keywords.push(occurrence);
            }
            match keyword.as_str() {
                "$ref" | "$dynamicRef" if keyword == "$ref" || modern => {
                    self.references.push((
                        location.to_owned(),
                        LocatedString {
                            pointer: path.clone(),
                            value: value.as_str().ok_or(InventoryError::Structure)?.to_owned(),
                        },
                    ));
                }
                "format" => self.audit.formats.push(LocatedString {
                    pointer: path.clone(),
                    value: value.as_str().ok_or(InventoryError::Structure)?.to_owned(),
                }),
                "pattern" => self.audit.patterns.push(RegexPattern {
                    pointer: path.clone(),
                    value: value.as_str().ok_or(InventoryError::Structure)?.to_owned(),
                    kind: PatternKind::Pattern,
                }),
                _ => {}
            }
            self.children(keyword, value, &path, location, &dialect)?;
        }
        Ok(())
    }

    fn child(
        &mut self,
        value: &Value,
        path: &str,
        parent: &str,
        dialect: &Draft,
    ) -> Result<(), InventoryError> {
        self.graph
            .entry(parent.to_owned())
            .or_default()
            .insert(path.to_owned());
        self.visit(value, path, dialect)
    }

    fn children(
        &mut self,
        keyword: &str,
        value: &Value,
        path: &str,
        parent: &str,
        dialect: &Draft,
    ) -> Result<(), InventoryError> {
        let modern = *dialect == Draft::Draft202012;
        match keyword {
            "properties" | "patternProperties" | "definitions" | "$defs" | "dependentSchemas"
                if matches!(keyword, "properties" | "patternProperties")
                    || (!modern && keyword == "definitions")
                    || (modern && matches!(keyword, "$defs" | "dependentSchemas")) =>
            {
                for (name, child) in value.as_object().ok_or(InventoryError::Structure)? {
                    let child_path = pointer(path, name);
                    if keyword == "patternProperties" {
                        self.audit.patterns.push(RegexPattern {
                            pointer: child_path.clone(),
                            value: name.clone(),
                            kind: PatternKind::PropertyName,
                        });
                    }
                    self.child(child, &child_path, parent, dialect)?;
                }
            }
            "dependencies" if !modern => {
                for (name, child) in value.as_object().ok_or(InventoryError::Structure)? {
                    if !child.is_array() {
                        self.child(child, &pointer(path, name), parent, dialect)?;
                    }
                }
            }
            "allOf" | "anyOf" | "oneOf" | "prefixItems" if keyword != "prefixItems" || modern => {
                for (index, child) in value
                    .as_array()
                    .ok_or(InventoryError::Structure)?
                    .iter()
                    .enumerate()
                {
                    self.child(child, &pointer(path, &index.to_string()), parent, dialect)?;
                }
            }
            "items" if !modern && value.is_array() => {
                for (index, child) in value
                    .as_array()
                    .ok_or(InventoryError::Structure)?
                    .iter()
                    .enumerate()
                {
                    self.child(child, &pointer(path, &index.to_string()), parent, dialect)?;
                }
            }
            "additionalProperties" | "additionalItems" if !modern && value.is_boolean() => {}
            "items" | "not" | "additionalProperties" => self.child(value, path, parent, dialect)?,
            "additionalItems" if !modern => self.child(value, path, parent, dialect)?,
            "if"
            | "then"
            | "else"
            | "contains"
            | "propertyNames"
            | "unevaluatedProperties"
            | "unevaluatedItems"
                if modern =>
            {
                self.child(value, path, parent, dialect)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn finish(mut self, schema: &Value) -> SchemaAudit {
        for (source, reference) in self.references {
            let local_fragment = if reference.value.is_empty() {
                Some("")
            } else {
                reference.value.strip_prefix('#')
            };
            let Some(fragment) = local_fragment else {
                self.audit.external_references.push(reference);
                continue;
            };
            let target = if reference.pointer.ends_with("/$dynamicRef")
                || scoped_reference(&source, &self.audit.metadata_keywords)
            {
                LocalTarget::Unresolved
            } else {
                local_target(schema, fragment, &self.graph)
            };
            if let LocalTarget::Schema(destination) = &target {
                self.graph
                    .entry(source)
                    .or_default()
                    .insert(destination.clone());
            }
            self.audit.local_references.push(LocalReference {
                pointer: reference.pointer,
                value: reference.value,
                target,
            });
        }
        self.audit.local_reference_cycles = cyclic_components(&self.graph);
        self.audit.schema_positions.sort_unstable();
        for items in [
            &mut self.audit.validation_keywords,
            &mut self.audit.annotations,
            &mut self.audit.metadata_keywords,
            &mut self.audit.unknown_keywords,
        ] {
            items.sort_unstable_by(|left, right| left.pointer.cmp(&right.pointer));
        }
        self.audit
            .formats
            .sort_unstable_by(|left, right| left.pointer.cmp(&right.pointer));
        self.audit
            .patterns
            .sort_unstable_by(|left, right| left.pointer.cmp(&right.pointer));
        self.audit
            .local_references
            .sort_unstable_by(|left, right| left.pointer.cmp(&right.pointer));
        self.audit
            .external_references
            .sort_unstable_by(|left, right| left.pointer.cmp(&right.pointer));
        self.audit
    }
}

fn scoped_reference(source: &str, metadata: &[Occurrence]) -> bool {
    metadata.iter().any(|item| {
        if !matches!(item.keyword.as_str(), "id" | "$id")
            || item
                .value
                .as_str()
                .is_some_and(|value| value.is_empty() || value.starts_with('#'))
        {
            return false;
        }
        let Some((owner, _keyword)) = item.pointer.rsplit_once('/') else {
            return false;
        };
        !owner.is_empty()
            && (source == owner
                || source
                    .strip_prefix(owner)
                    .is_some_and(|suffix| suffix.starts_with('/')))
    })
}

fn local_target(schema: &Value, fragment: &str, positions: &Graph) -> LocalTarget {
    let Ok(decoded) = percent_encoding::percent_decode_str(fragment).decode_utf8() else {
        return LocalTarget::Unresolved;
    };
    if !decoded.is_empty() && !decoded.starts_with('/') {
        return LocalTarget::Unresolved;
    }
    let mut bytes = decoded.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'~' && !matches!(bytes.next(), Some(b'0' | b'1')) {
            return LocalTarget::Unresolved;
        }
    }
    if schema.pointer(&decoded).is_none() {
        return LocalTarget::Unresolved;
    }
    if positions.contains_key(decoded.as_ref()) {
        LocalTarget::Schema(decoded.into_owned())
    } else {
        LocalTarget::NonSchema(decoded.into_owned())
    }
}

fn cyclic_components(graph: &Graph) -> Vec<Vec<String>> {
    // Iterative Kosaraju traversal keeps reference chains off the native call stack.
    let mut visited = BTreeSet::new();
    let mut order = Vec::new();
    for root in graph.keys() {
        let mut pending = vec![(root.clone(), false)];
        while let Some((node, finishing)) = pending.pop() {
            if finishing {
                order.push(node);
                continue;
            }
            if visited.insert(node.clone()) {
                pending.push((node.clone(), true));
                if let Some(edges) = graph.get(&node) {
                    pending.extend(edges.iter().rev().map(|next| (next.clone(), false)));
                }
            }
        }
    }
    let mut reverse = Graph::new();
    for (source, edges) in graph {
        for target in edges {
            reverse
                .entry(target.clone())
                .or_default()
                .insert(source.clone());
        }
    }
    visited.clear();
    let mut cycles = Vec::new();
    for root in order.into_iter().rev() {
        if visited.contains(&root) {
            continue;
        }
        let mut component = Vec::new();
        let mut pending = vec![root.clone()];
        while let Some(node) = pending.pop() {
            if visited.insert(node.clone()) {
                if let Some(edges) = reverse.get(&node) {
                    pending.extend(edges.iter().cloned());
                }
                component.push(node);
            }
        }
        if component.len() > 1 || graph.get(&root).is_some_and(|edges| edges.contains(&root)) {
            component.sort_unstable();
            cycles.push(component);
        }
    }
    cycles.sort_unstable();
    cycles
}

/// Inspect schema positions only; this does not determine engine support or validity.
/// Empty/fragment-only JSON Pointers form a local graph, never retrieving external refs.
/// Named anchors, nested URI scopes and dynamic references remain explicitly unresolved.
pub fn audit_schema(schema: &Value) -> Result<SchemaAudit, InventoryError> {
    guard_structure(schema)?;
    let dialect = draft(schema, &Draft::Unspecified)?;
    let audit = SchemaAudit {
        draft: dialect.clone(),
        schema_positions: Vec::new(),
        validation_keywords: Vec::new(),
        annotations: Vec::new(),
        metadata_keywords: Vec::new(),
        unknown_keywords: Vec::new(),
        formats: Vec::new(),
        patterns: Vec::new(),
        local_references: Vec::new(),
        external_references: Vec::new(),
        local_reference_cycles: Vec::new(),
    };
    let mut builder = AuditBuilder {
        audit,
        graph: Graph::new(),
        references: Vec::new(),
    };
    builder.visit(schema, "", &dialect)?;
    Ok(builder.finish(schema))
}

#[cfg(test)]
mod tests {
    use super::fixture::inventory;
    use super::{
        Draft, InventoryError, LocalTarget, MAX_DEPTH, MAX_DOCUMENT_BYTES, MAX_SCHEMA_BYTES,
        PatternKind, audit_schema,
    };
    use serde_json::{Value, json};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn audit(value: &Value) -> super::SchemaAudit {
        audit_schema(value).expect("structural audit")
    }

    #[test]
    fn property_names_are_not_keywords_but_nested_schemas_are() {
        let result = audit(&json!({
            "$schema": "http://json-schema.org/draft-04/schema#",
            "properties": {
                "format": {"type": "string"},
                "enum": {"format": "hostname"},
                "a/b~c": {"pattern": "\\d+"}
            }
        }));
        assert_eq!(result.draft, Draft::Draft4);
        assert_eq!(result.formats.len(), 1);
        assert_eq!(result.formats[0].pointer, "/properties/enum/format");
        assert_eq!(result.formats[0].value, "hostname");
        assert_eq!(result.patterns[0].pointer, "/properties/a~1b~0c/pattern");
        assert_eq!(result.patterns[0].value, "\\d+");
        assert!(result.unknown_keywords.is_empty());
        assert!(
            !result
                .validation_keywords
                .iter()
                .any(|item| item.keyword == "enum")
        );
    }

    #[test]
    fn annotations_enum_and_unknown_payloads_are_opaque() {
        let decoy = json!({"format": "uri", "$ref": "#", "pattern": "bad", "type": "object"});
        let result = audit(&json!({
            "title": "title", "description": "description",
            "default": decoy, "examples": [decoy], "enum": [decoy],
            "x-extension": {"properties": {"hidden": decoy}}
        }));
        assert_eq!(result.schema_positions, [""]);
        assert_eq!(result.annotations.len(), 4);
        assert_eq!(result.validation_keywords.len(), 1);
        assert_eq!(result.validation_keywords[0].keyword, "enum");
        assert_eq!(
            result
                .unknown_keywords
                .iter()
                .map(|item| item.keyword.as_str())
                .collect::<Vec<_>>(),
            ["examples", "x-extension"]
        );
        assert!(result.formats.is_empty());
        assert!(result.patterns.is_empty());
        assert!(result.local_references.is_empty());
    }

    #[test]
    fn draft4_container_context_selects_only_schema_children() {
        let result = audit(&json!({
            "$schema": "http://json-schema.org/draft-04/schema#",
            "properties": {"properties": {"format": "property"}},
            "patternProperties": {"format": {"format": "pattern-property"}},
            "definitions": {"format": {"format": "definition"}},
            "additionalProperties": {"format": "additional-property"},
            "items": [{"format": "tuple"}, {"items": {"format": "item"}}],
            "additionalItems": {"format": "additional-item"},
            "dependencies": {"format": ["format"], "schema": {"format": "dependency"}},
            "allOf": [{"format": "all"}], "anyOf": [{"format": "any"}],
            "oneOf": [{"format": "one"}], "not": {"format": "not"}
        }));
        let locations: Vec<_> = result
            .formats
            .iter()
            .map(|item| item.pointer.as_str())
            .collect();
        assert_eq!(
            locations,
            [
                "/additionalItems/format",
                "/additionalProperties/format",
                "/allOf/0/format",
                "/anyOf/0/format",
                "/definitions/format/format",
                "/dependencies/schema/format",
                "/items/0/format",
                "/items/1/items/format",
                "/not/format",
                "/oneOf/0/format",
                "/patternProperties/format/format",
                "/properties/properties/format"
            ]
        );
        assert_eq!(result.patterns.len(), 1);
        assert_eq!(result.patterns[0].kind, PatternKind::PropertyName);
        assert_eq!(result.patterns[0].pointer, "/patternProperties/format");
        assert!(
            !result
                .schema_positions
                .iter()
                .any(|item| item == "/dependencies/format")
        );
    }

    #[test]
    fn booleans_do_not_turn_draft4_switches_into_schema_positions() {
        let result = audit(&json!({"additionalProperties": false, "additionalItems": true}));
        assert_eq!(result.schema_positions, [""]);
        assert_eq!(result.validation_keywords.len(), 2);
    }

    #[test]
    fn local_reference_edges_and_recursive_components_are_separate_from_external_refs() {
        let result = audit(&json!({
            "definitions": {
                "a": {"$ref": "#/definitions/b"},
                "b": {"$ref": "#/definitions/a"},
                "self": {"$ref": "#/definitions/self"},
                "recursive": {"properties": {"child": {"$ref": "#/definitions/recursive"}}}
            },
            "allOf": [
                {"$ref": "#/definitions/a"}, {"$ref": "other.json#/definitions/a"},
                {"$ref": "https://example.invalid/schema"}, {"$ref": "file:///tmp/schema.json"},
                {"$ref": "#/missing"}, {"$ref": "#/default"}
            ],
            "default": {"$ref": "#"}
        }));
        assert_eq!(result.local_references.len(), 7);
        assert_eq!(result.external_references.len(), 3);
        assert_eq!(
            result.local_reference_cycles,
            [
                vec!["/definitions/a", "/definitions/b"],
                vec![
                    "/definitions/recursive",
                    "/definitions/recursive/properties/child"
                ],
                vec!["/definitions/self"]
            ]
        );
        let missing = result
            .local_references
            .iter()
            .find(|item| item.value == "#/missing")
            .expect("missing ref retained");
        assert_eq!(missing.target, LocalTarget::Unresolved);
        let opaque = result
            .local_references
            .iter()
            .find(|item| item.value == "#/default")
            .expect("non-schema ref retained");
        assert_eq!(opaque.target, LocalTarget::NonSchema("/default".to_owned()));
        let root = audit(&json!({"$ref": "#"}));
        assert_eq!(root.local_reference_cycles, [vec![""]]);
    }

    #[test]
    fn pointer_fragments_decode_without_treating_anchors_as_pointers() {
        let result = audit(&json!({
            "definitions": {"a/b~ c": {}},
            "anyOf": [{"$ref": "#/definitions/a~1b~0%20c"}, {"$ref": "#anchor"}]
        }));
        assert_eq!(
            result.local_references[0].target,
            LocalTarget::Schema("/definitions/a~1b~0 c".to_owned())
        );
        assert_eq!(result.local_references[1].target, LocalTarget::Unresolved);
    }

    #[test]
    fn empty_uri_is_a_local_root_reference() {
        let result = audit(&json!({"$ref": ""}));
        assert!(result.external_references.is_empty());
        assert_eq!(
            result.local_references[0].target,
            LocalTarget::Schema(String::new())
        );
        assert_eq!(result.local_reference_cycles, [vec![""]]);
    }

    #[test]
    fn scoped_and_dynamic_references_are_retained_without_claiming_uri_resolution() {
        let result = audit(&json!({
            "definitions": {
                "scoped": {"id": "nested.json", "properties": {"child": {"$ref": "#"}}}
            }
        }));
        assert_eq!(result.local_references[0].target, LocalTarget::Unresolved);
        assert!(result.local_reference_cycles.is_empty());
        let dynamic = audit(&json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$dynamicRef": "#"
        }));
        assert_eq!(dynamic.local_references[0].target, LocalTarget::Unresolved);
        assert!(dynamic.local_reference_cycles.is_empty());
    }

    #[test]
    fn invalid_pointer_escapes_do_not_resolve_to_schema_names() {
        let result = audit(&json!({
            "definitions": {"a~2b": {}},
            "$ref": "#/definitions/a~2b"
        }));
        assert_eq!(result.local_references[0].target, LocalTarget::Unresolved);
    }

    #[test]
    fn draft202012_conditional_positions_are_audited_without_accepting_new_draft4_keywords() {
        let body = json!({"if": {"format": "hostname"}, "then": {"const": 1_u64}, "else": {"not": {"type": "null"}}});
        let old = audit(&body);
        assert_eq!(old.draft, Draft::Unspecified);
        assert!(old.formats.is_empty());
        assert_eq!(old.unknown_keywords.len(), 3);
        let mut modern = body;
        modern["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
        let result = audit(&modern);
        assert_eq!(result.draft, Draft::Draft202012);
        assert_eq!(result.formats.len(), 1);
        assert!(result.unknown_keywords.is_empty());
        assert!(
            result
                .validation_keywords
                .iter()
                .any(|item| item.pointer == "/then/const")
        );
    }

    #[test]
    fn opaque_payloads_are_still_subject_to_structural_depth_limits() {
        let mut value = json!(null);
        for _ in 0..=MAX_DEPTH {
            value = json!([value]);
        }
        assert_eq!(
            audit_schema(&json!({"default": value})),
            Err(InventoryError::Limit)
        );
    }

    struct Fixture {
        directory: TempDir,
        bidders: PathBuf,
        source: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let (directory, bidders, source) = bidder_owned::OwnedFixture::new()
                .expect("independent owned-layout fixture")
                .into_parts();
            Self {
                directory,
                bidders,
                source,
            }
        }

        fn check(&self) -> Result<super::Inventory, InventoryError> {
            inventory(&self.bidders, &self.source)
        }

        fn manifest(&self, change: impl FnOnce(&mut toml::Value)) {
            let path = self.bidders.join("fake_base/bidder.toml");
            let mut value = toml::from_str(&fs::read_to_string(&path).expect("manifest bytes"))
                .expect("manifest TOML");
            change(&mut value);
            fs::write(path, toml::to_string(&value).expect("manifest encoding"))
                .expect("write mutated manifest");
        }

        fn source(&self, change: impl FnOnce(&mut toml::Value)) {
            let mut value =
                toml::from_str(&fs::read_to_string(&self.source).expect("inventory text"))
                    .expect("inventory TOML");
            change(&mut value);
            fs::write(
                &self.source,
                toml::to_string(&value).expect("inventory encoding"),
            )
            .expect("write mutated inventory");
        }
    }

    fn fake(manifest: &mut toml::Value) -> &mut toml::Value {
        manifest["provenance"]["artifacts"]
            .as_array_mut()
            .expect("manifest artifacts")
            .iter_mut()
            .find(|row| row["source_path"].as_str() == Some("static/bidder-params/fake_base.json"))
            .expect("fake witness")
    }

    #[test]
    fn changed_raw_schema_bytes_are_rejected_before_parsing() {
        let fixture = Fixture::new();
        let path = fixture.bidders.join("fake_base/parameters.json");
        let mut bytes = fs::read(&path).expect("schema bytes");
        bytes[0] = b'[';
        fs::write(path, bytes).expect("changed schema");
        assert_eq!(fixture.check(), Err(InventoryError::Witness));
    }

    #[test]
    fn missing_extra_and_same_count_renamed_schemas_are_rejected() {
        for mutation in ["missing", "extra", "renamed"] {
            let fixture = Fixture::new();
            let schema = fixture.bidders.join("fake_base/parameters.json");
            match mutation {
                "missing" => fs::remove_file(schema).expect("remove fixture"),
                "extra" => fs::write(
                    fixture.bidders.join("fake_base/schemas/extra.schema.json"),
                    serde_json::to_vec(&json!({})).expect("JSON fixture"),
                )
                .expect("extra schema"),
                _ => fs::rename(schema, fixture.bidders.join("fake_base/renamed.json"))
                    .expect("rename schema"),
            }
            assert_eq!(
                fixture.check(),
                Err(InventoryError::Membership),
                "{mutation}"
            );
        }
    }

    #[test]
    fn either_revision_and_a_consistently_repinned_pair_must_match_literal_pin() {
        for (manifest, source) in [(true, false), (false, true), (true, true)] {
            let fixture = Fixture::new();
            if manifest {
                fixture.manifest(|value| {
                    value["provenance"]["sources"]["upstream"]["revision"] =
                        toml::Value::String("0".repeat(40));
                });
            }
            if source {
                fixture.source(|value| value["revision"] = toml::Value::String("0".repeat(40)));
            }
            assert_eq!(fixture.check(), Err(InventoryError::Revision));
        }
    }

    #[test]
    fn every_manifest_schema_witness_field_is_checked() {
        for (retained, field, value) in [
            (false, "source_blob", toml::Value::String("0".repeat(40))),
            (false, "source_sha256", toml::Value::String("0".repeat(64))),
            (true, "source_sha256", toml::Value::String("0".repeat(64))),
            (
                false,
                "source_path",
                toml::Value::String("static/bidder-params/other.json".into()),
            ),
            (
                true,
                "source_path",
                toml::Value::String("schemas/other.schema.json".into()),
            ),
            (true, "source_bytes", toml::Value::Integer(3_777)),
            (false, "source_bytes", toml::Value::Integer(3_777)),
        ] {
            let fixture = Fixture::new();
            fixture.manifest(|manifest| {
                let row = fake(manifest);
                if retained {
                    row["retained"][field] = value;
                } else {
                    row[field] = value;
                }
            });
            assert_eq!(
                fixture.check(),
                Err(InventoryError::Witness),
                "unchecked witness field: retained={retained} {field}"
            );
        }
    }

    #[test]
    fn manifest_duplicate_and_missing_schema_rows_are_rejected() {
        for duplicate in [false, true] {
            let fixture = Fixture::new();
            fixture.manifest(|manifest| {
                let row = fake(manifest).clone();
                let rows = manifest["provenance"]["artifacts"]
                    .as_array_mut()
                    .expect("artifacts");
                if duplicate {
                    rows.push(row);
                } else {
                    rows.retain(|item| {
                        item["source_path"].as_str() != Some("static/bidder-params/fake_base.json")
                    });
                }
            });
            assert_eq!(fixture.check(), Err(InventoryError::Membership));
        }
    }

    #[test]
    fn counts_and_schema_size_witnesses_are_literal_not_merely_self_consistent() {
        for (field, value) in [
            ("schemas", 260),
            ("schema_bytes", 164_517),
            ("largest_schema_bytes", 3_777),
        ] {
            let fixture = Fixture::new();
            fixture.source(|inventory| {
                inventory["counts"][field] = toml::Value::Integer(value);
            });
            assert_eq!(fixture.check(), Err(InventoryError::Counts), "{field}");
        }
        for retained in [false, true] {
            let fixture = Fixture::new();
            fixture.manifest(|manifest| {
                let row = fake(manifest);
                if retained {
                    row["retained"]["source_bytes"] = toml::Value::Integer(3_777);
                } else {
                    row["source_bytes"] = toml::Value::Integer(3_777);
                }
            });
            assert_eq!(fixture.check(), Err(InventoryError::Witness));
        }
    }

    #[test]
    fn coordinated_manifest_and_source_digest_drift_cannot_replace_committed_witnesses() {
        let fixture = Fixture::new();
        fixture.manifest(|manifest| {
            let row = fake(manifest);
            row["source_sha256"] = toml::Value::String("0".repeat(64));
            row["retained"]["source_sha256"] = toml::Value::String("0".repeat(64));
        });
        fixture.source(|value| {
            let row = value["schema"]
                .as_array_mut()
                .expect("schemas")
                .iter_mut()
                .find(|row| row["name"].as_str() == Some("fake_base"))
                .expect("schema witness");
            row["sha256"] = toml::Value::String("0".repeat(64));
        });
        assert_eq!(fixture.check(), Err(InventoryError::Witness));
    }

    #[test]
    fn a_local_exclusion_cannot_hide_a_pinned_schema() {
        let fixture = Fixture::new();
        fixture.manifest(|manifest| {
            manifest["provenance"]["sources"]["upstream"]["revision"] =
                toml::Value::String("2".repeat(40));
        });
        let directory = fixture.bidders.join("fake_base/tests/schema");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("inventory-exclusion.toml"), "name='fake_base'\nrepository='https://example.invalid/fake.git'\nrevision='2222222222222222222222222222222222222222'\n").unwrap();
        assert_eq!(fixture.check(), Err(InventoryError::Witness));
    }

    #[test]
    fn unused_and_same_count_replaced_implementation_owners_are_rejected() {
        for extra in [true, false] {
            let fixture = Fixture::new();
            fixture.source(|source| {
                let rows = source["implementations"].as_array_mut().unwrap();
                let mut row = rows[0].clone();
                row["source_directory"] = toml::Value::String("adapters/fake_unowned".into());
                row["internal_key"] = toml::Value::String("fake_unowned".into());
                if extra {
                    rows.push(row);
                } else {
                    rows[0] = row;
                }
            });
            assert_eq!(fixture.check(), Err(InventoryError::Membership));
        }
    }

    #[test]
    fn source_witness_membership_and_each_field_are_checked() {
        for field in [
            "name",
            "path",
            "blob",
            "bytes",
            "sha256",
            "duplicate",
            "missing",
        ] {
            let fixture = Fixture::new();
            fixture.source(|value| {
                let rows = value["schema"].as_array_mut().expect("schema rows");
                match field {
                    "duplicate" => rows.push(rows[0].clone()),
                    "missing" => {
                        rows.pop().expect("schema row");
                    }
                    "bytes" => rows[0][field] = toml::Value::Integer(1),
                    _ => rows[0][field] = toml::Value::String("wrong".to_owned()),
                }
            });
            let error = match field {
                "name" | "path" => InventoryError::Path,
                "duplicate" | "missing" => InventoryError::Membership,
                _ => InventoryError::Witness,
            };
            assert_eq!(fixture.check(), Err(error), "{field}");
        }
    }

    #[test]
    fn inventory_order_is_independent_of_witness_array_order() {
        let fixture = Fixture::new();
        let original = fixture.check().expect("valid copied fixtures");
        fixture.manifest(|value| {
            value["provenance"]["artifacts"]
                .as_array_mut()
                .expect("manifest artifacts")
                .reverse();
        });
        fixture.source(|value| {
            value["schema"]
                .as_array_mut()
                .expect("schema rows")
                .reverse();
        });
        assert_eq!(fixture.check().expect("reordered fixtures"), original);
    }

    #[test]
    fn unsafe_manifest_paths_and_schema_subdirectories_are_rejected() {
        for path in [
            "schemas/../parameters.json",
            "/parameters.json",
            "schemas/nested/fake.schema.json",
            "schemas\\fake.schema.json",
        ] {
            let fixture = Fixture::new();
            fixture.manifest(|manifest| {
                fake(manifest)["retained"]["source_path"] = toml::Value::String(path.into());
            });
            assert_eq!(
                fixture.check(),
                Err(InventoryError::Path),
                "unsafe path: {path}"
            );
        }
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.bidders.join("fake_base/schemas/nested"))
            .expect("extra directory");
        assert_eq!(fixture.check(), Err(InventoryError::Membership));
    }

    #[test]
    fn missing_required_provenance_and_oversized_files_refuse_inventory() {
        let missing = Fixture::new();
        fs::remove_file(&missing.source).expect("remove source witness");
        assert_eq!(missing.check(), Err(InventoryError::Input));
        for (path, bound) in [
            ("fake_base/parameters.json", MAX_SCHEMA_BYTES),
            ("fake_base/bidder.toml", MAX_DOCUMENT_BYTES),
        ] {
            let fixture = Fixture::new();
            let file = fs::OpenOptions::new()
                .write(true)
                .open(fixture.bidders.join(path))
                .expect("fixture file");
            file.set_len(bound.checked_add(1).expect("bounded length"))
                .expect("oversized fixture");
            assert_eq!(fixture.check(), Err(InventoryError::Limit), "{path}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn schema_file_directory_manifest_and_source_symlinks_are_rejected() {
        use std::os::unix::fs::symlink;

        for relative in [
            "fake_base/parameters.json",
            "fake_base/schemas",
            "fake_base/bidder.toml",
            "../source-inventory.toml",
        ] {
            let fixture = Fixture::new();
            let path = fixture.bidders.join(relative);
            let moved = fixture.directory.path().join("original");
            fs::rename(&path, &moved).expect("move symlink target");
            symlink(moved, path).expect("source symlink");
            assert_eq!(fixture.check(), Err(InventoryError::Path), "{relative}");
        }
    }

    mod bidder_owned {
        use super::super::fixture::inventory;
        use super::super::{Inventory, InventoryError, MAX_DOCUMENT_BYTES, MAX_SCHEMA_BYTES};
        use serde_json::Value;
        use sha2::{Digest as _, Sha256};
        use std::error::Error;
        use std::fs;
        use std::path::PathBuf;
        use tempfile::TempDir;

        type TestResult<T = ()> = Result<T, Box<dyn Error>>;

        pub(super) struct OwnedFixture {
            directory: TempDir,
            bidders: PathBuf,
            source: PathBuf,
        }

        impl OwnedFixture {
            pub(super) fn new() -> TestResult<Self> {
                let super::super::fixture::Fixture {
                    directory,
                    bidders,
                    source,
                } = super::super::fixture::Fixture::new()?;
                Ok(Self {
                    directory,
                    bidders,
                    source,
                })
            }

            pub(super) fn into_parts(self) -> (TempDir, PathBuf, PathBuf) {
                (self.directory, self.bidders, self.source)
            }

            fn check(&self) -> Result<Inventory, InventoryError> {
                inventory(&self.bidders, &self.source)
            }

            fn valid(&self) -> TestResult<Inventory> {
                self.check().map_err(|error| {
                    format!("valid bidder-owned baseline failed: {error:?}").into()
                })
            }

            fn manifest(
                &self,
                key: &str,
                change: impl FnOnce(&mut toml::Value) -> TestResult,
            ) -> TestResult {
                let path = self.bidders.join(key).join("bidder.toml");
                let mut value = toml::from_str(&fs::read_to_string(&path)?)?;
                change(&mut value)?;
                fs::write(path, toml::to_string(&value)?)?;
                Ok(())
            }

            fn source(&self, change: impl FnOnce(&mut toml::Value) -> TestResult) -> TestResult {
                let mut value = toml::from_str(&fs::read_to_string(&self.source)?)?;
                change(&mut value)?;
                fs::write(&self.source, toml::to_string(&value)?)?;
                Ok(())
            }

            fn add_supplemental(&self) -> TestResult {
                let destination = self.bidders.join("fake_supplemental");
                fs::create_dir_all(destination.join("tests/schema"))?;
                let mut manifest: toml::Value = toml::from_str(&fs::read_to_string(
                    self.bidders.join("fake_secondary/bidder.toml"),
                )?)?;
                manifest["key"] = toml::Value::String("fake_supplemental".into());
                manifest["provenance"]["sources"]["upstream"]["revision"] =
                    toml::Value::String("2".repeat(40));
                let artifacts = manifest["provenance"]["artifacts"].as_array_mut().unwrap();
                artifacts.truncate(1);
                artifacts[0]["source_path"] =
                    toml::Value::String("static/bidder-params/fake_supplemental.json".into());
                fs::write(destination.join("bidder.toml"), toml::to_string(&manifest)?)?;
                fs::write(
                    destination.join("tests/schema/inventory-exclusion.toml"),
                    "name='fake_supplemental'\nrepository='https://example.invalid/fake.git'\nrevision='2222222222222222222222222222222222222222'\n",
                )?;
                fs::copy(
                    self.bidders.join("fake_secondary/parameters.json"),
                    destination.join("parameters.json"),
                )?;
                Ok(())
            }
        }

        fn artifact<'a>(
            manifest: &'a mut toml::Value,
            name: &str,
        ) -> TestResult<&'a mut toml::Value> {
            let source_path = format!("static/bidder-params/{name}.json");
            let rows = manifest["provenance"]["artifacts"]
                .as_array_mut()
                .ok_or("missing artifacts")?;
            rows.iter_mut()
                .find(|row| row["source_path"].as_str() == Some(source_path.as_str()))
                .ok_or_else(|| "missing schema artifact".into())
        }

        #[test]
        fn synthetic_inventory_uses_exact_owned_paths_without_legacy_manifest() {
            let fixture = OwnedFixture::new().unwrap();
            assert!(!fixture.bidders.join("manifest.json").exists());
            assert!(!fixture.bidders.join("schemas").exists());
            let result = fixture.valid().unwrap();
            assert_eq!(result.revision, "1111111111111111111111111111111111111111");
            assert_eq!(result.schemas.len(), 7);
            assert_eq!(result.schema_bytes, super::super::fixture::schema_bytes());
            assert_eq!(result.largest_schema.path, "fake_base/parameters.json");
            assert_eq!(
                result.largest_schema.bytes,
                super::super::fixture::largest_bytes()
            );
            for (name, path) in [
                ("fake_base", "fake_base/parameters.json"),
                ("fake_alias", "fake_base/schemas/fake_alias.schema.json"),
                ("fake_secondary", "fake_secondary/parameters.json"),
                (
                    "fake_secondary_alias",
                    "fake_secondary/schemas/fake_secondary_alias.schema.json",
                ),
                ("fake_numeric", "fake_numeric/parameters.json"),
                ("fake_Camel", "fake_camel/parameters.json"),
                ("fake_format", "fake_format/parameters.json"),
            ] {
                let row = result
                    .schemas
                    .iter()
                    .find(|row| row.witness.name == name)
                    .unwrap();
                assert_eq!(row.witness.path, path, "{name}");
                assert_eq!(
                    row.witness.source_path,
                    format!("static/bidder-params/{name}.json")
                );
                assert!(fixture.bidders.join(&row.witness.path).is_file());
            }
            assert_eq!(fixture.valid().unwrap(), result);
        }

        #[test]
        fn supplemental_schemas_and_local_metadata_do_not_join_pinned_inventory() {
            let fixture = OwnedFixture::new().unwrap();
            fixture.valid().unwrap();
            fixture.add_supplemental().unwrap();
            let local = fixture.bidders.join("fake_local");
            fs::create_dir_all(&local).unwrap();
            fs::write(local.join("bidder.toml"), toml::to_string(&serde_json::json!({"key":"fake_local", "factory":false, "provenance":{"origin":"local"}})).unwrap()).unwrap();
            let result = fixture.valid().unwrap();
            assert_eq!(result.schemas.len(), 7);
            assert_eq!(result.schema_bytes, super::super::fixture::schema_bytes());
            assert!(result.schemas.iter().all(|row| !matches!(
                row.witness.name.as_str(),
                "fake_supplemental" | "fake_local"
            )));
        }

        fn replace_supplemental_capture(
            fixture: &OwnedFixture,
            manifest: &mut toml::Value,
        ) -> TestResult {
            let bytes = serde_json::to_vec(&serde_json::json!({"replaced":true}))?;
            let row = artifact(manifest, "fake_supplemental")?;
            for retained in [false, true] {
                let target = if retained {
                    &mut row["retained"]
                } else {
                    &mut *row
                };
                target["source_bytes"] = toml::Value::Integer(i64::try_from(bytes.len())?);
                target["source_sha256"] = toml::Value::String(hex::encode(Sha256::digest(&bytes)));
            }
            fs::write(
                fixture.bidders.join("fake_supplemental/parameters.json"),
                bytes,
            )?;
            Ok(())
        }

        #[test]
        fn coordinated_supplemental_provenance_and_exclusion_drift_cannot_replace_frozen_pins() {
            for field in [
                "repository",
                "revision",
                "name",
                "source_blob",
                "source_bytes",
                "source_sha256",
                "retained_path",
                "retained_bytes",
                "retained_sha256",
                "capture",
            ] {
                let fixture = OwnedFixture::new().unwrap();
                fixture.add_supplemental().unwrap();
                fixture.valid().unwrap();
                let local = fixture
                    .bidders
                    .join("fake_supplemental/tests/schema/inventory-exclusion.toml");
                let mut exclusion: toml::Value =
                    toml::from_str(&fs::read_to_string(&local).unwrap()).unwrap();
                fixture
                    .manifest("fake_supplemental", |manifest| {
                        match field {
                            "repository" => {
                                let changed = toml::Value::String(
                                    "https://example.invalid/changed.git".into(),
                                );
                                manifest["provenance"]["sources"]["upstream"]["repository"] =
                                    changed.clone();
                                exclusion["repository"] = changed;
                            }
                            "revision" => {
                                let changed = toml::Value::String("3".repeat(40));
                                manifest["provenance"]["sources"]["upstream"]["revision"] =
                                    changed.clone();
                                exclusion["revision"] = changed;
                            }
                            "name" => {
                                artifact(manifest, "fake_supplemental")?["source_path"] =
                                    toml::Value::String(
                                        "static/bidder-params/fake_replaced.json".into(),
                                    );
                                exclusion["name"] = toml::Value::String("fake_replaced".into());
                            }
                            "capture" => {
                                replace_supplemental_capture(&fixture, manifest)?;
                            }
                            other => {
                                let row = artifact(manifest, "fake_supplemental")?;
                                let (target, key) =
                                    if let Some(key) = other.strip_prefix("retained_") {
                                        (
                                            &mut row["retained"],
                                            if key == "path" {
                                                "source_path"
                                            } else if key == "bytes" {
                                                "source_bytes"
                                            } else {
                                                "source_sha256"
                                            },
                                        )
                                    } else {
                                        (&mut *row, other)
                                    };
                                target[key] = if key == "source_bytes" {
                                    toml::Value::Integer(1)
                                } else if key == "source_path" {
                                    toml::Value::String("schemas/replaced.schema.json".into())
                                } else {
                                    toml::Value::String("0".repeat(if key == "source_blob" {
                                        40
                                    } else {
                                        64
                                    }))
                                };
                            }
                        }
                        Ok(())
                    })
                    .unwrap();
                fs::write(local, toml::to_string(&exclusion).unwrap()).unwrap();
                assert_eq!(fixture.check(), Err(InventoryError::Witness), "{field}");
            }
        }

        #[test]
        fn actual_schema_bytes_are_verified_before_parsing() {
            let fixture = OwnedFixture::new().unwrap();
            fixture.valid().unwrap();
            let path = fixture.bidders.join("fake_base/parameters.json");
            let mut bytes = fs::read(&path).unwrap();
            *bytes.first_mut().unwrap() = b'[';
            fs::write(path, bytes).unwrap();
            assert_eq!(fixture.check(), Err(InventoryError::Witness));
        }

        #[test]
        fn each_source_and_retained_witness_field_is_independently_checked() {
            for (retained, field, value) in [
                (false, "source", toml::Value::String("unbound".into())),
                (
                    false,
                    "source_path",
                    toml::Value::String("static/bidder-params/other.json".into()),
                ),
                (false, "source_blob", toml::Value::String("0".repeat(40))),
                (false, "source_bytes", toml::Value::Integer(3_777)),
                (false, "source_sha256", toml::Value::String("0".repeat(64))),
                (
                    true,
                    "source_path",
                    toml::Value::String("schemas/other.schema.json".into()),
                ),
                (true, "source_bytes", toml::Value::Integer(3_777)),
                (true, "source_sha256", toml::Value::String("0".repeat(64))),
            ] {
                let fixture = OwnedFixture::new().unwrap();
                fixture.valid().unwrap();
                fixture
                    .manifest("fake_base", |manifest| {
                        let row = artifact(manifest, "fake_base")?;
                        if retained {
                            row["retained"][field] = value;
                        } else {
                            row[field] = value;
                        }
                        Ok(())
                    })
                    .unwrap();
                assert_eq!(
                    fixture.check(),
                    Err(InventoryError::Witness),
                    "retained={retained} {field}"
                );
            }
        }

        #[test]
        fn schema_membership_is_not_inferred_from_retained_filename_or_other_categories() {
            for mutation in ["category", "missing", "duplicate"] {
                let fixture = OwnedFixture::new().unwrap();
                fixture.valid().unwrap();
                fixture
                    .manifest("fake_base", |manifest| {
                        let row = artifact(manifest, "fake_alias")?;
                        if mutation == "category" {
                            row["category"] = toml::Value::String("metadata".into());
                        } else {
                            let duplicate = row.clone();
                            let rows = manifest["provenance"]["artifacts"]
                                .as_array_mut()
                                .ok_or("missing artifacts")?;
                            if mutation == "duplicate" {
                                rows.push(duplicate);
                            } else {
                                rows.retain(|item| {
                                    item["source_path"].as_str()
                                        != Some("static/bidder-params/fake_alias.json")
                                });
                            }
                        }
                        Ok(())
                    })
                    .unwrap();
                assert_eq!(
                    fixture.check(),
                    Err(InventoryError::Membership),
                    "{mutation}"
                );
            }
        }

        #[test]
        fn missing_extra_and_same_count_renamed_owned_schemas_are_rejected() {
            for mutation in ["missing", "extra", "uppercase", "renamed"] {
                let fixture = OwnedFixture::new().unwrap();
                fixture.valid().unwrap();
                let schema = fixture.bidders.join("fake_base/parameters.json");
                match mutation {
                    "missing" => fs::remove_file(schema).unwrap(),
                    "extra" => {
                        fs::copy(
                            schema,
                            fixture.bidders.join("fake_base/schemas/extra.schema.json"),
                        )
                        .unwrap();
                    }
                    "uppercase" => {
                        fs::copy(schema, fixture.bidders.join("fake_base/extra.JSON")).unwrap();
                    }
                    _ => {
                        fs::rename(schema, fixture.bidders.join("fake_base/renamed.json")).unwrap();
                    }
                }
                assert_eq!(
                    fixture.check(),
                    Err(InventoryError::Membership),
                    "{mutation}"
                );
            }
        }

        #[test]
        fn shared_public_schema_cannot_be_rehomed_by_matching_digests() {
            let fixture = OwnedFixture::new().unwrap();
            fixture.valid().unwrap();
            let mut moved = None;
            fixture
                .manifest("fake_base", |manifest| {
                    moved = Some(artifact(manifest, "fake_alias")?.clone());
                    manifest["provenance"]["artifacts"]
                        .as_array_mut()
                        .ok_or("missing artifacts")?
                        .retain(|row| {
                            row["source_path"].as_str()
                                != Some("static/bidder-params/fake_alias.json")
                        });
                    Ok(())
                })
                .unwrap();
            fixture
                .manifest("fake_secondary", |manifest| {
                    manifest["provenance"]["artifacts"]
                        .as_array_mut()
                        .ok_or("missing artifacts")?
                        .push(moved.take().ok_or("missing moved artifact")?);
                    Ok(())
                })
                .unwrap();
            fs::create_dir_all(fixture.bidders.join("fake_secondary/schemas")).unwrap();
            fs::rename(
                fixture
                    .bidders
                    .join("fake_base/schemas/fake_alias.schema.json"),
                fixture
                    .bidders
                    .join("fake_secondary/schemas/fake_alias.schema.json"),
            )
            .unwrap();
            assert_eq!(fixture.check(), Err(InventoryError::Witness));
        }

        #[test]
        fn original_source_repository_is_exact_not_merely_a_matching_revision() {
            for repository in [
                "https://github.com/prebid/prebid-server",
                "https://github.com/stackpop/prebid-server.git",
            ] {
                let fixture = OwnedFixture::new().unwrap();
                fixture.valid().unwrap();
                fixture
                    .manifest("fake_base", |manifest| {
                        manifest["provenance"]["sources"]["upstream"]["repository"] =
                            toml::Value::String(repository.into());
                        Ok(())
                    })
                    .unwrap();
                assert_eq!(
                    fixture.check(),
                    Err(InventoryError::Witness),
                    "{repository}"
                );
            }
        }

        #[test]
        fn either_revision_and_joint_repins_still_require_independent_original_pin() {
            for (manifests, source) in [(true, false), (false, true), (true, true)] {
                let fixture = OwnedFixture::new().unwrap();
                fixture.valid().unwrap();
                if manifests {
                    for result in fs::read_dir(&fixture.bidders).unwrap() {
                        let entry = result.unwrap();
                        let key = entry.file_name().into_string().unwrap();
                        fixture
                            .manifest(&key, |manifest| {
                                manifest["provenance"]["sources"]["upstream"]["revision"] =
                                    toml::Value::String("0".repeat(40));
                                Ok(())
                            })
                            .unwrap();
                    }
                }
                if source {
                    fixture
                        .source(|value| {
                            value["revision"] = toml::Value::String("0".repeat(40));
                            Ok(())
                        })
                        .unwrap();
                }
                assert_eq!(fixture.check(), Err(InventoryError::Revision));
            }
        }

        #[test]
        fn coordinated_valid_byte_and_digest_drift_cannot_replace_frozen_source() {
            let fixture = OwnedFixture::new().unwrap();
            fixture.valid().unwrap();
            let path = fixture.bidders.join("fake_base/parameters.json");
            let original = fs::read(&path).unwrap();
            let mut schema: Value = serde_json::from_slice(&original).unwrap();
            schema["required"].as_array_mut().unwrap().reverse();
            let bytes = serde_json::to_vec(&schema).unwrap();
            assert_eq!(bytes.len(), original.len());
            assert_ne!(bytes, original);
            assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap(), schema);
            let digest = hex::encode(Sha256::digest(&bytes));
            fs::write(path, bytes).unwrap();
            fixture
                .manifest("fake_base", |manifest| {
                    let row = artifact(manifest, "fake_base")?;
                    row["source_sha256"] = toml::Value::String(digest.clone());
                    row["retained"]["source_sha256"] = toml::Value::String(digest.clone());
                    Ok(())
                })
                .unwrap();
            fixture
                .source(|source| {
                    let row = source["schema"]
                        .as_array_mut()
                        .ok_or("missing schema rows")?
                        .iter_mut()
                        .find(|row| row["name"].as_str() == Some("fake_base"))
                        .ok_or("missing frozen fake")?;
                    row["sha256"] = toml::Value::String(digest);
                    Ok(())
                })
                .unwrap();
            assert_eq!(fixture.check(), Err(InventoryError::Witness));
        }

        #[test]
        fn frozen_source_counts_and_schema_membership_are_still_mandatory() {
            for (field, value) in [
                ("schemas", 260),
                ("schema_bytes", 164_517),
                ("largest_schema_bytes", 3_777),
            ] {
                let fixture = OwnedFixture::new().unwrap();
                fixture.valid().unwrap();
                fixture
                    .source(|source| {
                        source["counts"][field] = toml::Value::Integer(value);
                        Ok(())
                    })
                    .unwrap();
                assert_eq!(fixture.check(), Err(InventoryError::Counts), "{field}");
            }
            for duplicate in [false, true] {
                let fixture = OwnedFixture::new().unwrap();
                fixture.valid().unwrap();
                fixture
                    .source(|source| {
                        let rows = source["schema"]
                            .as_array_mut()
                            .ok_or("missing schema rows")?;
                        if duplicate {
                            rows.push(rows[0].clone());
                        } else {
                            rows.pop().ok_or("empty schema rows")?;
                        }
                        Ok(())
                    })
                    .unwrap();
                assert_eq!(fixture.check(), Err(InventoryError::Membership));
            }
        }

        #[test]
        fn retained_paths_cannot_escape_or_add_unapproved_depth() {
            for path in [
                "../parameters.json",
                "/parameters.json",
                "schemas/../parameters.json",
                "schemas/nested/fake.schema.json",
                "schemas\\fake.schema.json",
            ] {
                let fixture = OwnedFixture::new().unwrap();
                fixture.valid().unwrap();
                fixture
                    .manifest("fake_base", |manifest| {
                        artifact(manifest, "fake_base")?["retained"]["source_path"] =
                            toml::Value::String(path.into());
                        Ok(())
                    })
                    .unwrap();
                assert_eq!(fixture.check(), Err(InventoryError::Path), "{path}");
            }
            let fixture = OwnedFixture::new().unwrap();
            fixture.valid().unwrap();
            fs::create_dir_all(fixture.bidders.join("fake_base/schemas/nested")).unwrap();
            assert_eq!(fixture.check(), Err(InventoryError::Membership));
        }

        #[test]
        fn malformed_or_missing_owned_provenance_is_not_optional() {
            for mutation in ["missing", "malformed", "retained", "key"] {
                let fixture = OwnedFixture::new().unwrap();
                fixture.valid().unwrap();
                let path = fixture.bidders.join("fake_base/bidder.toml");
                match mutation {
                    "missing" => fs::remove_file(path).unwrap(),
                    "malformed" => {
                        let mut bytes = fs::read(&path).unwrap();
                        bytes.push(0);
                        fs::write(path, bytes).unwrap();
                    }
                    _ => fixture
                        .manifest("fake_base", |manifest| {
                            if mutation == "key" {
                                manifest["key"] = toml::Value::String("fake_secondary".into());
                            } else {
                                artifact(manifest, "fake_base")?
                                    .as_table_mut()
                                    .ok_or("missing artifact table")?
                                    .remove("retained");
                            }
                            Ok(())
                        })
                        .unwrap(),
                }
                assert!(fixture.check().is_err(), "{mutation}");
            }
            let fixture = OwnedFixture::new().unwrap();
            fixture.valid().unwrap();
            fs::remove_file(&fixture.source).unwrap();
            assert_eq!(fixture.check(), Err(InventoryError::Input));
        }

        #[test]
        fn owned_schema_manifest_and_source_keep_existing_byte_limits() {
            for (relative, bound) in [
                ("bidders/fake_base/parameters.json", MAX_SCHEMA_BYTES),
                ("bidders/fake_base/bidder.toml", MAX_DOCUMENT_BYTES),
                ("source-inventory.toml", MAX_DOCUMENT_BYTES),
            ] {
                let fixture = OwnedFixture::new().unwrap();
                fixture.valid().unwrap();
                fs::OpenOptions::new()
                    .write(true)
                    .open(fixture.directory.path().join(relative))
                    .unwrap()
                    .set_len(bound.checked_add(1).unwrap())
                    .unwrap();
                assert_eq!(fixture.check(), Err(InventoryError::Limit), "{relative}");
            }
        }

        #[cfg(unix)]
        #[test]
        fn owned_schema_manifest_source_and_all_directory_symlinks_are_rejected() {
            use std::os::unix::fs::symlink;

            for relative in [
                "bidders",
                "bidders/fake_base",
                "bidders/fake_base/schemas",
                "bidders/fake_base/parameters.json",
                "bidders/fake_base/schemas/fake_alias.schema.json",
                "bidders/fake_base/bidder.toml",
                "source-inventory.toml",
            ] {
                let fixture = OwnedFixture::new().unwrap();
                fixture.valid().unwrap();
                let path = fixture.directory.path().join(relative);
                let moved = fixture.directory.path().join("original");
                fs::rename(&path, &moved).unwrap();
                symlink(moved, path).unwrap();
                assert_eq!(fixture.check(), Err(InventoryError::Path), "{relative}");
            }
        }

        #[test]
        fn owned_inventory_order_does_not_follow_toml_artifact_array_order() {
            let fixture = OwnedFixture::new().unwrap();
            let original = fixture.valid().unwrap();
            fixture
                .manifest("fake_base", |manifest| {
                    manifest["provenance"]["artifacts"]
                        .as_array_mut()
                        .ok_or("missing artifacts")?
                        .reverse();
                    Ok(())
                })
                .unwrap();
            fixture
                .source(|source| {
                    source["schema"]
                        .as_array_mut()
                        .ok_or("missing schema rows")?
                        .reverse();
                    Ok(())
                })
                .unwrap();
            assert_eq!(fixture.valid().unwrap(), original);
        }
    }
}
