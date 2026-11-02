//! Fixed-input, non-serving build adapter. This does not qualify or publish bidders.

use crate::bidder_discovery::Discovery;
use crate::build_catalog::{self, Artifact, Catalog, Category, Manifest, Provenance, Source};
use crate::schema_staging::{
    self, ArtifactIdentity, BuildContext, Declaration, Input, InputKind, Schema, Selection, digest,
};
use crate::schema_staging_codegen::{self, Draft, HelperAvailability, SchemaDeclaration};
use crate::types::BidderMetadataDeclaration;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{ErrorKind, Read as _, Write as _};
use std::path::{Component, Path, PathBuf};
use std::str::from_utf8;

const MAX_INPUT_BYTES: u64 = 1 << 20;
// These are planning DATA, not executable test modules or serving configuration.
const PLANNING: [(InputKind, &str, &str); 5] = [
    (
        InputKind::SourceInventory,
        "tests/conformance/fixtures/reference/catalog/source-inventory.toml",
        "b22f74407fd081cd7d073b8724ba8f74e1385163b9bc30c2277342adc38bb6c6",
    ),
    (
        InputKind::SourceBindings,
        "tests/conformance/fixtures/reference/catalog/source-bindings.toml",
        "d1a0f18d99f53cbd4f62fe74c5d1e5580f8688fce95a29b1b05ee934dbe9ff0c",
    ),
    (
        InputKind::PriorityWave,
        "tests/conformance/fixtures/waves/priority.toml",
        "aa9d38e3e6221c13d13b28ba18684e041f48e3c15608aa60ee11db86924a0c24",
    ),
    (
        InputKind::SupplementalWave,
        "tests/conformance/fixtures/waves/priority-supplemental.toml",
        "ee94f5753f6244fa2980670a3fdf65ea69df2a157532a8776d061d9b2e1f66aa",
    ),
    (
        InputKind::SupplementalInventory,
        "tests/conformance/fixtures/reference/catalog/supplemental-schemas.toml",
        "2022d18b8e5580824111a2a3781039b701c7c6df85a0456206ce2b370775ebbb",
    ),
];

#[derive(Clone, Copy)]
pub struct BuildRequest<'a> {
    pub crate_dir: &'a Path,
    pub out_dir: &'a Path,
    pub discovery: &'a Discovery,
    pub metadata: &'a [BidderMetadataDeclaration],
    pub context: BuildContext<'a>,
    /// Cargo's actual build variables, including `OPT_LEVEL`, `DEBUG` and target cfgs.
    pub build_environment: &'a BTreeMap<String, String>,
}

pub struct GeneratedStaging {
    pub rust: TokenStream,
    pub manifest_json: String,
    pub identity: ArtifactIdentity,
    pub rerun_paths: BTreeSet<PathBuf>,
    pub snapshots: Vec<SchemaSnapshot>,
}

/// Captured schema bytes, never a request to reread the original source file.
#[derive(Clone, Debug, Serialize)]
pub struct SchemaSnapshot {
    pub owner: String,
    pub relative_path: PathBuf,
    pub sha256: String,
    #[serde(skip)]
    pub bytes: Vec<u8>,
}

/// Plan one deterministic output-local snapshot of already verified bytes.
///
/// # Errors
/// Refuses an empty owner identity.
pub fn plan_snapshot(owner: &str, index: usize, bytes: Vec<u8>) -> Result<SchemaSnapshot, String> {
    if owner.trim().is_empty() {
        return Err("empty schema snapshot owner".into());
    }
    let sha256 = digest(&bytes);
    Ok(SchemaSnapshot {
        owner: owner.to_owned(),
        relative_path: format!("schema-staging-input-{index}-{sha256}.json").into(),
        sha256,
        bytes,
    })
}

/// Write verified snapshot bytes only to regular direct children of `OUT_DIR`.
/// Validate the whole plan first. Existing identical files are left untouched;
/// conflicting files and symlinks refuse without following or overwriting them.
///
/// # Errors
/// Refuses invalid output paths, stale bytes, duplicate paths, conflicting files,
/// symlinks, and I/O errors. Never reads the original schema source path.
pub fn write_snapshots(out_dir: &Path, snapshots: &[SchemaSnapshot]) -> Result<(), String> {
    if !regular(out_dir)?.is_dir() {
        return Err("schema snapshot OUT_DIR is not a directory".into());
    }
    let mut paths = BTreeSet::new();
    for snapshot in snapshots {
        let mut components = snapshot.relative_path.components();
        let Some(Component::Normal(name)) = components.next() else {
            return Err("schema snapshot must be a direct child of OUT_DIR".into());
        };
        if components.next().is_some() || snapshot.relative_path.as_os_str() != name {
            return Err("non-normal schema snapshot filename".into());
        }
        if !paths.insert(&snapshot.relative_path) {
            return Err("duplicate schema snapshot filename".into());
        }
        if snapshot.owner.trim().is_empty()
            || u64::try_from(snapshot.bytes.len()).map_err(|error| error.to_string())?
                > MAX_INPUT_BYTES
            || digest(&snapshot.bytes) != snapshot.sha256
        {
            return Err("invalid schema snapshot bytes or identity".into());
        }
        matching_snapshot(&out_dir.join(&snapshot.relative_path), &snapshot.bytes)?;
    }
    for snapshot in snapshots {
        let path = out_dir.join(&snapshot.relative_path);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => file
                .write_all(&snapshot.bytes)
                .map_err(|error| error.to_string())?,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                if !matching_snapshot(&path, &snapshot.bytes)? {
                    return Err("schema snapshot destination changed during write".into());
                }
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

fn matching_snapshot(path: &Path, bytes: &[u8]) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err("nonregular schema snapshot destination".into());
            }
            if read(path)? != bytes {
                return Err("conflicting existing schema snapshot bytes".into());
            }
            Ok(true)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}

struct OwnedInput {
    kind: InputKind,
    id: String,
    bytes: Vec<u8>,
    sha256: String,
}

