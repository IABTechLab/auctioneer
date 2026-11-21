//! Selected source and compiler-expanded membership only, not linked artifacts.
//! Executable declarations determine membership; source captures only witness it.

use super::schema_qualification::{self, Draft, SchemaAudit, SchemaInventory, SchemaWitness};
use super::schema_selection::{self, Identity, Schema};
use auctioneer_config::build_catalog::{self, Catalog, Category, Provenance};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const SOURCE: &str =
    include_str!("../conformance/fixtures/reference/catalog/source-inventory.toml");
const BINDINGS: &str =
    include_str!("../conformance/fixtures/reference/catalog/source-bindings.toml");
const WITNESSES: &str =
    include_str!("../conformance/fixtures/reference/catalog/supplemental-schemas.toml");

#[derive(Deserialize)]
struct SourceProjection {
    revision: String,
    identity: Vec<SourceIdentity>,
    implementations: Vec<Implementation>,
}
#[derive(Deserialize)]
struct SourceIdentity {
    name: String,
    alias_of: Option<String>,
}
#[derive(Deserialize)]
struct Implementation {
    source_directory: String,
    internal_key: String,
}
#[derive(Deserialize)]
struct BindingsProjection {
    revision: String,
    bindings: BTreeMap<String, String>,
}
#[derive(Clone)]
struct Definition {
    name: String,
    parent: Option<String>,
    implementation: String,
    feature: Option<String>,
    owner: Option<String>,
    local_free: bool,
}
#[derive(Clone, Serialize)]
struct Publication {
    name: String,
    implementation: String,
    required_feature: Option<String>,
    native_only: bool,
}
struct Inputs {
    root: PathBuf,
    definitions: Vec<Definition>,
    implemented: BTreeSet<String>,
    schemas: BTreeMap<String, SchemaInventory>,
    sources: BTreeMap<String, Value>,
    factories: BTreeMap<String, Option<String>>,
    publications: Vec<Publication>,
}

type SchemaMap = BTreeMap<String, SchemaInventory>;
type SourceMap = BTreeMap<String, Value>;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Lane {
    Secure,
    Development,
}

#[derive(Debug)]
pub(crate) struct SelectedCohort {
    pub root: PathBuf,
    pub schemas: Vec<SchemaInventory>,
    pub manifest: Value,
}

