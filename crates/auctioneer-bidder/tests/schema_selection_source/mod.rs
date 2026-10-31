//! Selected source and compiler-expanded membership only, not linked artifacts.
//! The digest-pinned priority projection does not replace CLI wave validation.

use super::schema_qualification::{self, Draft, SchemaAudit, SchemaInventory, SchemaWitness};
use super::schema_selection::{self, Identity, Schema};
use auctioneer_config_schema::build_catalog::{self, Catalog, Category, Provenance};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const SOURCE: &str =
    include_str!("../conformance/fixtures/reference/catalog/source-inventory.toml");
const BINDINGS: &str =
    include_str!("../conformance/fixtures/reference/catalog/source-bindings.toml");
const PRIORITY: &str = include_str!("../conformance/fixtures/waves/priority.toml");
const SUPPLEMENTAL: &str = include_str!("../conformance/fixtures/waves/priority-supplemental.toml");

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
    blob: String,
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
#[derive(Deserialize)]
struct WaveProjection {
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
struct SupplementalProjection {
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
#[derive(Clone)]
struct Definition {
    name: String,
    parent: Option<String>,
    implementation: String,
    feature: Option<String>,
    owner: Option<String>,
    local_free: bool,
}
struct Inputs {
    root: PathBuf,
    definitions: Vec<Definition>,
    published: BTreeSet<String>,
    planned: BTreeSet<String>,
    schemas: BTreeMap<String, SchemaInventory>,
    sources: BTreeMap<String, Value>,
    factories: BTreeMap<String, Option<String>>,
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
    format!("{:x}", Sha256::digest(bytes))
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
    let wave: WaveProjection = pinned(
        PRIORITY,
        "aa9d38e3e6221c13d13b28ba18684e041f48e3c15608aa60ee11db86924a0c24",
    )?;
    let supplemental: SupplementalProjection = pinned(
        SUPPLEMENTAL,
        "ee94f5753f6244fa2980670a3fdf65ea69df2a157532a8776d061d9b2e1f66aa",
    )?;
    if bindings.revision != source.revision
        || wave.revision != source.revision
        || wave.schema_version != 1
        || supplemental.schema_version != 1
        || supplemental.kind != "priority_supplemental_schema_membership"
        || supplemental.witness_inventory != "reference/catalog/supplemental-schemas.toml"
    {
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
    add_extra_roots(&mut definitions, &catalog, &schemas, &supplemental)?;
    for row in &wave.bidders {
        let identity = source
            .identity
            .iter()
            .find(|identity| identity.name == row.name)
            .ok_or("unknown planned root")?;
        let definition = definitions
            .iter()
            .find(|definition| definition.name == row.name)
            .ok_or("missing planned binding")?;
        let schema = schemas
            .get(&row.base)
            .ok_or("missing planned schema witness")?;
        if identity.blob != row.metadata_blob
            || schema.witness.source_blob != row.schema_blob
            || definition.implementation != row.implementation
        {
            return Err("planned source witness mismatch".to_owned());
        }
    }
    for row in &supplemental.bidders {
        let schema = schemas
            .get(&row.schema_witness)
            .ok_or("missing supplemental schema witness")?;
        if schema.witness.name != row.name
            || schema.witness.path.split('/').next() != Some(row.implementation.as_str())
        {
            return Err("supplemental source witness mismatch".to_owned());
        }
    }
    Ok(Inputs {
        root,
        definitions,
        schemas,
        sources,
        published: auctioneer_config::bidder_metadata_declarations()
            .iter()
            .map(|row| row.bidder.to_owned())
            .collect(),
        planned: wave
            .bidders
            .into_iter()
            .map(|row| row.name)
            .chain(supplemental.bidders.into_iter().map(|row| row.name))
            .collect(),
        factories: catalog
            .manifests
            .values()
            .filter(|manifest| manifest.factory)
            .map(|manifest| (manifest.key.clone(), manifest.required_feature.clone()))
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
    let declarations = auctioneer_config::bidder_metadata_declarations();
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
    let active = auctioneer_config::bidder_implementation_bindings();
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

fn add_extra_roots(
    definitions: &mut Vec<Definition>,
    catalog: &Catalog,
    schemas: &SchemaMap,
    supplemental: &SupplementalProjection,
) -> Result<(), String> {
    for (name, implementation) in auctioneer_config::bidder_metadata_declarations()
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
                return Err("publication/source implementation mismatch".to_owned());
            }
            continue;
        }
        let manifest = catalog
            .manifests
            .get(implementation)
            .ok_or("missing supplemental/local manifest")?;
        let local_free = matches!(manifest.provenance, Provenance::Local {});
        if !local_free && !schemas.contains_key(name) {
            return Err(format!("missing schema witness: {name}"));
        }
        definitions.push(Definition {
            name: name.to_owned(),
            parent: None,
            implementation: implementation.to_owned(),
            feature: manifest.required_feature.clone(),
            owner: schemas.get(name).map(|_| name.to_owned()),
            local_free,
        });
    }
    Ok(())
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
            manifest: json!({"schema_version": 1, "source": source,
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
        let roots: Vec<_> = self
            .published
            .union(&self.planned)
            .map(String::as_str)
            .collect();
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
            "published": self.published.contains(row.name), "planned": self.planned.contains(row.name),
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
            manifest: json!({"roots": active_roots, "members": members, "schema_count": schemas.len(), "schemas": schemas,
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
        let manifest = json!({"schema_version": 1_u8, "kind": "selected_source_and_expanded_generated_membership",
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
                "priority": digest(PRIORITY.as_bytes()), "priority_supplemental": digest(SUPPLEMENTAL.as_bytes())},
            "expansion": {"cache": "explicit", "engine": "regex", "validate_formats": true,
                "size_limit": 0x0001_0000_u32, "dfa_size_limit": 0x0001_0000_u32, "methods": ["is_valid"]},
            "planned_roots": self.planned, "published_roots_all_features": self.published,
            "actual_cargo_insecure_bidders": cfg!(feature = "insecure-bidders"),
            "actual_active_published_bindings": auctioneer_config::bidder_implementation_bindings().iter()
                .map(|row| (row.bidder, row.implementation)).collect::<Vec<_>>(),
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

/// Reconcile pinned source roots and publication predicates independently of a report.
pub(crate) fn planned_source_roots() -> Result<BTreeSet<String>, String> {
    load().map(|inputs| inputs.planned)
}

pub(crate) fn selected_cohort(lane: Lane) -> Result<SelectedCohort, String> {
    load()?.selected_lane(lane)
}

#[cfg(test)]
mod tests {
    use super::{Definition, Inputs, Lane, admit, digest, pinned, witness_bytes};
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
            published: BTreeSet::from(["fake_root".to_owned()]),
            planned: BTreeSet::new(),
            schemas: BTreeMap::new(),
            sources: BTreeMap::new(),
            factories: BTreeMap::from([("fake_factory".to_owned(), None)]),
        }
    }

    fn fake_schema_inputs() -> (tempfile::TempDir, Inputs) {
        let directory = tempfile::tempdir().expect("fake schema directory");
        let mut inputs = fake_inputs();
        inputs.root = directory.path().to_owned();
        inputs.definitions.clear();
        inputs.published.clear();
        inputs.factories.clear();
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
            inputs.published.insert(name.to_owned());
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
        for (cohort, name, count) in [
            (&secure, "secure", 1_usize),
            (&development, "development", 2_usize),
        ] {
            assert_eq!(cohort.manifest["schema_version"], 1_u8);
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
        inputs.published.insert("fake_unknown".to_owned());
        assert!(inputs.lane(&[]).unwrap_err().contains("unknown root"));
        inputs.published.remove("fake_unknown");
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