#[derive(Deserialize)]
struct SourceProjection {
    schema_version: u32,
    kind: String,
    revision: String,
    identity: Vec<SourceIdentity>,
    implementations: Vec<Implementation>,
    schema: Vec<SourceSchema>,
}
#[derive(Deserialize)]
struct SourceIdentity {
    name: String,
    alias_of: Option<String>,
    blob: String,
}
#[derive(Deserialize)]
struct Implementation {
    source_directory: String,
    internal_key: String,
}
#[derive(Deserialize)]
struct SourceSchema {
    name: String,
    path: String,
    blob: String,
    bytes: u64,
    sha256: String,
}
#[derive(Deserialize)]
struct BindingProjection {
    schema_version: u32,
    revision: String,
    bindings: BTreeMap<String, String>,
}
#[derive(Deserialize)]
struct Wave {
    schema_version: u32,
    revision: String,
    bidders: Vec<Planned>,
}
#[derive(Deserialize)]
struct Planned {
    name: String,
    base: String,
    implementation: String,
    metadata_blob: String,
    schema_blob: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SupplementalWave {
    schema_version: u32,
    kind: String,
    witness_inventory: String,
    bidders: Vec<SupplementalRoot>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SupplementalRoot {
    name: String,
    implementation: String,
    schema_witness: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SupplementalInventory {
    schema_version: u32,
    kind: String,
    require_all: bool,
    schema: Vec<SupplementalSchema>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SupplementalSchema {
    key: String,
    name: String,
    repository: String,
    revision: String,
    source_path: String,
    source_blob: String,
    source_bytes: u64,
    source_sha256: String,
    retained: Retained,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Retained {
    #[serde(rename = "source_path")]
    path: String,
    #[serde(rename = "source_bytes")]
    bytes: u64,
    #[serde(rename = "source_sha256")]
    sha256: String,
}

struct Definition {
    name: String,
    parent: Option<String>,
    implementation: String,
    feature: Option<String>,
    owner: Option<String>,
    local_free: bool,
}
#[derive(Serialize)]
struct Witness {
    name: String,
    path: String,
    source_path: String,
    source_blob: String,
    bytes: u64,
    sha256: String,
    repository: String,
    revision: String,
}
struct Projection {
    definitions: Vec<Definition>,
    schemas: BTreeMap<String, Witness>,
    published: BTreeSet<String>,
    planned: BTreeSet<String>,
}

fn pinned<T: DeserializeOwned>(bytes: &[u8], expected: &str) -> Result<T, String> {
    if digest(bytes) != expected {
        return Err("staging planning digest mismatch".into());
    }
    toml::from_str(from_utf8(bytes).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

fn validate_publications(
    catalog: &Catalog,
    metadata: &[BidderMetadataDeclaration],
) -> Result<(), String> {
    let published: BTreeSet<_> = metadata
        .iter()
        .map(|row| (row.bidder, row.implementation, row.required_feature))
        .collect();
    let expected: BTreeSet<_> = catalog
        .manifests
        .values()
        .flat_map(|manifest| {
            manifest.definitions.keys().map(|name| {
                (
                    name.as_str(),
                    manifest.key.as_str(),
                    manifest.required_feature.as_deref(),
                )
            })
        })
        .collect();
    if expected != published || published.len() != metadata.len() {
        return Err("staging publication declarations mismatch".into());
    }
    Ok(())
}

fn project(
    catalog: &Catalog,
    metadata: &[BidderMetadataDeclaration],
    source: &SourceProjection,
    bindings: &BindingProjection,
    wave: &Wave,
    supplemental: &SupplementalWave,
    extra: &SupplementalInventory,
) -> Result<Projection, String> {
    if source.schema_version != 2
        || source.kind != "reference_inventory_only"
        || bindings.schema_version != 1
        || wave.schema_version != 1
        || source.revision != bindings.revision
        || source.revision != wave.revision
        || supplemental.schema_version != 1
        || supplemental.kind != "priority_supplemental_schema_membership"
        || supplemental.witness_inventory != "reference/catalog/supplemental-schemas.toml"
        || extra.schema_version != 1
        || extra.kind != "supplemental_schema_reference_only"
        || !extra.require_all
    {
        return Err("staging planning contract mismatch".into());
    }
    validate_publications(catalog, metadata)?;
    let source_schemas: BTreeMap<_, _> = source
        .schema
        .iter()
        .map(|row| (row.name.as_str(), row))
        .collect();
    let identities: BTreeMap<_, _> = source
        .identity
        .iter()
        .map(|row| (row.name.as_str(), row))
        .collect();
    let implementations: BTreeMap<_, _> = source
        .implementations
        .iter()
        .map(|row| (row.source_directory.as_str(), row.internal_key.as_str()))
        .collect();
    if source_schemas.len() != source.schema.len()
        || identities.len() != source.identity.len()
        || implementations.len() != source.implementations.len()
    {
        return Err("duplicate staging source identity".into());
    }
    let schemas = project_schemas(catalog, source, &source_schemas, extra)?;
    let mut definitions = source_definitions(
        catalog,
        source,
        bindings,
        &schemas,
        &identities,
        &implementations,
    )?;
    extend_roots(&mut definitions, catalog, metadata, supplemental, &schemas)?;
    validate_planned(&definitions, &schemas, &identities, wave, supplemental)?;
    Ok(Projection {
        definitions,
        schemas,
        published: metadata.iter().map(|row| row.bidder.to_owned()).collect(),
        planned: wave
            .bidders
            .iter()
            .map(|row| row.name.clone())
            .chain(supplemental.bidders.iter().map(|row| row.name.clone()))
            .collect(),
    })
}

fn project_schemas(
    catalog: &Catalog,
    source: &SourceProjection,
    source_schemas: &BTreeMap<&str, &SourceSchema>,
    extra: &SupplementalInventory,
) -> Result<BTreeMap<String, Witness>, String> {
    let mut schemas = BTreeMap::new();
    for manifest in catalog.manifests.values() {
        if manifest
            .required_feature
            .as_deref()
            .is_some_and(|feature| feature != "insecure-bidders")
        {
            return Err(format!("unsupported staging lane: {}", manifest.key));
        }
        let Provenance::Reference {
            sources, artifacts, ..
        } = &manifest.provenance
        else {
            continue;
        };
        for artifact in artifacts
            .iter()
            .filter(|row| row.category == Category::Schema)
        {
            let witness = schema_witness(
                manifest,
                artifact,
                sources,
                &source.revision,
                source_schemas,
                extra,
            )?;
            let name = witness.name.clone();
            if schemas.insert(name.clone(), witness).is_some() {
                return Err(format!("ambiguous schema witness: {name}"));
            }
        }
    }
    if source_schemas
        .keys()
        .any(|name| !schemas.contains_key(*name))
        || extra
            .schema
            .iter()
            .any(|row| !schemas.contains_key(&row.name))
    {
        return Err("missing staging inventory schema witness".into());
    }
    Ok(schemas)
}

fn schema_witness(
    manifest: &Manifest,
    artifact: &Artifact,
    sources: &BTreeMap<String, Source>,
    revision: &str,
    source_schemas: &BTreeMap<&str, &SourceSchema>,
    extra: &SupplementalInventory,
) -> Result<Witness, String> {
    let name = Path::new(&artifact.source_path)
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or("invalid schema identity")?;
    if artifact.source_path != format!("static/bidder-params/{name}.json") {
        return Err("unsupported schema source mapping".into());
    }
    let retained = artifact
        .retained
        .as_ref()
        .ok_or("missing retained schema")?;
    let origin = sources
        .get(&artifact.source)
        .ok_or("missing schema origin")?;
    if let Some(pin) = source_schemas.get(name) {
        if pin.path != artifact.source_path
            || pin.blob != artifact.source_blob
            || pin.bytes != artifact.source_bytes
            || pin.sha256 != artifact.source_sha256
            || origin.revision != revision
        {
            return Err(format!("stale staging source witness: {name}"));
        }
    } else {
        let pin = extra
            .schema
            .iter()
            .find(|pin| pin.name == name && pin.key == manifest.key)
            .ok_or("unlisted supplemental schema")?;
        if pin.repository != origin.repository
            || pin.revision != origin.revision
            || pin.source_path != artifact.source_path
            || pin.source_blob != artifact.source_blob
            || pin.source_bytes != artifact.source_bytes
            || pin.source_sha256 != artifact.source_sha256
            || pin.retained.path != retained.path
            || pin.retained.bytes != retained.bytes
            || pin.retained.sha256 != retained.sha256
        {
            return Err(format!("stale staging supplemental witness: {name}"));
        }
    }
    Ok(Witness {
        name: name.to_owned(),
        path: format!("{}/{}", manifest.key, retained.path),
        source_path: artifact.source_path.clone(),
        source_blob: artifact.source_blob.clone(),
        bytes: retained.bytes,
        sha256: retained.sha256.clone(),
        repository: origin.repository.clone(),
        revision: origin.revision.clone(),
    })
}

fn source_definitions(
    catalog: &Catalog,
    source: &SourceProjection,
    bindings: &BindingProjection,
    schemas: &BTreeMap<String, Witness>,
    identities: &BTreeMap<&str, &SourceIdentity>,
    implementations: &BTreeMap<&str, &str>,
) -> Result<Vec<Definition>, String> {
    let mut definitions = Vec::new();
    for row in &source.identity {
        let mut current = row;
        let mut visited = BTreeSet::new();
        let implementation = loop {
            if !visited.insert(&current.name) {
                return Err("source alias cycle".into());
            }
            if let Some(directory) = bindings.bindings.get(&current.name) {
                break *implementations
                    .get(directory.as_str())
                    .ok_or("missing implementation mapping")?;
            }
            current = identities
                .get(
                    current
                        .alias_of
                        .as_deref()
                        .ok_or("missing source binding")?,
                )
                .ok_or("missing source parent")?;
        };
        let manifest = catalog
            .manifests
            .get(implementation)
            .ok_or("missing typed manifest")?;
        definitions.push(Definition {
            name: row.name.clone(),
            parent: row.alias_of.clone(),
            implementation: implementation.to_owned(),
            feature: manifest.required_feature.clone(),
            owner: schemas.contains_key(&row.name).then(|| row.name.clone()),
            local_free: false,
        });
    }
    Ok(definitions)
}

fn extend_roots(
    definitions: &mut Vec<Definition>,
    catalog: &Catalog,
    metadata: &[BidderMetadataDeclaration],
    supplemental: &SupplementalWave,
    schemas: &BTreeMap<String, Witness>,
) -> Result<(), String> {
    for (name, implementation) in metadata
        .iter()
        .map(|row| (row.bidder, row.implementation))
        .chain(
            supplemental
                .bidders
                .iter()
                .map(|row| (row.name.as_str(), row.implementation.as_str())),
        )
    {
        if let Some(row) = definitions.iter().find(|row| row.name == name) {
            if row.implementation != implementation {
                return Err("publication/source implementation mismatch".into());
            }
            continue;
        }
        let manifest = catalog
            .manifests
            .get(implementation)
            .ok_or("missing supplemental/local manifest")?;
        let local_free = matches!(manifest.provenance, Provenance::Local {});
        if !local_free && !schemas.contains_key(name) {
            return Err(format!("missing published/planned schema: {name}"));
        }
        definitions.push(Definition {
            name: name.to_owned(),
            parent: None,
            implementation: implementation.to_owned(),
            feature: manifest.required_feature.clone(),
            owner: schemas.contains_key(name).then(|| name.to_owned()),
            local_free,
        });
    }
    Ok(())
}

fn validate_planned(
    definitions: &[Definition],
    schemas: &BTreeMap<String, Witness>,
    identities: &BTreeMap<&str, &SourceIdentity>,
    wave: &Wave,
    supplemental: &SupplementalWave,
) -> Result<(), String> {
    for row in &wave.bidders {
        let identity = identities
            .get(row.name.as_str())
            .ok_or("unknown planned root")?;
        let definition = definitions
            .iter()
            .find(|definition| definition.name == row.name)
            .ok_or("missing planned binding")?;
        let schema = schemas.get(&row.base).ok_or("missing planned schema")?;
        if identity.blob != row.metadata_blob
            || schema.source_blob != row.schema_blob
            || definition.implementation != row.implementation
        {
            return Err("planned source witness mismatch".into());
        }
    }
    for row in &supplemental.bidders {
        let schema = schemas
            .get(&row.schema_witness)
            .ok_or("missing supplemental witness")?;
        if schema.name != row.name
            || Path::new(&schema.path)
                .components()
                .next()
                .and_then(|part| part.as_os_str().to_str())
                != Some(row.implementation.as_str())
        {
            return Err("supplemental source witness mismatch".into());
        }
    }
    Ok(())
}

#[derive(Debug)]
struct Audit {
    draft: Draft,
    formats: BTreeSet<String>,
}

// Audit only schema positions. Property names, enum/const values and annotations
// are data, so a field named `format` or `$ref` cannot invent a helper/reference.
fn audit(value: &Value) -> Result<Audit, String> {
    let draft = dialect(value.get("$schema").ok_or("missing staging schema draft")?)?;
    let mut formats = BTreeSet::new();
    audit_node(value, draft, 0, &mut formats)?;
    Ok(Audit { draft, formats })
}

fn dialect(value: &Value) -> Result<Draft, String> {
    Ok(match value.as_str() {
        Some(
            "http://json-schema.org/draft-04/schema#" | "https://json-schema.org/draft-04/schema#",
        ) => Draft::Draft4,
        Some(
            "https://json-schema.org/draft/2020-12/schema"
            | "https://json-schema.org/draft/2020-12/schema#",
        ) => Draft::Draft202012,
        _ => return Err("unqualified staging schema draft".into()),
    })
}

fn audit_node(
    node: &Value,
    draft: Draft,
    depth: usize,
    formats: &mut BTreeSet<String>,
) -> Result<(), String> {
    if depth > 64 {
        return Err("staging schema depth limit".into());
    }
    if draft == Draft::Draft202012 && node.is_boolean() {
        return Ok(());
    }
    let object = node.as_object().ok_or("invalid staging schema position")?;
    let modern = draft == Draft::Draft202012;
    for (key, value) in object {
        if key == "$schema" {
            if dialect(value)? != draft {
                return Err("nested staging dialect change".into());
            }
            continue;
        }
        let mut children = Vec::new();
        match key.as_str() {
            "$ref" | "$dynamicRef" | "$recursiveRef" => {
                return Err("unqualified staging reference".into());
            }
            "title" | "description" | "default" | "type" | "enum" | "multipleOf" | "maximum"
            | "exclusiveMaximum" | "minimum" | "exclusiveMinimum" | "maxLength" | "minLength"
            | "pattern" | "maxItems" | "minItems" | "uniqueItems" | "maxProperties"
            | "minProperties" | "required" => {}
            "id" if !modern => {}
            "$id" | "$anchor" | "$dynamicAnchor" | "const" | "dependentRequired"
            | "minContains" | "maxContains" | "examples" | "$comment" | "deprecated"
            | "readOnly" | "writeOnly"
                if modern => {}
            "format" => {
                let format = value.as_str().ok_or("invalid staging format")?;
                if !matches!(format, "uuid" | "hostname") {
                    return Err(format!("unqualified staging format: {format}"));
                }
                formats.insert(format.to_owned());
            }
            "properties" | "patternProperties" => {
                children.extend(value.as_object().ok_or("invalid schema map")?.values());
            }
            "definitions" if !modern => {
                children.extend(value.as_object().ok_or("invalid definitions map")?.values());
            }
            "$defs" | "dependentSchemas" if modern => {
                children.extend(value.as_object().ok_or("invalid schema map")?.values());
            }
            "dependencies" if !modern => children.extend(
                value
                    .as_object()
                    .ok_or("invalid dependencies")?
                    .values()
                    .filter(|child| !child.is_array()),
            ),
            "allOf" | "anyOf" | "oneOf" => {
                children.extend(value.as_array().ok_or("invalid schema array")?);
            }
            "prefixItems" if modern => {
                children.extend(value.as_array().ok_or("invalid schema array")?);
            }
            "items" if !modern && value.is_array() => {
                children.extend(value.as_array().ok_or("invalid schema tuple")?);
            }
            "additionalProperties" | "additionalItems" if !modern && value.is_boolean() => {}
            "items" | "not" | "additionalProperties" => children.push(value),
            "additionalItems" if !modern => children.push(value),
            "if"
            | "then"
            | "else"
            | "contains"
            | "propertyNames"
            | "unevaluatedItems"
            | "unevaluatedProperties"
                if modern =>
            {
                children.push(value);
            }
            _ => return Err(format!("unqualified staging keyword: {key}")),
        }
        for child in children {
            audit_node(
                child,
                draft,
                depth.checked_add(1).ok_or("schema depth overflow")?,
                formats,
            )?;
        }
    }
    Ok(())
}

fn regular(path: &Path) -> Result<fs::Metadata, String> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
        || path.components().collect::<PathBuf>().as_os_str() != path.as_os_str()
    {
        return Err(format!("non-normal staging path: {}", path.display()));
    }
    let mut prefix = PathBuf::new();
    for part in path.components() {
        prefix.push(part);
        if fs::symlink_metadata(&prefix)
            .map_err(|error| format!("{}: {error}", prefix.display()))?
            .file_type()
            .is_symlink()
        {
            return Err(format!("symlink staging input: {}", prefix.display()));
        }
    }
    fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    if !regular(path)?.is_file() {
        return Err(format!("non-file staging input: {}", path.display()));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if u64::try_from(bytes.len()).map_err(|error| error.to_string())? > MAX_INPUT_BYTES {
        return Err("staging input byte limit".into());
    }
    Ok(bytes)
}

fn input(kind: InputKind, id: String, bytes: Vec<u8>) -> OwnedInput {
    let sha256 = digest(&bytes);
    OwnedInput {
        kind,
        id,
        bytes,
        sha256,
    }
}

fn add_file(
    inputs: &mut Vec<OwnedInput>,
    rerun: &mut BTreeSet<PathBuf>,
    workspace: &Path,
    path: &Path,
    kind: InputKind,
) -> Result<(), String> {
    let id = path
        .strip_prefix(workspace)
        .map_err(|error| error.to_string())?
        .to_str()
        .ok_or("non UTF-8 staging input ID")?
        .to_owned();
    inputs.push(input(kind, id, read(path)?));
    rerun.insert(path.to_owned());
    Ok(())
}

fn vendor_sources(
    inputs: &mut Vec<OwnedInput>,
    rerun: &mut BTreeSet<PathBuf>,
    workspace: &Path,
    root: &Path,
) -> Result<(), String> {
    if !regular(root)?.is_dir() {
        return Err("missing staging vendor source directory".into());
    }
    rerun.insert(root.to_owned());
    let children: BTreeSet<_> = fs::read_dir(root)
        .map_err(|error| error.to_string())?
        .map(|result| result.map(|entry| entry.path()))
        .collect::<Result<_, _>>()
        .map_err(|error| error.to_string())?;
    for path in children {
        let metadata = regular(&path)?;
        if metadata.is_dir() {
            vendor_sources(inputs, rerun, workspace, &path)?;
            continue;
        }
        if path.extension().is_some_and(|value| value == "rs")
            || path.file_name().is_some_and(|value| value == "Cargo.toml")
        {
            add_file(inputs, rerun, workspace, &path, InputKind::GeneratorSource)?;
        }
    }
    Ok(())
}

fn retention(owners: &[String], identity: &str) -> Result<TokenStream, String> {
    if identity.len() != 64 || !identity.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("invalid inspection identity".into());
    }
    let identity_bytes = syn::LitByteStr::new(identity.as_bytes(), proc_macro2::Span::call_site());
    let mut entries = TokenStream::new();
    let mut references = Vec::new();
    for (index, owner) in owners.iter().enumerate() {
        if owner.is_empty() || owner.len() > 64 || !owner.is_ascii() {
            return Err("invalid inspection owner".into());
        }
        let mut padded = [0_u8; 64];
        padded
            .get_mut(..owner.len())
            .ok_or("invalid inspection owner")?
            .copy_from_slice(owner.as_bytes());
        let owner_bytes = syn::LitByteStr::new(&padded, proc_macro2::Span::call_site());
        let owner_len = syn::LitInt::new(&owner.len().to_string(), proc_macro2::Span::call_site());
        let marker = format_ident!("StagedSchema{index}");
        let cache = format_ident!("StagedSchema{index}SearchCache");
        let entry = format_ident!("STAGED_VALIDATOR_{index}");
        entries.extend(quote! {
            static #entry: StagedValidator<#cache> = StagedValidator {
                magic: *b"AUCTIONEERSCHEMA", identity: *#identity_bytes,
                owner: *#owner_bytes, owner_len: #owner_len,
                validate: #marker::is_valid, new_cache: #marker::new_search_cache,
                initialize: #marker::initialize_regexes,
            };
            const _: () = {
                use std::mem::{align_of, offset_of, size_of};
                let word = size_of::<usize>();
                let (validate, new_cache, initialize, row_bytes) = match word {
                    4 => (148, 152, 156, 160),
                    8 => (152, 160, 168, 176),
                    _ => panic!("unsupported schema inspection pointer width"),
                };
                assert!(size_of::<fn(&serde_json::Value, &mut #cache) -> bool>() == word,
                    "validation function pointer width");
                assert!(size_of::<fn() -> Option<#cache>>() == word,
                    "cache function pointer width");
                assert!(size_of::<fn() -> bool>() == word,
                    "initialization function pointer width");
                assert!(align_of::<StagedValidator<#cache>>() == word,
                    "schema inspection row alignment");
                assert!(offset_of!(StagedValidator<#cache>, magic) == 0,
                    "schema inspection magic offset");
                assert!(offset_of!(StagedValidator<#cache>, identity) == 16,
                    "schema inspection identity offset");
                assert!(offset_of!(StagedValidator<#cache>, owner) == 80,
                    "schema inspection owner offset");
                assert!(offset_of!(StagedValidator<#cache>, owner_len) == 144,
                    "schema inspection owner length offset");
                assert!(offset_of!(StagedValidator<#cache>, validate) == validate,
                    "schema inspection validation pointer offset");
                assert!(offset_of!(StagedValidator<#cache>, new_cache) == new_cache,
                    "schema inspection cache pointer offset");
                assert!(offset_of!(StagedValidator<#cache>, initialize) == initialize,
                    "schema inspection initialization pointer offset");
                assert!(size_of::<StagedValidator<#cache>>() == row_bytes,
                    "schema inspection row size");
            };
        });
        references.push(quote! { &#entry });
    }
    Ok(quote! {
        use std::hint::black_box;
        #[repr(C)]
        struct StagedValidator<C> {
            magic: [u8; 16], identity: [u8; 64], owner: [u8; 64], owner_len: u32,
            validate: fn(&serde_json::Value, &mut C) -> bool,
            new_cache: fn() -> Option<C>, initialize: fn() -> bool,
        }
        #entries
        const STAGING_INPUT_ID: &str = #identity;
        pub(super) fn retain_for_inspection() {
            black_box((#(#references,)* STAGING_INPUT_ID));
        }
    })
}

fn native_dispatch(owners: &[String]) -> TokenStream {
    let mut initializers = Vec::new();
    let mut validators = Vec::new();
    for (index, owner) in owners.iter().enumerate() {
        let marker = format_ident!("StagedSchema{index}");
        initializers.push(quote! { #marker::initialize_regexes() });
        validators.push(quote! {
            #owner => {
                let mut cache = #marker::new_search_cache()?;
                Some(#marker::is_valid(value, &mut cache))
            }
        });
    }
    let initialization = if initializers.is_empty() {
        quote! { true }
    } else {
        quote! { #(#initializers)&&* }
    };
    quote! {
        #[cfg(not(target_arch = "wasm32"))]
        pub(super) fn initialize_native() -> bool {
            #initialization
        }
        #[cfg(not(target_arch = "wasm32"))]
        pub(super) fn validate_native(owner: &str, value: &serde_json::Value) -> Option<bool> {
            match owner {
                #(#validators,)*
                _ => None,
            }
        }
    }
}

/// Generate a private staged table and a sidecar describing its exact inputs.
/// The build adapter selects this through the internal Cargo feature. This function
/// never installs, initializes or executes a validator and never supplies costs.
///
/// # Errors
/// Refuses invalid discovery/planning/schema inputs, stale witnesses, unsupported
/// declarations/features/audits, and incomplete build identity.
pub fn generate(request: BuildRequest<'_>) -> Result<GeneratedStaging, String> {
    let BuildRequest {
        crate_dir,
        out_dir,
        discovery,
        metadata,
        context,
        ..
    } = request;
    if !regular(out_dir)?.is_dir() {
        return Err("schema snapshot OUT_DIR is not a directory".into());
    }
    discovery.validate_metadata(metadata)?;
    let workspace = crate_dir
        .parent()
        .and_then(Path::parent)
        .ok_or("missing staging workspace root")?;
    let root = crate_dir.join("src/bidders");
    let cargo: toml::Value = toml::from_str(
        from_utf8(&read(&crate_dir.join("Cargo.toml"))?).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let features: BTreeSet<String> = cargo
        .get("features")
        .and_then(toml::Value::as_table)
        .ok_or("missing bidder features")?
        .keys()
        .cloned()
        .collect();
    if context
        .enabled_features
        .iter()
        .any(|feature| !features.contains(*feature))
    {
        return Err("undeclared staging build feature".into());
    }
    let catalog = build_catalog::scan(&root, &features).map_err(|error| error.to_string())?;
    let mut rerun = discovery
        .rerun_paths()
        .union(&catalog.input_paths)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut inputs = Vec::new();
    for (kind, relative, expected) in PLANNING {
        let path = crate_dir.join(relative);
        let bytes = read(&path)?;
        if digest(&bytes) != expected {
            return Err(format!("staging planning digest mismatch: {relative}"));
        }
        rerun.insert(path);
        inputs.push(input(kind, relative.to_owned(), bytes));
    }
    let planning = |kind| {
        inputs
            .iter()
            .find(|input| input.kind == kind)
            .ok_or("missing staging planning data")
    };
    let source: SourceProjection =
        pinned(&planning(InputKind::SourceInventory)?.bytes, PLANNING[0].2)?;
    let bindings: BindingProjection =
        pinned(&planning(InputKind::SourceBindings)?.bytes, PLANNING[1].2)?;
    let wave: Wave = pinned(&planning(InputKind::PriorityWave)?.bytes, PLANNING[2].2)?;
    let supplemental: SupplementalWave =
        pinned(&planning(InputKind::SupplementalWave)?.bytes, PLANNING[3].2)?;
    let extra: SupplementalInventory = pinned(
        &planning(InputKind::SupplementalInventory)?.bytes,
        PLANNING[4].2,
    )?;
    let projection = project(
        &catalog,
        metadata,
        &source,
        &bindings,
        &wave,
        &supplemental,
        &extra,
    )?;
    generate_projection(
        request,
        workspace,
        &catalog,
        &projection,
        &mut inputs,
        rerun,
    )
}

fn generate_projection(
    request: BuildRequest<'_>,
    workspace: &Path,
    catalog: &Catalog,
    projection: &Projection,
    inputs: &mut Vec<OwnedInput>,
    rerun: BTreeSet<PathBuf>,
) -> Result<GeneratedStaging, String> {
    let slots: Vec<_> = projection
        .definitions
        .iter()
        .map(|row| [row.owner.as_deref().unwrap_or("")])
        .collect();
    let declarations: Vec<_> = projection
        .definitions
        .iter()
        .zip(&slots)
        .map(|(row, owner)| Declaration {
            name: &row.name,
            parent: row.parent.as_deref(),
            implementation: &row.implementation,
            required_feature: row.feature.as_deref(),
            schema: if row.local_free {
                Schema::LocalSchemaFree
            } else if row.owner.is_some() {
                Schema::Owners(owner)
            } else {
                Schema::Inherit
            },
        })
        .collect();
    let published: Vec<_> = projection.published.iter().map(String::as_str).collect();
    let planned: Vec<_> = projection.planned.iter().map(String::as_str).collect();
    let selection = schema_staging::select(
        &declarations,
        &published,
        &planned,
        request.context.enabled_features,
    )?;
    for member in &selection.members {
        let factory = catalog
            .manifests
            .get(member.implementation)
            .ok_or("missing selected factory")?;
        if !factory.factory {
            return Err(format!(
                "missing selected factory: {}",
                member.implementation
            ));
        }
    }
    emit_selection(
        request, workspace, catalog, projection, &selection, inputs, rerun,
    )
}

struct CapturedSchema<'a> {
    witness: &'a Witness,
    path: PathBuf,
    audit: Audit,
    snapshot: SchemaSnapshot,
}

fn capture_schemas<'a>(
    projection: &'a Projection,
    selection: &Selection<'_>,
    root: &Path,
    out_dir: &Path,
    inputs: &mut Vec<OwnedInput>,
    rerun: &mut BTreeSet<PathBuf>,
) -> Result<Vec<CapturedSchema<'a>>, String> {
    let mut selected = Vec::new();
    for (index, owner) in selection.schema_owners.iter().enumerate() {
        let witness = projection
            .schemas
            .get(*owner)
            .ok_or("missing selected schema witness")?;
        let path = root.join(&witness.path);
        let bytes = read(&path)?;
        if u64::try_from(bytes.len()).map_err(|error| error.to_string())? != witness.bytes
            || digest(&bytes) != witness.sha256
        {
            return Err("selected schema witness mismatch".into());
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        let snapshot = plan_snapshot(owner, index, bytes.clone())?;
        let compiled_path = out_dir.join(&snapshot.relative_path);
        // The macro also tracks this input. Revalidate its identity before any
        // incremental compilation can consume an edited generated snapshot.
        rerun.insert(compiled_path.clone());
        selected.push(CapturedSchema {
            witness,
            path: compiled_path,
            audit: audit(&value)?,
            snapshot,
        });
        inputs.push(input(InputKind::Schema, (*owner).to_owned(), bytes));
    }
    Ok(selected)
}

fn emit_selection(
    request: BuildRequest<'_>,
    workspace: &Path,
    catalog: &Catalog,
    projection: &Projection,
    selection: &Selection<'_>,
    inputs: &mut Vec<OwnedInput>,
    mut rerun: BTreeSet<PathBuf>,
) -> Result<GeneratedStaging, String> {
    let BuildRequest {
        crate_dir,
        out_dir,
        context,
        build_environment,
        ..
    } = request;
    let selected = capture_schemas(
        projection,
        selection,
        &crate_dir.join("src/bidders"),
        out_dir,
        inputs,
        &mut rerun,
    )?;
    let format_slots: Vec<Vec<_>> = selected
        .iter()
        .map(|schema| schema.audit.formats.iter().map(String::as_str).collect())
        .collect();
    let schemas: Vec<_> = selected
        .iter()
        .zip(&format_slots)
        .map(|(schema, formats)| SchemaDeclaration {
            owner: &schema.witness.name,
            path: &schema.path,
            draft: schema.audit.draft,
            formats,
            unknown_keywords: &[],
            local_references: &[],
            external_references: &[],
        })
        .collect();
    let generated = schema_staging_codegen::quote_validators(
        selection,
        &schemas,
        HelperAvailability {
            uuid: true,
            hostname: true,
        },
    )?;
    inputs.push(input(
        InputKind::GeneratorSource,
        "generated-declarations.rs".into(),
        generated.items.to_string().into_bytes(),
    ));
    inputs.push(input(
        InputKind::GeneratorSource,
        "cargo-build-environment.json".into(),
        serde_json::to_vec(build_environment).map_err(|error| error.to_string())?,
    ));
    record_source_inputs(crate_dir, workspace, catalog, inputs, &mut rerun)?;
    let borrowed: Vec<_> = inputs
        .iter()
        .map(|input| Input {
            kind: input.kind,
            id: &input.id,
            bytes: &input.bytes,
            expected_sha256: &input.sha256,
        })
        .collect();
    let identity = schema_staging::identify(selection, context, &borrowed)?;
    let mut rust = generated.items;
    rust.extend(retention(&generated.owners, &identity.sha256)?);
    rust.extend(native_dispatch(&generated.owners));
    rust.extend(native_publication_bindings(catalog, selection));
    let manifest_json = serde_json::to_string_pretty(&serde_json::json!({
        "schema_version": 1_u32, "kind": "unqualified_non_serving_schema_staging", "identity": identity,
        "linked_or_serving_qualification": false, "artifact_inspection": "pending",
        "pending_gaps": ["linked_membership", "actual_imports", "resource_bounds", "serving_integration", "final_artifact_recheck"],
        "build": context, "build_environment": build_environment, "options": schema_staging_codegen::CANDIDATE_OPTIONS,
        "published_roots_all_features": projection.published, "planned_roots": projection.planned,
        "selection": selection, "schemas": selected.iter().map(|schema| schema.witness).collect::<Vec<_>>(),
        "compiled_schema_snapshots": selected.iter().map(|schema| &schema.snapshot).collect::<Vec<_>>(),
        "required_formats": generated.required_formats, "emitted_rust_sha256": digest(rust.to_string().as_bytes()),
        "runtime_dependency_note": "jsonschema runtime retained; actual guest imports require artifact inspection"
    })).map_err(|error| error.to_string())?;
    Ok(GeneratedStaging {
        rust,
        manifest_json,
        identity,
        rerun_paths: rerun,
        snapshots: selected.into_iter().map(|schema| schema.snapshot).collect(),
    })
}

fn native_publication_bindings(catalog: &Catalog, selection: &Selection<'_>) -> TokenStream {
    let mut native_bindings: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut native_required = Vec::new();
    for member in &selection.members {
        let Some(manifest) = catalog.manifests.get(member.implementation) else {
            continue;
        };
        if !manifest.definitions.contains_key(member.name) {
            continue;
        }
        if let Some(owner) = member.schema_owner {
            native_bindings.entry(owner).or_default().push(member.name);
        }
        if manifest.native_only {
            native_required.push(member.name);
        }
    }
    let owner_argument = if native_bindings.is_empty() {
        quote! { _ }
    } else {
        quote! { name }
    };
    let owner_lookup = if native_bindings.is_empty() {
        quote! { None }
    } else {
        let arms = native_bindings.iter().map(|(owner, names)| {
            quote! { #(#names)|* => Some(#owner) }
        });
        quote! { match name { #(#arms,)* _ => None } }
    };
    let required_argument = if native_required.is_empty() {
        quote! { _ }
    } else {
        quote! { name }
    };
    let required_lookup = if native_required.is_empty() {
        quote! { false }
    } else {
        quote! { matches!(name, #(#native_required)|*) }
    };
    quote! {
        #[cfg(not(target_arch = "wasm32"))]
        pub(super) fn native_owner(#owner_argument: &str) -> Option<&'static str> {
            #owner_lookup
        }
        #[cfg(not(target_arch = "wasm32"))]
        pub(super) fn native_required(#required_argument: &str) -> bool {
            #required_lookup
        }
    }
}

fn record_source_inputs(
    crate_dir: &Path,
    workspace: &Path,
    catalog: &Catalog,
    inputs: &mut Vec<OwnedInput>,
    rerun: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    for (relative, kind) in [
        ("Cargo.toml", InputKind::WorkspaceManifest),
        ("Cargo.lock", InputKind::Lockfile),
    ] {
        add_file(inputs, rerun, workspace, &workspace.join(relative), kind)?;
    }
    for path in catalog
        .input_paths
        .iter()
        .filter(|path| path.file_name().is_some_and(|name| name == "bidder.toml"))
    {
        add_file(inputs, rerun, workspace, path, InputKind::GeneratorSource)?;
    }
    for relative in [
        "crates/auctioneer-bidder/Cargo.toml",
        "crates/auctioneer-bidder/build.rs",
        "crates/auctioneer-config-schema/Cargo.toml",
        "crates/auctioneer-config-schema/src/schema_staging.rs",
        "crates/auctioneer-config-schema/src/schema_staging_codegen.rs",
        "crates/auctioneer-config-schema/src/schema_staging_build.rs",
        "crates/auctioneer-config-schema/src/bidder_discovery.rs",
        "crates/auctioneer-config-schema/src/build_catalog.rs",
        "crates/auctioneer-config-schema/src/bidder_manifest.rs",
    ] {
        add_file(
            inputs,
            rerun,
            workspace,
            &workspace.join(relative),
            InputKind::GeneratorSource,
        )?;
    }
    add_file(
        inputs,
        rerun,
        workspace,
        &crate_dir.join("src/schema/staging.rs"),
        InputKind::FormatHelperSource,
    )?;
    for relative in ["vendor/jsonschema-macros-core", "vendor/regex-automata"] {
        vendor_sources(inputs, rerun, workspace, &workspace.join(relative))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        BindingProjection, Draft, SourceProjection, SupplementalInventory, SupplementalWave, Wave,
        audit, native_dispatch, native_publication_bindings, pinned, project, retention,
    };
    use super::{SchemaSnapshot, plan_snapshot, write_snapshots};
    use crate::build_catalog::{Catalog, Manifest};
    use crate::schema_staging::{Declaration, Member, Schema, Selection, digest, select};
    use crate::types::BidderMetadataDeclaration;
    use serde_json::json;
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs,
        path::PathBuf,
        slice::from_ref,
    };

    fn fake_snapshot() -> SchemaSnapshot {
        let bytes = serde_json::to_vec(&json!({
            "$schema": "http://json-schema.org/draft-04/schema#", "type": "string"
        }))
        .expect("valid fake schema JSON");
        SchemaSnapshot {
            owner: "fake_owner".into(),
            relative_path: "fake-snapshot.json".into(),
            sha256: digest(&bytes),
            bytes,
        }
    }

    #[test]
    fn fake_snapshot_captures_source_before_mutation_and_quotes_only_output_path() {
        use crate::schema_staging::Selection;
        use crate::schema_staging_codegen::{
            HelperAvailability, SchemaDeclaration, quote_validators,
        };
        let temporary = tempfile::tempdir().expect("fake build directory");
        let root = temporary
            .path()
            .canonicalize()
            .expect("normal absolute path");
        let out_dir = root.join("out");
        fs::create_dir_all(&out_dir).expect("OUT_DIR");
        let original = root.join("parameters.json");
        let expected = fake_snapshot().bytes;
        fs::write(&original, &expected).expect("original verified input");
        let snapshot = plan_snapshot(
            "fake_owner",
            0,
            super::read(&original).expect("capture input"),
        )
        .expect("snapshot plan");
        fs::write(&original, b"false").expect("edit original after capture");
        write_snapshots(&out_dir, from_ref(&snapshot)).expect("write captured bytes");
        let compiled_path = out_dir.join(&snapshot.relative_path);
        assert_eq!(fs::read(&compiled_path).expect("snapshot"), expected);
        assert_eq!(snapshot.sha256, digest(&expected));
        let generated = quote_validators(
            &Selection {
                members: vec![],
                schema_owners: vec!["fake_owner"],
            },
            &[SchemaDeclaration {
                owner: "fake_owner",
                path: &compiled_path,
                draft: Draft::Draft4,
                formats: &[],
                unknown_keywords: &[],
                local_references: &[],
                external_references: &[],
            }],
            HelperAvailability {
                uuid: false,
                hostname: false,
            },
        )
        .expect("snapshot-backed declaration");
        let path = compiled_path.to_str().expect("UTF-8 snapshot path");
        let expected_path = quote::quote!(path = #path).to_string();
        let source = generated.items.to_string();
        assert!(source.contains(&expected_path));
        assert!(!source.contains(original.to_str().expect("UTF-8 original path")));
    }

    #[test]
    fn fake_snapshot_writer_refuses_escapes_before_any_write() {
        let temporary = tempfile::tempdir().expect("fake build directory");
        let root = temporary
            .path()
            .canonicalize()
            .expect("normal absolute path");
        let out_dir = root.join("out");
        fs::create_dir_all(&out_dir).expect("OUT_DIR");
        let outside = root.join("outside.json");
        fs::write(&outside, b"outside canary").expect("outside file");
        let direct_child = "schema snapshot must be a direct child of OUT_DIR";
        let non_normal = "non-normal schema snapshot filename";
        for (relative, reason) in [
            (PathBuf::new(), direct_child),
            (PathBuf::from("../outside.json"), direct_child),
            (outside.clone(), direct_child),
            (PathBuf::from("nested/../../outside.json"), non_normal),
            (PathBuf::from("../new-outside.json"), direct_child),
            (PathBuf::from("nested/child.json"), non_normal),
            (PathBuf::from("./child.json"), direct_child),
        ] {
            let good = fake_snapshot();
            let mut bad = fake_snapshot();
            bad.relative_path = relative;
            assert_eq!(
                write_snapshots(&out_dir, &[good.clone(), bad]).expect_err("unsafe output"),
                reason
            );
            assert!(
                !out_dir.join(&good.relative_path).exists(),
                "preflight before any write"
            );
            assert_eq!(
                fs::read(&outside).expect("outside untouched"),
                b"outside canary"
            );
            assert!(!root.join("new-outside.json").exists());
        }
    }

    #[cfg(unix)]
    #[test]
    fn fake_snapshot_writer_refuses_symlinks_and_conflicting_hardlinks() {
        use std::os::unix::fs::symlink;
        let temporary = tempfile::tempdir().expect("fake build directory");
        let root = temporary
            .path()
            .canonicalize()
            .expect("normal absolute path");
        let out_dir = root.join("out");
        let outside_dir = root.join("outside");
        fs::create_dir_all(&out_dir).expect("OUT_DIR");
        fs::create_dir_all(&outside_dir).expect("outside directory");
        let canary = outside_dir.join("canary.json");
        fs::write(&canary, b"outside canary").expect("outside file");
        let snapshot = fake_snapshot();
        let alias = root.join("out-alias");
        symlink(&outside_dir, &alias).expect("symlink output directory");
        assert_eq!(
            write_snapshots(&alias, from_ref(&snapshot)).expect_err("symlink OUT_DIR"),
            format!("symlink staging input: {}", alias.display())
        );
        assert!(!outside_dir.join(&snapshot.relative_path).exists());
        let destination = out_dir.join(&snapshot.relative_path);
        symlink(&canary, &destination).expect("symlink output file");
        assert_eq!(
            write_snapshots(&out_dir, from_ref(&snapshot)).expect_err("symlink output"),
            "nonregular schema snapshot destination"
        );
        assert_eq!(
            fs::read(&canary).expect("outside untouched"),
            b"outside canary"
        );
        fs::remove_file(&destination).expect("remove test symlink");
        fs::hard_link(&canary, &destination).expect("hardlinked output file");
        assert_eq!(
            write_snapshots(&out_dir, from_ref(&snapshot)).expect_err("conflicting hardlink"),
            "conflicting existing schema snapshot bytes"
        );
        assert_eq!(
            fs::read(&canary).expect("outside untouched"),
            b"outside canary"
        );
    }

    #[test]
    fn fake_snapshot_writer_is_idempotent_and_refuses_mutated_bytes_and_conflicts() {
        let temporary = tempfile::tempdir().expect("fake build directory");
        let root = temporary
            .path()
            .canonicalize()
            .expect("normal absolute path");
        let out_dir = root.join("out");
        fs::create_dir_all(&out_dir).expect("OUT_DIR");
        let snapshot = fake_snapshot();
        let mut changed = snapshot.clone();
        changed.bytes = b"false".to_vec();
        assert_eq!(
            write_snapshots(&out_dir, &[changed]).expect_err("mutated snapshot bytes"),
            "invalid schema snapshot bytes or identity"
        );
        assert_eq!(
            fs::read_dir(&out_dir).expect("OUT_DIR unchanged").count(),
            0
        );
        write_snapshots(&out_dir, from_ref(&snapshot)).expect("first write");
        write_snapshots(&out_dir, from_ref(&snapshot)).expect("identical output");
        let path = out_dir.join(&snapshot.relative_path);
        fs::write(&path, b"conflicting existing bytes").expect("mutate generated copy");
        assert_eq!(
            write_snapshots(&out_dir, &[snapshot]).expect_err("conflicting output"),
            "conflicting existing schema snapshot bytes"
        );
        assert_eq!(
            fs::read(&path).expect("conflict left untouched"),
            b"conflicting existing bytes"
        );
    }
    #[test]
    fn fake_audit_only_visits_schema_positions_and_deduplicates_formats() {
        let value = json!({"$schema": "http://json-schema.org/draft-04/schema#", "type": "object",
            "description": {"fake_unknown": true}, "enum": [{"$ref": "fake"}],
            "properties": {"format": {"type": "string", "format": "uuid"}, "$ref": {"format": "uuid"}},
            "additionalProperties": false});
        let audit = audit(&value).expect("schema-position audit");
        assert_eq!(audit.draft, Draft::Draft4);
        assert_eq!(audit.formats.into_iter().collect::<Vec<_>>(), ["uuid"]);
    }

    #[test]
    fn fake_audit_refuses_unknown_keywords_formats_references_and_nested_dialects() {
        let cases = [
            json!({"fake_keyword": true}),
            json!({"format": "fake_format"}),
            json!({"$ref": "#/definitions/fake"}),
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema"}),
        ];
        for (child, reason) in cases.into_iter().zip([
            "unqualified staging keyword: fake_keyword",
            "unqualified staging format: fake_format",
            "unqualified staging reference",
            "nested staging dialect change",
        ]) {
            let value = json!({"$schema": "http://json-schema.org/draft-04/schema#", "properties": {"fake": child}});
            assert_eq!(
                audit(&value).expect_err("unsupported schema"),
                reason,
                "{value}"
            );
        }
        assert_eq!(
            audit(&json!({})).expect_err("unspecified draft"),
            "missing staging schema draft"
        );
    }

    #[test]
    fn fake_pin_refuses_stale_or_malformed_planning_data() {
        let bytes = b"fake = true\n";
        let hash = digest(bytes);
        let document = pinned::<toml::Value>(bytes, &hash).expect("valid planning data");
        assert_eq!(
            document.get("fake").and_then(toml::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            pinned::<toml::Value>(b"fake = false\n", &hash).expect_err("stale planning data"),
            "staging planning digest mismatch"
        );
        let invalid = b"not toml = [";
        pinned::<toml::Value>(invalid, &digest(invalid)).expect_err("malformed TOML must refuse");
    }

    fn fake_catalog(revision: &str, blob: &str, hash: &str) -> Catalog {
        let manifest: Manifest = serde_json::from_value(json!({
            "key": "fake_factory", "factory": true, "definitions": {"fake_pub": {}},
            "provenance": {"origin": "reference", "sources": {"fake": {"repository": "https://fake.invalid/repo", "revision": revision}},
                "artifacts": [{"category": "schema", "source": "fake", "source_path": "static/bidder-params/fake_base.json",
                    "source_blob": blob, "source_bytes": 2_u64, "source_sha256": hash,
                    "retained": {"source_path": "parameters.json", "source_bytes": 2_u64, "source_sha256": hash}}]}
        })).expect("fake discovered manifest");
        Catalog {
            manifests: BTreeMap::from([("fake_factory".into(), manifest)]),
            input_paths: BTreeSet::new(),
        }
    }

    #[test]
    fn fake_projection_uses_discovered_publication_and_wave_union_not_factory_roster() {
        let revision = "a".repeat(40);
        let blob = "b".repeat(40);
        let hash = "c".repeat(64);
        let catalog = fake_catalog(&revision, &blob, &hash);
        let source: SourceProjection = serde_json::from_value(json!({
            "schema_version": 2_u32, "kind": "reference_inventory_only", "revision": revision,
            "identity": [{"name": "fake_base", "blob": blob}, {"name": "fake_pub", "alias_of": "fake_base", "blob": blob},
                {"name": "fake_plan", "alias_of": "fake_base", "blob": blob}, {"name": "fake_unselected", "alias_of": "fake_base", "blob": blob}],
            "implementations": [{"source_directory": "adapters/fake", "internal_key": "fake_factory"}],
            "schema": [{"name": "fake_base", "path": "static/bidder-params/fake_base.json", "blob": blob, "bytes": 2_u64, "sha256": hash}]
        })).expect("fake source data");
        let bindings = BindingProjection {
            schema_version: 1,
            revision: revision.clone(),
            bindings: BTreeMap::from([("fake_base".into(), "adapters/fake".into())]),
        };
        let mut wave: Wave = serde_json::from_value(json!({"schema_version": 1_u32, "revision": revision,
            "bidders": [{"name": "fake_plan", "base": "fake_base", "implementation": "fake_factory", "metadata_blob": blob, "schema_blob": blob}]})).expect("fake wave");
        let supplemental = SupplementalWave {
            schema_version: 1,
            kind: "priority_supplemental_schema_membership".into(),
            witness_inventory: "reference/catalog/supplemental-schemas.toml".into(),
            bidders: vec![],
        };
        let extra = SupplementalInventory {
            schema_version: 1,
            kind: "supplemental_schema_reference_only".into(),
            require_all: true,
            schema: vec![],
        };
        let metadata = [BidderMetadataDeclaration {
            bidder: "fake_pub",
            implementation: "fake_factory",
            required_feature: None,
        }];
        let projection = project(
            &catalog,
            &metadata,
            &source,
            &bindings,
            &wave,
            &supplemental,
            &extra,
        )
        .expect("source projection");
        let slots: Vec<_> = projection
            .definitions
            .iter()
            .map(|row| [row.owner.as_deref().unwrap_or("")])
            .collect();
        let definitions: Vec<_> = projection
            .definitions
            .iter()
            .zip(&slots)
            .map(|(row, owner)| Declaration {
                name: &row.name,
                parent: row.parent.as_deref(),
                implementation: &row.implementation,
                required_feature: row.feature.as_deref(),
                schema: if row.owner.is_some() {
                    Schema::Owners(owner)
                } else {
                    Schema::Inherit
                },
            })
            .collect();
        let published: Vec<_> = projection.published.iter().map(String::as_str).collect();
        let planned: Vec<_> = projection.planned.iter().map(String::as_str).collect();
        let selected =
            select(&definitions, &published, &planned, &[]).expect("same shared selector");
        assert_eq!(selected.schema_owners, ["fake_base"]);
        assert_eq!(
            selected
                .members
                .iter()
                .map(|row| row.name)
                .collect::<Vec<_>>(),
            ["fake_base", "fake_plan", "fake_pub"]
        );
        wave.bidders[0].schema_blob = "fake_stale_blob".into();
        assert_eq!(
            project(
                &catalog,
                &metadata,
                &source,
                &bindings,
                &wave,
                &supplemental,
                &extra
            )
            .err()
            .as_deref(),
            Some("planned source witness mismatch")
        );
    }

    #[test]
    fn fake_retention_root_has_only_typed_pointers_and_no_target_calls() {
        let generated = retention(&["fake_owner".into()], &"ab".repeat(32)).unwrap();
        let file = syn::parse2::<syn::File>(generated).expect("quoted Rust");
        let table = file
            .items
            .iter()
            .find_map(|item| {
                let syn::Item::Static(table) = item else {
                    return None;
                };
                Some(table)
            })
            .expect("private table");
        assert!(matches!(table.vis, syn::Visibility::Inherited));
        let root = file
            .items
            .iter()
            .find_map(|item| {
                let syn::Item::Fn(function) = item else {
                    return None;
                };
                (function.sig.ident == "retain_for_inspection").then_some(function)
            })
            .expect("inspection root");
        assert_eq!(root.block.stmts.len(), 1);
        let syn::Stmt::Expr(syn::Expr::Call(call), _) = &root.block.stmts[0] else {
            panic!("only black_box is called")
        };
        let syn::Expr::Path(path) = call.func.as_ref() else {
            panic!("fixed black_box path")
        };
        assert_eq!(
            quote::quote!(#path).to_string(),
            quote::quote!(black_box).to_string()
        );
        let syn::Expr::Struct(row) = table.expr.as_ref() else {
            panic!("static pointer table")
        };
        assert_eq!(row.fields.len(), 7);
        let pointers = row
            .fields
            .iter()
            .filter(|field| {
                ["validate", "new_cache", "initialize"].iter().any(
                    |name| matches!(&field.member, syn::Member::Named(member) if member == name),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(pointers.len(), 3);
        assert!(
            pointers
                .iter()
                .all(|field| matches!(&field.expr, syn::Expr::Path(_)))
        );
    }

    #[test]
    fn fake_retention_rows_use_a_checked_native_and_guest_inspection_layout() {
        let generated = retention(&["fake_owner".into()], &"ab".repeat(32)).unwrap();
        let file = syn::parse2::<syn::File>(generated).expect("quoted Rust");
        let row = file
            .items
            .iter()
            .find_map(|item| {
                let syn::Item::Struct(structure) = item else {
                    return None;
                };
                (structure.ident == "StagedValidator").then_some(structure)
            })
            .expect("fixed-layout inspection row");
        assert!(row.attrs.iter().any(|attribute| {
            attribute.path().is_ident("repr")
                && attribute
                    .parse_args::<syn::Ident>()
                    .is_ok_and(|repr| repr == "C")
        }));
        let fields = row
            .fields
            .iter()
            .map(|field| field.ident.as_ref().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            fields,
            [
                "magic",
                "identity",
                "owner",
                "owner_len",
                "validate",
                "new_cache",
                "initialize"
            ]
        );
        let code = quote::quote!(#file).to_string();
        assert!(code.contains("offset_of"));
        assert!(code.contains("size_of"));
        assert!(code.contains("AUCTIONEERSCHEMA"));
    }

    #[test]
    fn native_dispatch_is_closed_and_allocates_only_the_selected_cache() {
        let code = native_dispatch(&["fake_a".into(), "fake_b".into()]);
        let file = syn::parse2::<syn::File>(code).expect("quoted native dispatch");
        let text = quote::quote!(#file).to_string();
        assert!(text.contains("StagedSchema0 :: initialize_regexes"));
        assert!(text.contains("StagedSchema1 :: initialize_regexes"));
        assert!(!text.contains("true &&"));
        assert!(text.contains("\"fake_a\" =>"));
        assert!(text.contains("\"fake_b\" =>"));
        assert!(text.contains("_ => None"));
        assert_eq!(text.matches("new_search_cache ()").count(), 2);
    }

    #[test]
    fn native_publication_groups_alias_patterns_and_excludes_unpublished_members() {
        let manifest: Manifest = serde_json::from_value(json!({
            "key": "fake_factory", "factory": true, "native_only": true,
            "definitions": {"fake_alias": {}, "fake_pub": {}},
            "provenance": {"origin": "local"}
        }))
        .expect("fake native manifest");
        let catalog = Catalog {
            manifests: BTreeMap::from([("fake_factory".into(), manifest)]),
            input_paths: BTreeSet::new(),
        };
        let selection = Selection {
            members: ["fake_alias", "fake_pub", "fake_unpublished"]
                .into_iter()
                .map(|name| Member {
                    name,
                    implementation: "fake_factory",
                    is_root: true,
                    ancestry: Vec::new(),
                    schema_owner: Some("fake_owner"),
                })
                .collect(),
            schema_owners: vec!["fake_owner"],
        };
        let file = syn::parse2::<syn::File>(native_publication_bindings(&catalog, &selection))
            .expect("quoted publication dispatch");
        let text = quote::quote!(#file).to_string();
        assert!(text.contains("\"fake_alias\" | \"fake_pub\" => Some (\"fake_owner\")"));
        assert!(text.contains("matches ! (name , \"fake_alias\" | \"fake_pub\")"));
        assert!(!text.contains("fake_unpublished"));
        let empty = Selection {
            members: Vec::new(),
            schema_owners: Vec::new(),
        };
        let empty_file = syn::parse2::<syn::File>(native_publication_bindings(&catalog, &empty))
            .expect("quoted empty publication dispatch");
        let empty_text = quote::quote!(#empty_file).to_string();
        assert!(!empty_text.contains("match"));
        assert!(empty_text.contains("native_owner (_ : & str)"));
        assert!(empty_text.contains("native_required (_ : & str)"));
    }
}