#[derive(Debug)]
struct LaneReport {
    schemas: Vec<SchemaInventory>,
    manifest: Value,
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn pinned<T: DeserializeOwned>(text: &str, sha256: &str) -> Result<T, String> {
    if digest(text.as_bytes()) != sha256 {
        return Err("source projection digest mismatch".to_owned());
    }
    toml::from_str(text).map_err(|error| error.to_string())
}

fn admit(audit: &SchemaAudit) -> Result<(), String> {
    if !matches!(audit.draft, Draft::Draft4 | Draft::Draft202012)
        || !audit.unknown_keywords.is_empty()
        || !audit.local_references.is_empty()
        || !audit.external_references.is_empty()
    {
        return Err("unqualified schema draft, keyword or reference".to_owned());
    }
    if audit
        .formats
        .iter()
        .any(|format| !matches!(format.value.as_str(), "uuid" | "hostname"))
    {
        return Err("unqualified shared format".to_owned());
    }
    Ok(())
}

fn witness_bytes(bytes: &[u8], length: u64, sha256: &str) -> Result<(), String> {
    if u64::try_from(bytes.len()).map_err(|error| error.to_string())? != length
        || digest(bytes) != sha256
    {
        return Err("schema witness mismatch".to_owned());
    }
    Ok(())
}

fn load() -> Result<Inputs, String> {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = crate_root.join("src/bidders");
    let snapshot = schema_qualification::inventory(
        &root,
        &crate_root.join("tests/conformance/fixtures/reference/catalog/source-inventory.toml"),
    )
    .map_err(|error| format!("inventory: {error:?}"))?;
    let source: SourceProjection = pinned(
        SOURCE,
        "b22f74407fd081cd7d073b8724ba8f74e1385163b9bc30c2277342adc38bb6c6",
    )?;
    let bindings: BindingsProjection = pinned(
        BINDINGS,
        "d1a0f18d99f53cbd4f62fe74c5d1e5580f8688fce95a29b1b05ee934dbe9ff0c",
    )?;
    if bindings.revision != source.revision {
        return Err("source projection contract mismatch".to_owned());
    }
    let crate_manifest: toml::Value = toml::from_str(
        &fs::read_to_string(crate_root.join("Cargo.toml")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let features = crate_manifest
        .get("features")
        .and_then(toml::Value::as_table)
        .ok_or("missing crate features")?
        .keys()
        .cloned()
        .collect();
    let catalog = build_catalog::scan(&root, &features).map_err(|error| error.to_string())?;
    let (schemas, sources) = schema_inputs(&root, &catalog, snapshot.schemas)?;
    let mut definitions = source_definitions(&source, &bindings, &catalog, &schemas)?;
    check_publications(&catalog)?;
    add_implemented_roots(&mut definitions, &catalog, &schemas)?;
    Ok(Inputs {
        root,
        definitions,
        schemas,
        sources,
        implemented: catalog
            .manifests
            .values()
            .filter(|manifest| manifest.factory)
            .flat_map(|manifest| manifest.definitions.keys().cloned())
            .collect(),
        factories: catalog
            .manifests
            .values()
            .filter(|manifest| manifest.factory)
            .map(|manifest| (manifest.key.clone(), manifest.required_feature.clone()))
            .collect(),
        publications: catalog
            .manifests
            .values()
            .flat_map(|manifest| {
                manifest.definitions.keys().map(|name| Publication {
                    name: name.clone(),
                    implementation: manifest.key.clone(),
                    required_feature: manifest.required_feature.clone(),
                    native_only: manifest.native_only,
                })
            })
            .collect(),
    })
}

fn schema_inputs(
    root: &Path,
    catalog: &Catalog,
    census: Vec<SchemaInventory>,
) -> Result<(SchemaMap, SourceMap), String> {
    let mut schemas: BTreeMap<_, _> = census
        .into_iter()
        .map(|schema| (schema.witness.name.clone(), schema))
        .collect();
    let mut sources = BTreeMap::new();
    for manifest in catalog.manifests.values() {
        if manifest
            .required_feature
            .as_deref()
            .is_some_and(|feature| feature != "insecure-bidders")
        {
            return Err(format!("unsupported build lane: {}", manifest.key));
        }
        let Provenance::Reference {
            sources: origins,
            artifacts,
            ..
        } = &manifest.provenance
        else {
            continue;
        };
        for artifact in artifacts
            .iter()
            .filter(|row| row.category == Category::Schema)
        {
            let name = Path::new(&artifact.source_path)
                .file_stem()
                .and_then(|name| name.to_str())
                .ok_or("invalid schema source identity")?;
            if artifact.source_path != format!("static/bidder-params/{name}.json") {
                return Err("unsupported schema source mapping".to_owned());
            }
            let retained = artifact
                .retained
                .as_ref()
                .ok_or("missing retained schema witness")?;
            let origin = origins
                .get(&artifact.source)
                .ok_or("missing schema source")?;
            // inventory has already validated the out-of-census supplemental exclusions.
            if !schemas.contains_key(name) {
                let bytes = fs::read(root.join(&manifest.key).join(&retained.path))
                    .map_err(|error| error.to_string())?;
                witness_bytes(&bytes, retained.bytes, &retained.sha256)?;
                let schema = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
                schemas.insert(
                    name.to_owned(),
                    SchemaInventory {
                        witness: SchemaWitness {
                            name: name.to_owned(),
                            path: format!("{}/{}", manifest.key, retained.path),
                            source_path: artifact.source_path.clone(),
                            source_blob: artifact.source_blob.clone(),
                            bytes: retained.bytes,
                            sha256: retained.sha256.clone(),
                        },
                        audit: schema_qualification::audit_schema(&schema)
                            .map_err(|error| format!("{error:?}"))?,
                    },
                );
            }
            if sources
                .insert(
                    name.to_owned(),
                    json!({"repository": origin.repository, "revision": origin.revision}),
                )
                .is_some()
            {
                return Err(format!("ambiguous schema witness: {name}"));
            }
        }
    }
    Ok((schemas, sources))
}

fn source_definitions(
    source: &SourceProjection,
    bindings: &BindingsProjection,
    catalog: &Catalog,
    schemas: &SchemaMap,
) -> Result<Vec<Definition>, String> {
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
    let mut definitions = Vec::new();
    for row in &source.identity {
        let mut current = row;
        let mut seen = BTreeSet::new();
        let implementation = loop {
            if !seen.insert(current.name.as_str()) {
                return Err("source alias cycle".to_owned());
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
            owner: schemas.get(&row.name).map(|_| row.name.clone()),
            local_free: false,
        });
    }
    Ok(definitions)
}

fn check_publications(catalog: &Catalog) -> Result<(), String> {
    let declarations = auctioneer_config_data::bidder_metadata_declarations();
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
    let published: BTreeSet<_> = declarations
        .iter()
        .map(|row| (row.bidder, row.implementation, row.required_feature))
        .collect();
    if expected != published || published.len() != declarations.len() {
        return Err("generated publication declarations mismatch".to_owned());
    }
    let expected_active: BTreeSet<_> = published
        .iter()
        .filter(|(_, implementation, feature)| {
            (feature.is_none() || cfg!(feature = "insecure-bidders"))
                && catalog
                    .manifests
                    .get(*implementation)
                    .is_some_and(|manifest| !manifest.native_only || !cfg!(target_arch = "wasm32"))
        })
        .map(|(name, implementation, _)| (*name, *implementation))
        .collect();
    let active = auctioneer_config_data::bidder_implementation_bindings();
    if active
        .iter()
        .map(|row| (row.bidder, row.implementation))
        .collect::<BTreeSet<_>>()
        != expected_active
        || active.len() != expected_active.len()
    {
        return Err("active Cargo publication lane mismatch".to_owned());
    }
    Ok(())
}

fn add_implemented_roots(
    definitions: &mut Vec<Definition>,
    catalog: &Catalog,
    schemas: &SchemaMap,
) -> Result<(), String> {
    for schema in schemas.values() {
        let name = &schema.witness.name;
        if definitions.iter().any(|row| row.name == *name) {
            continue;
        }
        let implementation = Path::new(&schema.witness.path)
            .components()
            .next()
            .and_then(|part| part.as_os_str().to_str())
            .ok_or("missing additional witness implementation")?;
        let manifest = catalog
            .manifests
            .get(implementation)
            .ok_or("missing additional witness manifest")?;
        definitions.push(Definition {
            name: name.clone(),
            parent: None,
            implementation: manifest.key.clone(),
            feature: manifest.required_feature.clone(),
            owner: Some(name.clone()),
            local_free: false,
        });
    }
    for manifest in catalog.manifests.values().filter(|manifest| manifest.factory) {
        for (name, declaration) in &manifest.definitions {
            let parent = declaration
                .get("aliasOf")
                .map(|value| value.as_str().ok_or("invalid implemented alias parent"))
                .transpose()?;
            if let Some(row) = definitions.iter_mut().find(|row| row.name == *name) {
                if row.implementation != manifest.key {
                    return Err("publication/source implementation mismatch".to_owned());
                }
                if let Some(parent) = parent {
                    row.parent = Some(parent.to_owned());
                }
                continue;
            }
            let local_free =
                parent.is_none() && matches!(manifest.provenance, Provenance::Local {});
            if !local_free && parent.is_none() && !schemas.contains_key(name) {
                return Err(format!("missing schema witness: {name}"));
            }
            definitions.push(Definition {
                name: name.to_owned(),
                parent: parent.map(str::to_owned),
                implementation: manifest.key.clone(),
                feature: manifest.required_feature.clone(),
                owner: schemas.get(name).map(|_| name.to_owned()),
                local_free,
            });
        }
    }
    for declaration in auctioneer_config_data::bidder_metadata_declarations() {
        if catalog
            .manifests
            .get(declaration.implementation)
            .is_none_or(|manifest| !manifest.factory)
        {
            return Err("implemented definition missing factory".to_owned());
        }
    }
    Ok(())
}

fn inventory_dispositions(inputs: &Inputs) -> Vec<Value> {
    let mut definitions: Vec<_> = inputs.definitions.iter().collect();
    definitions.sort_by_key(|row| row.name.as_str());
    definitions
        .into_iter()
        .map(|row| {
            json!({
                "name": row.name, "implementation": row.implementation,
                "disposition": if inputs.implemented.contains(&row.name) {
                    "implemented"
                } else {
                    "not_implemented"
                },
            })
        })
        .collect()
}

fn current_callables(publications: &[Publication]) -> BTreeSet<(&str, &str)> {
    publications
        .iter()
        .filter(|row| {
            (row.required_feature.is_none() || cfg!(feature = "insecure-bidders"))
                && (!row.native_only || !cfg!(target_arch = "wasm32"))
        })
        .map(|row| (row.name.as_str(), row.implementation.as_str()))
        .collect()
}

impl Inputs {
    fn selected_lane(&self, lane: Lane) -> Result<SelectedCohort, String> {
        let (source, selected) = self.manifest(lane)?;
        let name = match lane {
            Lane::Secure => "secure",
            Lane::Development => "development",
        };
        Ok(SelectedCohort {
            root: self.root.clone(),
            schemas: selected.schemas,
            manifest: json!({"schema_version": 2, "source": source,
                "selection": selected.manifest, "lane": name}),
        })
    }

    fn lane(&self, features: &[&str]) -> Result<LaneReport, String> {
        let slots: Vec<_> = self
            .definitions
            .iter()
            .map(|row| [row.owner.as_deref().unwrap_or("")])
            .collect();
        let definitions: Vec<_> = self
            .definitions
            .iter()
            .zip(&slots)
            .map(|(row, owner)| Identity {
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
        let roots: Vec<_> = self.implemented.iter().map(String::as_str).collect();
        let selection = schema_selection::select(&definitions, &roots, features)?;
        for member in &selection.members {
            let declaration = self
                .factories
                .get(member.implementation)
                .ok_or_else(|| format!("missing factory declaration: {}", member.implementation))?;
            let definition = self
                .definitions
                .iter()
                .find(|row| row.name == member.name)
                .ok_or("missing selected source identity")?;
            if declaration.as_deref() != definition.feature.as_deref() {
                return Err(format!("factory feature mismatch: {}", member.name));
            }
        }
        let mut schemas = Vec::new();
        let mut selected_schemas = Vec::new();
        let mut formats = BTreeSet::new();
        let mut helper_count = 0;
        for owner in &selection.schema_owners {
            let schema = self
                .schemas
                .get(*owner)
                .ok_or_else(|| format!("missing schema witness: {owner}"))?;
            admit(&schema.audit)?;
            let bytes = fs::read(self.root.join(&schema.witness.path))
                .map_err(|error| error.to_string())?;
            witness_bytes(&bytes, schema.witness.bytes, &schema.witness.sha256)?;
            let mut helpers = super::bundle_patterns(&self.root, schema)?;
            helpers.sort();
            helper_count =
                usize::checked_add(helper_count, helpers.len()).ok_or("helper count overflow")?;
            formats.extend(
                schema
                    .audit
                    .formats
                    .iter()
                    .map(|format| format.value.as_str()),
            );
            schemas.push(json!({"witness": schema.witness,
                "source": self.sources.get(*owner).ok_or("missing schema source witness")?,
                "generated_helpers_sha256": digest(&serde_json::to_vec(&helpers).map_err(|error| error.to_string())?),
                "generated_helpers": helpers, "formats": schema.audit.formats,
                "patterns": schema.audit.patterns, "references": schema.audit.local_references}));
            selected_schemas.push(schema.clone());
        }
        let members: Vec<_> = selection.members.iter().map(|row| json!({
            "name": row.name, "implementation": row.implementation, "is_root": row.is_root,
            "implemented": self.implemented.contains(row.name),
            "ancestry": row.ancestry, "effective_schema_owner": row.schema_owner,
        })).collect();
        let active_roots: Vec<_> = selection
            .members
            .iter()
            .filter(|row| row.is_root)
            .map(|row| row.name)
            .collect();
        let shared_regex_helpers = if formats.contains("hostname") {
            vec![("shared-hostname", super::schema_formats::HOSTNAME_SOURCE)]
        } else {
            vec![]
        };
        Ok(LaneReport {
            schemas: selected_schemas,
            manifest: json!({"roots": active_roots, "members": members,
            "effective_schema_owners": selection.schema_owners, "schema_count": schemas.len(), "schemas": schemas,
            "generated_helper_count": helper_count, "shared_formats": formats,
            "shared_regex_helpers": shared_regex_helpers}),
        })
    }

    fn manifest(&self, lane: Lane) -> Result<(Value, LaneReport), String> {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let compiler_vendor = super::schema_vendor::verify(&workspace_root)?;
        let regex_vendor = super::schema_build::schema_regex_vendor::verify(&workspace_root)
            .map_err(|error| error.to_string())?;
        let workspace_manifest =
            fs::read(workspace_root.join("Cargo.toml")).map_err(|error| error.to_string())?;
        let build: toml::Value = toml::from_str(include_str!(concat!(
            env!("OUT_DIR"),
            "/schema-layout-build.toml"
        )))
        .map_err(|error| error.to_string())?;
        let lock = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock"))
            .map_err(|error| error.to_string())?;
        let secure = self.lane(&[])?;
        let development = self.lane(&["insecure-bidders"])?;
        let manifest = json!({"schema_version": 2_u8, "kind": "selected_source_and_expanded_generated_membership",
            "linked_or_serving_qualification": false,
            "pending_gaps": ["linked_artifact_membership", "serving_integration", "resident_resource_bounds", "all_target_qualification"],
            "native_test_build": build,
            "workspace_manifest_sha256": digest(&workspace_manifest),
            "compiler_vendor": {"verified_root": compiler_vendor,
                "verifier_source_sha256": digest(include_bytes!("../schema_vendor/mod.rs"))},
            "regex_vendor": {"verified_source": regex_vendor,
                "verifier_source_sha256": digest(include_bytes!("../schema_build/schema_regex_vendor/mod.rs"))},
            "lock_sha256": digest(&lock),
            "producer_source_sha256": digest(include_bytes!("mod.rs")),
            "expansion_source_sha256": digest(include_bytes!("../schema_source_conformance.rs")),
            "retained_schema_witness_count": self.schemas.len(),
            "shared_formats_source_sha256": digest(include_bytes!("../schema_formats/mod.rs")),
            "source_digests": {"inventory": digest(SOURCE.as_bytes()), "bindings": digest(BINDINGS.as_bytes()),
                "additional_witnesses": digest(WITNESSES.as_bytes())},
            "expansion": {"cache": "explicit", "engine": "regex", "validate_formats": true,
                "size_limit": 0x0001_0000_u32, "dfa_size_limit": 0x0001_0000_u32, "methods": ["is_valid"]},
            "implemented_roots_all_features": self.implemented,
            "inventory_dispositions": inventory_dispositions(self),
            "publication_predicates": self.publications,
            "actual_cargo_insecure_bidders": cfg!(feature = "insecure-bidders"),
            "callable_bindings": current_callables(&self.publications),
            "lanes": {"secure": &secure.manifest, "development": &development.manifest}});
        let selected = match lane {
            Lane::Secure => secure,
            Lane::Development => development,
        };
        Ok((manifest, selected))
    }
}

pub(crate) fn produce(reverse_definitions: bool) -> Result<Value, String> {
    let mut inputs = load()?;
    if reverse_definitions {
        inputs.definitions.reverse();
    }
    inputs.manifest(Lane::Secure).map(|(manifest, _)| manifest)
}

/// A separate alias walk over raw declarations, never production-selected roots.
/// Factory/definition membership comes directly from validated manifests.
pub(crate) fn independent_membership(lane: Lane) -> Result<Value, String> {
    let inputs = load()?;
    let roots: BTreeSet<_> = inputs
        .publications
        .iter()
        .map(|row| row.name.as_str())
        .collect();
    let declarations: BTreeMap<_, _> = inputs
        .definitions
        .iter()
        .map(|row| (row.name.to_ascii_lowercase(), row))
        .collect();
    if declarations.len() != inputs.definitions.len() || roots.len() != inputs.publications.len() {
        return Err("independent declaration identity mismatch".into());
    }
    let mut members = BTreeMap::new();
    let mut owners = BTreeSet::new();
    let mut lane_roots = BTreeSet::new();
    for publication in &inputs.publications {
        if publication.required_feature.is_some() && !matches!(lane, Lane::Development) {
            continue;
        }
        lane_roots.insert(publication.name.as_str());
        let mut path = Vec::new();
        let mut visited = BTreeSet::new();
        let mut current = publication.name.as_str();
        loop {
            let folded = current.to_ascii_lowercase();
            if !visited.insert(folded.clone()) {
                return Err("independent alias cycle".into());
            }
            let row = *declarations
                .get(&folded)
                .ok_or("independent alias parent missing")?;
            path.push(row.name.as_str());
            if row.implementation != publication.implementation
                || row.feature != publication.required_feature
            {
                return Err("independent alias ownership mismatch".into());
            }
            match row.parent.as_deref() {
                Some(parent) => current = parent,
                None => break,
            }
        }
        for (index, name) in path.iter().enumerate() {
            let row = declarations
                .get(&name.to_ascii_lowercase())
                .ok_or("independent member missing")?;
            let owner = path[index..].iter().find_map(|ancestor| {
                declarations
                    .get(&ancestor.to_ascii_lowercase())
                    .and_then(|row| row.owner.as_deref())
            });
            if owner.is_none() && !row.local_free {
                return Err("independent schema owner missing".into());
            }
            if index == 0 {
                owners.extend(owner);
            }
            members.insert(
                name.to_ascii_lowercase(),
                json!({"name": row.name, "implementation": row.implementation,
                    "is_root": roots.contains(row.name.as_str()),
                    "implemented": roots.contains(row.name.as_str()),
                    "ancestry": &path[index + 1..], "effective_schema_owner": owner}),
            );
        }
    }
    for owner in &owners {
        if !inputs.schemas.contains_key(*owner) {
            return Err("independent schema witness missing".into());
        }
    }
    let callables: BTreeSet<_> = inputs
        .publications
        .iter()
        .filter(|row| {
            (row.required_feature.is_none() || cfg!(feature = "insecure-bidders"))
                && (!row.native_only || !cfg!(target_arch = "wasm32"))
        })
        .map(|row| (row.name.as_str(), row.implementation.as_str()))
        .collect();
    let mut inventory_rows: Vec<_> = inputs.definitions.iter().collect();
    inventory_rows.sort_by(|left, right| left.name.cmp(&right.name));
    let dispositions: Vec<_> = inventory_rows
        .iter()
        .map(|row| {
            let implemented = inputs
                .publications
                .iter()
                .any(|publication| publication.name == row.name);
            json!({"name": row.name, "implementation": row.implementation,
                "disposition": if implemented { "implemented" } else { "not_implemented" }})
        })
        .collect();
    Ok(json!({"implemented_roots_all_features": roots,
        "inventory_dispositions": dispositions,
        "publication_predicates": inputs.publications,
        "callable_bindings": callables,
        "roots": lane_roots, "members": members.into_values().collect::<Vec<_>>(),
        "effective_schema_owners": owners}))
}

pub(crate) fn selected_cohort(lane: Lane) -> Result<SelectedCohort, String> {
    load()?.selected_lane(lane)
}

#[cfg(test)]
mod tests {
    use super::{Definition, Inputs, Lane, Publication, admit, digest, pinned, witness_bytes};
    use crate::schema_qualification::{SchemaInventory, SchemaWitness, audit_schema};
    use serde_json::json;
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::PathBuf;

    fn fake_inputs() -> Inputs {
        Inputs {
            root: PathBuf::new(),
            definitions: vec![Definition {
                name: "fake_root".to_owned(),
                parent: None,
                implementation: "fake_factory".to_owned(),
                feature: None,
                owner: None,
                local_free: true,
            }],
            implemented: BTreeSet::from(["fake_root".to_owned()]),
            schemas: BTreeMap::new(),
            sources: BTreeMap::new(),
            factories: BTreeMap::from([("fake_factory".to_owned(), None)]),
            publications: vec![Publication {
                name: "fake_root".into(),
                implementation: "fake_factory".into(),
                required_feature: None,
                native_only: false,
            }],
        }
    }

    fn fake_schema_inputs() -> (tempfile::TempDir, Inputs) {
        let directory = tempfile::tempdir().expect("fake schema directory");
        let mut inputs = fake_inputs();
        inputs.root = directory.path().to_owned();
        inputs.definitions.clear();
        inputs.implemented.clear();
        inputs.factories.clear();
        inputs.publications.clear();
        let value = json!({"$schema":"http://json-schema.org/draft-04/schema#", "type":"string"});
        let bytes = serde_json::to_vec(&value).expect("fake schema bytes");
        for (name, feature, revision) in [
            ("fake_primary", None, "fake_primary_revision"),
            (
                "fake_supplemental",
                Some("insecure-bidders"),
                "fake_supplemental_revision",
            ),
        ] {
            let path = format!("{name}.json");
            fs::write(inputs.root.join(&path), &bytes).expect("fake retained schema");
            inputs.definitions.push(Definition {
                name: name.to_owned(),
                parent: None,
                implementation: name.to_owned(),
                feature: feature.map(str::to_owned),
                owner: Some(name.to_owned()),
                local_free: false,
            });
            inputs.implemented.insert(name.to_owned());
            inputs.publications.push(Publication {
                name: name.to_owned(),
                implementation: name.to_owned(),
                required_feature: feature.map(str::to_owned),
                native_only: false,
            });
            inputs
                .factories
                .insert(name.to_owned(), feature.map(str::to_owned));
            inputs.schemas.insert(
                name.to_owned(),
                SchemaInventory {
                    witness: SchemaWitness {
                        name: name.to_owned(),
                        path,
                        source_path: format!("static/bidder-params/{name}.json"),
                        source_blob: format!("fake_blob_{name}"),
                        bytes: u64::try_from(bytes.len()).expect("fake schema length"),
                        sha256: digest(&bytes),
                    },
                    audit: audit_schema(&value).expect("fake schema audit"),
                },
            );
            inputs.sources.insert(
                name.to_owned(),
                json!({
                    "repository": "https://fake.example/source", "revision": revision,
                }),
            );
        }
        (directory, inputs)
    }

    #[test]
    fn fake_selected_cohort_owns_lane_schemas_and_preserves_source_witnesses() {
        let (_directory, inputs) = fake_schema_inputs();
        let secure = inputs
            .selected_lane(Lane::Secure)
            .expect("fake secure cohort");
        let development = inputs
            .selected_lane(Lane::Development)
            .expect("fake development cohort");
        let expected: Vec<_> = inputs.schemas.values().cloned().collect();
        let root = inputs.root.clone();
        drop(inputs);
        assert_eq!(secure.root, root);
        assert_eq!(development.root, root);
        assert_eq!(secure.schemas, expected[..1]);
        assert_eq!(development.schemas, expected);
        assert_eq!(secure.manifest["source"], development.manifest["source"]);
        assert_eq!(
            secure.manifest["source"]["implemented_roots_all_features"],
            json!(["fake_primary", "fake_supplemental"])
        );
        let expected_callables = if cfg!(feature = "insecure-bidders") {
            json!([["fake_primary", "fake_primary"], ["fake_supplemental", "fake_supplemental"]])
        } else {
            json!([["fake_primary", "fake_primary"]])
        };
        assert_eq!(
            secure.manifest["source"]["callable_bindings"],
            expected_callables
        );
        for (cohort, name, count) in [
            (&secure, "secure", 1_usize),
            (&development, "development", 2_usize),
        ] {
            assert_eq!(cohort.manifest["schema_version"], 2_u8);
            assert_eq!(cohort.manifest["lane"], name);
            assert_eq!(cohort.manifest["selection"]["schema_count"], count);
            assert_eq!(
                cohort.manifest["source"]["linked_or_serving_qualification"],
                false
            );
            assert_eq!(
                cohort.manifest["selection"],
                cohort.manifest["source"]["lanes"][name]
            );
            let witnesses: Vec<_> = cohort
                .schemas
                .iter()
                .map(|row| json!(row.witness))
                .collect();
            let reported: Vec<_> = cohort.manifest["selection"]["schemas"]
                .as_array()
                .expect("fake selection schemas")
                .iter()
                .map(|row| row["witness"].clone())
                .collect();
            assert_eq!(witnesses, reported);
        }
        for (schema, row) in development.schemas.iter().zip(
            development.manifest["selection"]["schemas"]
                .as_array()
                .expect("fake manifest schemas"),
        ) {
            assert_eq!(row["witness"], json!(schema.witness));
            assert_eq!(
                row["source"]["revision"],
                format!("{}_revision", schema.witness.name)
            );
        }
    }

    #[test]
    fn fake_selected_cohort_reuses_factory_reconciliation() {
        let mut inputs = fake_inputs();
        inputs.factories.clear();
        assert!(
            inputs
                .selected_lane(Lane::Secure)
                .unwrap_err()
                .contains("factory declaration")
        );
        inputs.factories.insert(
            "fake_factory".to_owned(),
            Some("insecure-bidders".to_owned()),
        );
        assert!(
            inputs
                .selected_lane(Lane::Secure)
                .unwrap_err()
                .contains("factory feature")
        );
    }

    #[test]
    fn fake_selected_cohort_rechecks_retained_bytes_and_source_witnesses() {
        let (_directory, mut inputs) = fake_schema_inputs();
        inputs
            .schemas
            .get_mut("fake_primary")
            .expect("fake primary schema")
            .witness
            .bytes += 1;
        assert!(
            inputs
                .selected_lane(Lane::Secure)
                .unwrap_err()
                .contains("schema witness mismatch")
        );
        inputs
            .schemas
            .get_mut("fake_primary")
            .expect("fake primary schema")
            .witness
            .bytes -= 1;
        inputs.sources.remove("fake_primary");
        assert!(
            inputs
                .selected_lane(Lane::Secure)
                .unwrap_err()
                .contains("missing schema source witness")
        );
    }

    #[test]
    fn fake_missing_roots_and_schema_witnesses_refuse() {
        let mut inputs = fake_inputs();
        inputs.implemented.insert("fake_unknown".to_owned());
        assert!(inputs.lane(&[]).unwrap_err().contains("unknown root"));
        inputs.implemented.remove("fake_unknown");
        inputs.definitions[0].local_free = false;
        assert!(
            inputs
                .lane(&[])
                .unwrap_err()
                .contains("missing schema owner")
        );
        inputs.definitions[0].owner = Some("fake_schema".to_owned());
        assert!(
            inputs
                .lane(&[])
                .unwrap_err()
                .contains("missing schema witness")
        );
    }

    #[test]
    fn fake_selected_members_require_matching_factory_declarations() {
        let mut inputs = fake_inputs();
        inputs.factories.clear();
        assert!(
            inputs
                .lane(&[])
                .unwrap_err()
                .contains("factory declaration")
        );
        inputs.factories.insert(
            "fake_factory".to_owned(),
            Some("insecure-bidders".to_owned()),
        );
        assert!(inputs.lane(&[]).unwrap_err().contains("factory feature"));
    }

    #[test]
    fn fake_unused_source_only_definition_does_not_require_a_factory() {
        let mut inputs = fake_inputs();
        inputs.definitions.push(Definition {
            name: "fake_source_only".to_owned(),
            implementation: "fake_unimplemented".to_owned(),
            owner: Some("fake_source_schema".to_owned()),
            local_free: false,
            parent: None,
            feature: None,
        });
        inputs
            .lane(&[])
            .expect("unused source-only factory is not required");
    }

    #[test]
    fn fake_projection_and_schema_digest_drift_refuse() {
        let text = "name = 'fake_projection'\n";
        pinned::<toml::Value>(text, &digest(text.as_bytes())).expect("pinned projection");
        pinned::<toml::Value>(text, &"0".repeat(64)).expect_err("projection drift");
        witness_bytes(b"fake_schema", 11, &digest(b"fake_schema")).expect("schema witness");
        witness_bytes(b"fake_schema", 12, &digest(b"fake_schema")).expect_err("size drift");
        witness_bytes(b"fake_schema", 11, &digest(b"fake_changed")).expect_err("digest drift");
    }

    #[test]
    fn fake_external_unqualified_local_references_drafts_and_formats_refuse() {
        let valid = json!({"$schema":"http://json-schema.org/draft-04/schema#", "type":"string"});
        admit(&audit_schema(&valid).unwrap()).expect("qualified fake schema");
        for extra in [
            json!({"$ref":"https://fake.example/schema"}),
            json!({"$ref":"#/missing"}),
            json!({"$ref":"#fake_anchor"}),
            json!({"$ref":"#/definitions/fake", "definitions":{"fake":{"type":"string"}}}),
            json!({"format":"fake_unsupported"}),
            json!({"fake_keyword":true}),
        ] {
            let mut schema = valid.clone();
            schema
                .as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            admit(&audit_schema(&schema).unwrap()).expect_err("unqualified fake schema");
        }
        admit(&audit_schema(&json!({"type":"string"})).unwrap()).expect_err("unqualified draft");
    }
}
