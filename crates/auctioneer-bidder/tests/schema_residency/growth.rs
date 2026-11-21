//! Same-build data-only source reconstruction relative to d26fcf7.
//! No historical build, serving activation, physical fit or common C/I claim.

#![cfg(all(test, not(target_arch = "wasm32")))]

use super::{descriptor, expanded, observations, runtime_sources};
use crate::schema_build::{self, application::CapturedSeed, schema_regex_vendor};
use crate::schema_formats::HOSTNAME_SOURCE;
use crate::schema_selection_source::{Lane, independent_source_membership};
use crate::schema_vendor;
use auctioneer_config::bidder_discovery::{self, Discovery};
use auctioneer_config::build_catalog::{Category, Provenance};
use auctioneer_config::schema_staging::BuildContext;
use auctioneer_config::schema_staging_build::{
    BuildRequest, GeneratedStaging, catalog_reference, generate, write_snapshots,
};
use auctioneer_config::schema_staging_codegen::CANDIDATE_OPTIONS;
use auctioneer_config::types::BidderMetadataDeclaration;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::env::{self, consts};
use std::ffi::OsString;
use std::fs;
use std::io::Read as _;
use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
use std::path::{Component, Path, PathBuf};
use std::str;

const LABEL: &str = "same-build data-only source reconstruction relative to d26fcf7";
const INCOMPLETE: &str = "catalog growth reconstruction incomplete";
const SCHEDULE: &str = "expanded schedule mismatch";
const SCHEMAS: &str = "independent schema/provenance mismatch";
// Same existing per-input staging cap; this is only a test-fixture interning fence.
const INPUT_BYTES: usize = 1 << 20;
const NATIVE_STAGING: &[u8] = include_bytes!("../../src/schema/staging.rs");
const CURRENT_MANIFEST: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/schema-staging.json"));
const CURRENT_RUST: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/schema-staging.rs"));
const CURRENT_LAYOUT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/schema-layout-build.toml"));

// Source is data here, never a handwritten import of another owner's code.
macro_rules! measurement_sources {
    ($($relative:literal),+ $(,)?) => {
        &[$(($relative, include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"), "/../../", $relative
        )).as_slice())),+]
    };
}

const MEASUREMENT_SOURCES: &[(&str, &[u8])] = measurement_sources!(
    "Cargo.toml",
    "Cargo.lock",
    "crates/auctioneer-bidder/Cargo.toml",
    "crates/auctioneer-bidder/build.rs",
    "crates/auctioneer-bidder/src/schema/staging.rs",
    "crates/auctioneer-config/Cargo.toml",
    "crates/auctioneer-config/src/lib.rs",
    "crates/auctioneer-config/src/types.rs",
    "crates/auctioneer-config/src/schema_staging.rs",
    "crates/auctioneer-config/src/schema_staging_codegen.rs",
    "crates/auctioneer-config/src/schema_staging_build.rs",
    "crates/auctioneer-config/src/catalog_reference.rs",
    "crates/auctioneer-config/src/bidder_discovery.rs",
    "crates/auctioneer-config/src/bidder_reconcile.rs",
    "crates/auctioneer-config/src/bidder_manifest.rs",
    "crates/auctioneer-config/src/build_catalog.rs",
    "crates/auctioneer-config/src/endpoint_contract.rs",
    "crates/auctioneer-config/src/codegen/mod.rs",
    "crates/auctioneer-config/src/codegen/storage.rs",
    "crates/auctioneer-config/src/bindings.rs",
    "crates/auctioneer-config-data/Cargo.toml",
    "crates/auctioneer-config-data/build.rs",
    "crates/auctioneer-config-data/src/lib.rs",
    "crates/auctioneer-bidder/tests/schema_source_conformance.rs",
    "crates/auctioneer-bidder/tests/schema_residency/growth.rs",
    "crates/auctioneer-bidder/tests/schema_residency/expanded.rs",
    "crates/auctioneer-bidder/tests/schema_residency/auxiliary.rs",
    "crates/auctioneer-bidder/tests/schema_residency/options.rs",
    "crates/auctioneer-bidder/tests/schema_residency/storage.rs",
    "crates/auctioneer-bidder/tests/schema_residency/observations.rs",
    "crates/auctioneer-bidder/tests/schema_residency/runtime_sources.rs",
    "crates/auctioneer-bidder/tests/schema_build/mod.rs",
    "crates/auctioneer-bidder/tests/schema_build/application.rs",
    "crates/auctioneer-bidder/tests/schema_build/schema_regex_vendor/mod.rs",
    "crates/auctioneer-bidder/tests/schema_vendor/mod.rs",
    "crates/auctioneer-bidder/tests/schema_cache_model/mod.rs",
    "crates/auctioneer-bidder/tests/schema_formats/mod.rs",
    "crates/auctioneer-bidder/tests/schema_meter/mod.rs",
    "crates/auctioneer-bidder/tests/schema_selection/mod.rs",
    "crates/auctioneer-bidder/tests/schema_selection_source/mod.rs",
    "crates/auctioneer-bidder/tests/schema_qualification/mod.rs",
);

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct Delta {
    added: BTreeSet<String>,
    removed: BTreeSet<String>,
    changed: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct View {
    source: Value,
    measurement: Value,
    membership: Value,
    owners: BTreeMap<String, String>,
    programs: Vec<(String, String)>,
    seed_sha256: String,
    expanded_sha256: String,
    generated_source_bytes: u64,
}

fn reference_path(value: Option<OsString>) -> Result<PathBuf, String> {
    let path = value.ok_or("catalog reference directory is required")?;
    if path.is_empty() {
        return Err("catalog reference directory is empty".into());
    }
    Ok(PathBuf::from(path))
}

fn intern_rows(
    rows: &[(&str, &str, Option<&str>)],
) -> Result<Vec<BidderMetadataDeclaration>, String> {
    let mut bytes = 0_usize;
    let mut names = BTreeSet::new();
    for &(bidder, implementation, feature) in rows {
        for field in [Some(bidder), Some(implementation), feature]
            .into_iter()
            .flatten()
        {
            bytes = bytes
                .checked_add(field.len())
                .ok_or("metadata interning bound")?;
            if bytes > INPUT_BYTES {
                return Err("metadata interning bound".into());
            }
        }
        if !names.insert(bidder.to_ascii_lowercase()) {
            return Err("duplicate metadata declaration".into());
        }
    }
    // The API uses static declarations. Bound and validate the entire arena
    // before interning; this fixture allocation is outside observation windows.
    Ok(rows
        .iter()
        .map(
            |&(bidder, implementation, required_feature)| BidderMetadataDeclaration {
                bidder: Box::leak(bidder.to_owned().into_boxed_str()),
                implementation: Box::leak(implementation.to_owned().into_boxed_str()),
                required_feature: required_feature.map(|feature| {
                    let interned: &'static str = Box::leak(feature.to_owned().into_boxed_str());
                    interned
                }),
            },
        )
        .collect())
}

fn owner_delta(
    reference: &BTreeMap<String, String>,
    candidate: &BTreeMap<String, String>,
) -> Delta {
    Delta {
        added: candidate
            .keys()
            .filter(|owner| !reference.contains_key(*owner))
            .cloned()
            .collect(),
        removed: reference
            .keys()
            .filter(|owner| !candidate.contains_key(*owner))
            .cloned()
            .collect(),
        changed: reference
            .iter()
            .filter(|(owner, schema)| candidate.get(*owner).is_some_and(|other| other != *schema))
            .map(|(owner, _schema)| owner.clone())
            .collect(),
    }
}

fn ordinal(marker: &str) -> Result<usize, String> {
    let suffix = marker.strip_prefix("StagedSchema").ok_or(SCHEDULE)?;
    let number = suffix
        .parse::<usize>()
        .map_err(|error| format!("{SCHEDULE}: {error}"))?;
    if suffix != number.to_string() {
        return Err(SCHEDULE.into());
    }
    Ok(number)
}

fn owner_programs(view: &View) -> Result<BTreeMap<String, Vec<(String, String)>>, String> {
    let refusal = "undeclared program substitution";
    if view.programs.first() != Some(&("shared::hostname".into(), HOSTNAME_SOURCE.into())) {
        return Err(refusal.into());
    }
    let mut result: BTreeMap<_, Vec<_>> = view
        .owners
        .keys()
        .map(|owner| (owner.clone(), Vec::new()))
        .collect();
    let mut symbols = BTreeSet::new();
    let mut markers = BTreeMap::new();
    let mut owner_markers = BTreeMap::new();
    for (identity, pattern) in view.programs.iter().skip(1) {
        let parts = identity.split("::").collect::<Vec<_>>();
        let [owner, marker, symbol] = parts.as_slice() else {
            return Err(refusal.into());
        };
        if owner.is_empty()
            || symbol.is_empty()
            || ordinal(marker).is_err()
            || !symbols.insert((*owner, *symbol))
            || markers
                .insert(*marker, *owner)
                .is_some_and(|prior| prior != *owner)
            || owner_markers
                .insert(*owner, *marker)
                .is_some_and(|prior| prior != *marker)
        {
            return Err(refusal.into());
        }
        result
            .get_mut(*owner)
            .ok_or(refusal)?
            .push(((*symbol).into(), pattern.clone()));
    }
    Ok(result)
}

fn match_views(reference: &View, candidate: &View, declared: &Delta) -> Result<Value, String> {
    if reference.source["kind"] != "pinned-git-object-data"
        || reference.source["commit"] != catalog_reference::REFERENCE_COMMIT
        || reference.source["tree"] != catalog_reference::REFERENCE_TREE
    {
        return Err("reference identity mismatch".into());
    }
    if reference.measurement != candidate.measurement {
        return Err("measurement context mismatch".into());
    }
    if owner_delta(&reference.owners, &candidate.owners) != *declared {
        return Err("undeclared owner delta".into());
    }
    let reference_programs = owner_programs(reference)?;
    let candidate_programs = owner_programs(candidate)?;
    for owner in reference
        .owners
        .keys()
        .filter(|owner| candidate.owners.contains_key(*owner) && !declared.changed.contains(*owner))
    {
        if reference_programs.get(owner) != candidate_programs.get(owner) {
            return Err("undeclared program substitution".into());
        }
    }
    Ok(json!({"reference":reference,"candidate":candidate,"declared":declared}))
}

fn verify_frozen(token: &Value, reference: &View, candidate: &View) -> Result<(), String> {
    if token["reference"] != serde_json::to_value(reference).map_err(|error| error.to_string())?
        || token["candidate"]
            != serde_json::to_value(candidate).map_err(|error| error.to_string())?
    {
        return Err("frozen inputs changed".into());
    }
    Ok(())
}

fn declaration_schedule(row: &Value) -> Result<Vec<(String, String)>, String> {
    let owner = row["owner"].as_str().ok_or(SCHEDULE)?;
    let marker = row["marker"].as_str().ok_or(SCHEDULE)?;
    let mut programs = BTreeMap::new();
    for program in row["programs"].as_array().ok_or(SCHEDULE)? {
        let symbol = program["symbol"]
            .as_str()
            .filter(|name| !name.is_empty())
            .ok_or(SCHEDULE)?;
        let pattern = program["pattern"].as_str().ok_or(SCHEDULE)?;
        if program["identity"] != format!("{owner}::{marker}::{symbol}")
            || programs.insert(symbol, pattern).is_some()
        {
            return Err(SCHEDULE.into());
        }
    }
    let mut result = Vec::new();
    for entry in row["initialization"].as_array().ok_or(SCHEDULE)? {
        let symbol = entry.as_str().ok_or(SCHEDULE)?;
        let pattern = programs.remove(symbol).ok_or(SCHEDULE)?;
        result.push((format!("{owner}::{marker}::{symbol}"), pattern.into()));
    }
    if !programs.is_empty() {
        return Err(SCHEDULE.into());
    }
    Ok(result)
}

fn schedule(
    graph: &Value,
    owners: &BTreeMap<String, String>,
) -> Result<Vec<(String, String)>, String> {
    let mut declarations = BTreeMap::new();
    let mut seen_owners = BTreeSet::new();
    for row in graph["declarations"].as_array().ok_or(SCHEDULE)? {
        let owner = row["owner"].as_str().ok_or(SCHEDULE)?;
        let marker = row["marker"].as_str().ok_or(SCHEDULE)?;
        let number = ordinal(marker)?;
        if !owners.contains_key(owner)
            || !seen_owners.insert(owner)
            || number >= owners.len()
            || declarations.insert(marker, row).is_some()
        {
            return Err(SCHEDULE.into());
        }
    }
    if declarations.len() != owners.len() {
        return Err(SCHEDULE.into());
    }
    let mut result = vec![("shared::hostname".into(), HOSTNAME_SOURCE.into())];
    let mut initialized = BTreeSet::new();
    let mut edges = BTreeSet::new();
    let mut auxiliary_edges = 0_usize;
    for edge in graph["auxiliary"]["dispatch_edges"]
        .as_array()
        .ok_or(SCHEDULE)?
    {
        let from = edge["from"].as_str().ok_or(SCHEDULE)?;
        let to = edge["to"].as_str().ok_or(SCHEDULE)?;
        let role = edge["role"].as_str().ok_or(SCHEDULE)?;
        if edge["ownership"] != "callee-owned" || !edges.insert((from, to, role)) {
            return Err(SCHEDULE.into());
        }
        match (from, role) {
            ("initialize_native", "initialize-regexes") => {
                let declaration = declarations.get(to).ok_or(SCHEDULE)?;
                if ordinal(to)? != initialized.len() || !initialized.insert(to) {
                    return Err(SCHEDULE.into());
                }
                result.extend(declaration_schedule(declaration)?);
            }
            ("retain_for_inspection", "inspection-reference") => {
                let suffix = to.strip_prefix("STAGED_VALIDATOR_").ok_or(SCHEDULE)?;
                let number = suffix.parse::<usize>().map_err(|error| error.to_string())?;
                if suffix != number.to_string() || number >= owners.len() {
                    return Err(SCHEDULE.into());
                }
                auxiliary_edges += 1;
            }
            ("validate_native", "validation-call-cache-factory" | "is-valid-with-call-cache") => {
                if !declarations.contains_key(to) {
                    return Err(SCHEDULE.into());
                }
                auxiliary_edges += 1;
            }
            _ => return Err(SCHEDULE.into()),
        }
    }
    if initialized.len() != declarations.len()
        || (auxiliary_edges != 0
            && auxiliary_edges != owners.len().checked_mul(3).ok_or(SCHEDULE)?)
    {
        return Err(SCHEDULE.into());
    }
    Ok(result)
}

fn string_set(value: &Value, refusal: &str) -> Result<BTreeSet<String>, String> {
    let entries = value.as_array().ok_or(refusal)?;
    let mut result = BTreeSet::new();
    for entry in entries {
        if !result.insert(
            entry
                .as_str()
                .filter(|text| !text.is_empty())
                .ok_or(refusal)?
                .to_owned(),
        ) {
            return Err(refusal.into());
        }
    }
    Ok(result)
}

fn membership(seed: &Value, independent: &Value) -> Result<(), String> {
    let refusal = "independent membership mismatch";
    for key in [
        "implemented_roots_all_features",
        "inventory_dispositions",
        "callable_bindings",
    ] {
        if seed.get(key).is_none()
            || seed[key].as_array().is_none()
            || seed[key] != independent[key]
        {
            return Err(refusal.into());
        }
    }
    let roots = string_set(&independent["roots"], refusal)?;
    let all_roots = string_set(&independent["implemented_roots_all_features"], refusal)?;
    let mut members = Vec::new();
    let mut selected_roots = BTreeSet::new();
    let mut names = BTreeSet::new();
    for member in independent["members"].as_array().ok_or(refusal)? {
        let name = member["name"].as_str().ok_or(refusal)?;
        if !names.insert(name)
            || member["implemented"].as_bool() != Some(all_roots.contains(name))
            || member["is_root"].as_bool() != Some(roots.contains(name))
        {
            return Err(refusal.into());
        }
        if roots.contains(name) {
            selected_roots.insert(name.to_owned());
        }
        members.push(
            json!({"name":member["name"],"implementation":member["implementation"],
            "is_root":member["is_root"],"ancestry":member["ancestry"],
            "schema_owner":member["effective_schema_owner"]}),
        );
    }
    string_set(&independent["effective_schema_owners"], refusal)?;
    if selected_roots != roots
        || seed["selection"]["members"] != json!(members)
        || seed["selection"]["schema_owners"] != independent["effective_schema_owners"]
    {
        return Err(refusal.into());
    }
    Ok(())
}

fn signed_delta(reference: u64, candidate: u64) -> Result<i64, String> {
    i64::try_from(i128::from(candidate) - i128::from(reference))
        .map_err(|error| format!("growth delta overflow: {error}"))
}

fn view_report(view: &View, observation: Value) -> Value {
    json!({"source":view.source,"measurement":view.measurement,"membership":view.membership,
        "schema_owner_count":view.owners.len(),
        "public_definition_count":view.membership["roots"].as_array().map(Vec::len),
        "schema_owners":view.owners,"programs":view.programs,"program_count":view.programs.len(),
        "seed_sha256":view.seed_sha256,"expanded_sha256":view.expanded_sha256,
        "generated_source_bytes":view.generated_source_bytes,
        "constructor_observation":observation,
        "metadata_constructor_observation":null,"registry_constructor_observation":null})
}

fn observation_delta(reference: &Value, candidate: &Value) -> Result<Value, String> {
    let mut result = Map::new();
    for phase in [
        "cold",
        "repeat",
        "cleanup",
        "late_size_limit_failure",
        "prefix_plus_failure",
    ] {
        let mut metrics = Map::new();
        for field in [
            "allocated_bytes",
            "peak_live_bytes",
            "retained_bytes",
            "allocation_calls",
        ] {
            let left = reference[phase].get(field);
            let right = candidate[phase].get(field);
            let difference = match (left, right) {
                (None, None) => Value::Null,
                (Some(before), Some(after)) => {
                    let before_number = before
                        .as_i64()
                        .map(i128::from)
                        .or_else(|| before.as_u64().map(i128::from))
                        .ok_or("invalid observation")?;
                    let after_number = after
                        .as_i64()
                        .map(i128::from)
                        .or_else(|| after.as_u64().map(i128::from))
                        .ok_or("invalid observation")?;
                    json!(
                        i64::try_from(after_number - before_number)
                            .map_err(|error| error.to_string())?
                    )
                }
                _ => return Err("unmatched observation components".into()),
            };
            metrics.insert(field.into(), difference);
        }
        result.insert(phase.into(), Value::Object(metrics));
    }
    Ok(Value::Object(result))
}

fn observe_matched(
    reference: &View,
    candidate: &View,
    declared: &Delta,
    mut observe: impl FnMut(&[(String, String)]) -> Result<Value, String>,
) -> Result<Value, String> {
    let token = match_views(reference, candidate, declared)?;
    let reference_observation = observe(&reference.programs)?;
    let candidate_observation = observe(&candidate.programs)?;
    verify_frozen(&token, reference, candidate)?;
    let constructor_delta = observation_delta(&reference_observation, &candidate_observation)?;
    Ok(json!({"label":LABEL,"owner_delta":declared,
        "reference":view_report(reference, reference_observation),
        "candidate":view_report(candidate, candidate_observation),
        "component_deltas":{
            "generated_source_bytes":signed_delta(reference.generated_source_bytes,
                candidate.generated_source_bytes)?,
            "requested_heap_constructor_observation":constructor_delta,
            "metadata":null,"registry":null,"common_C_I":null},
        "status":INCOMPLETE,"incomplete":["matched metadata constructors",
            "matched registry constructors","common C/I qualification"],
        "observation_scope":"fresh reconstructed Regex/LazyLock requested heap, not generated singletons",
        "common_C_I_qualified":false,"task8_complete":false,"physical_or_serving_qualification":false}))
}

fn native_format_first(source: &[u8]) -> Result<(), String> {
    if runtime_sources::initialization_order(source).is_err() {
        return Err("native format initialization order mismatch".into());
    }
    Ok(())
}

fn regular_metadata(path: &Path) -> Result<fs::Metadata, String> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
        || path.components().collect::<PathBuf>().as_os_str() != path.as_os_str()
    {
        return Err("non-normal growth source path".into());
    }
    let mut ancestor = PathBuf::new();
    for component in path.components() {
        ancestor.push(component);
        let metadata = fs::symlink_metadata(&ancestor).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() || (!metadata.is_file() && !metadata.is_dir()) {
            return Err("nonregular growth source path".into());
        }
    }
    fs::symlink_metadata(path).map_err(|error| error.to_string())
}

fn read_regular(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = regular_metadata(path)?;
    if !metadata.is_file() {
        return Err("growth source is not a file".into());
    }
    let cap = u64::try_from(INPUT_BYTES).map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(cap + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let length = u64::try_from(bytes.len()).map_err(|error| error.to_string())?;
    if length > cap || length != metadata.len() {
        return Err("growth source input bound or length mismatch".into());
    }
    Ok(bytes)
}

fn file_witness(bytes: &[u8]) -> Value {
    json!({"bytes":bytes.len(),"sha256":expanded::digest(bytes)})
}

fn source_files(
    workspace: &Path,
    paths: &BTreeSet<PathBuf>,
    snapshot_paths: &BTreeSet<PathBuf>,
) -> Result<Value, String> {
    if !regular_metadata(workspace)?.is_dir() {
        return Err("growth workspace is not a directory".into());
    }
    let mut files = Map::new();
    for path in paths {
        // Only an exact generated-snapshot path may be outside this source root.
        // CapturedSeed checks its bytes/containment separately, before and after.
        if snapshot_paths.contains(path) {
            continue;
        }
        let Ok(relative) = path.strip_prefix(workspace) else {
            return Err("unexpected source path outside workspace".into());
        };
        let name = relative
            .to_str()
            .ok_or("non UTF-8 growth source path")?
            .to_owned();
        let metadata = regular_metadata(path)?;
        let record = if metadata.is_dir() {
            let mut entries = BTreeSet::new();
            for child in fs::read_dir(path).map_err(|error| error.to_string())? {
                let child_entry = child.map_err(|error| error.to_string())?;
                entries.insert(
                    child_entry
                        .file_name()
                        .into_string()
                        .map_err(|name| format!("non UTF-8 growth directory entry: {name:?}"))?,
                );
            }
            json!({"kind":"directory","entries":entries})
        } else {
            file_witness(&read_regular(path)?)
        };
        if files.insert(name, record).is_some() {
            return Err("duplicate growth source path".into());
        }
    }
    Ok(Value::Object(files))
}

fn compiled_sources(workspace: &Path) -> Result<Value, String> {
    let mut sources = Map::new();
    for &(relative, compiled) in MEASUREMENT_SOURCES {
        if read_regular(&workspace.join(relative))? != compiled {
            return Err(format!(
                "measurement source differs from compiled bytes: {relative}"
            ));
        }
        if sources
            .insert(relative.into(), file_witness(compiled))
            .is_some()
        {
            return Err("duplicate measurement source witness".into());
        }
    }
    for (relative, witness) in descriptor::model_sources().as_object()
        .ok_or("missing component model witnesses")?
    {
        if file_witness(&read_regular(&workspace.join(relative))?) != *witness {
            return Err(format!("component model differs from compiled bytes: {relative}"));
        }
        if let Some(prior) = sources.insert(relative.clone(), witness.clone())
            && prior != *witness
        {
            return Err("conflicting component model witness".into());
        }
    }
    Ok(Value::Object(sources))
}

fn recheck_build_outputs() -> Result<(), String> {
    let out_dir = Path::new(env!("OUT_DIR"));
    for (name, compiled) in [
        ("schema-staging.json", CURRENT_MANIFEST),
        ("schema-staging.rs", CURRENT_RUST),
        ("schema-layout-build.toml", CURRENT_LAYOUT),
    ] {
        if read_regular(&out_dir.join(name))? != compiled {
            return Err(format!(
                "current build output differs from compiled bytes: {name}"
            ));
        }
    }
    Ok(())
}

#[derive(Deserialize, Serialize)]
struct Build {
    target: String,
    profile: String,
    enabled_features: Vec<String>,
    compiler_verbose: String,
    encoded_rustflags: String,
}

struct Measurement {
    manifest: Value,
    build: Build,
    environment: BTreeMap<String, String>,
    identity: Value,
}

fn check_features(build: &Build, environment: &BTreeMap<String, String>) -> Result<(), String> {
    let compiled = [
        ("insecure-bidders", cfg!(feature = "insecure-bidders")),
        (
            "schema-artifact-staging",
            cfg!(feature = "schema-artifact-staging"),
        ),
        ("test-support", cfg!(feature = "test-support")),
    ]
    .into_iter()
    .filter_map(|(feature, enabled)| enabled.then_some(feature))
    .collect::<Vec<_>>();
    let enabled = build
        .enabled_features
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let expected_environment = compiled
        .iter()
        .map(|feature| {
            (
                format!(
                    "CARGO_FEATURE_{}",
                    feature.replace('-', "_").to_ascii_uppercase()
                ),
                "1".to_owned(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let actual_environment = environment
        .iter()
        .filter(|(name, _value)| name.starts_with("CARGO_FEATURE_"))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    if enabled != compiled
        || actual_environment != expected_environment
        || environment.get("CARGO_CFG_FEATURE").map(String::as_str)
            != Some(compiled.join(",").as_str())
    {
        return Err("current build feature mismatch".into());
    }
    Ok(())
}

fn check_build(
    manifest: &Value,
    build: &Build,
    environment: &BTreeMap<String, String>,
) -> Result<Value, String> {
    let layout: BTreeMap<String, String> =
        toml::from_str(str::from_utf8(CURRENT_LAYOUT).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let endian = if cfg!(target_endian = "little") {
        "little"
    } else {
        "big"
    };
    let width = usize::BITS.to_string();
    for (field, key, expected) in [
        ("arch", "CARGO_CFG_TARGET_ARCH", consts::ARCH),
        (
            "pointer_width",
            "CARGO_CFG_TARGET_POINTER_WIDTH",
            width.as_str(),
        ),
        ("endian", "CARGO_CFG_TARGET_ENDIAN", endian),
    ] {
        if layout.get(field).map(String::as_str) != Some(expected)
            || environment.get(key).map(String::as_str) != Some(expected)
        {
            return Err("current build ABI mismatch".into());
        }
    }
    let out_dir = Path::new(env!("OUT_DIR"));
    let profile = out_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .ok_or("missing build profile")?;
    if manifest["schema_version"] != 2_u8
        || manifest["kind"] != "unqualified_non_serving_schema_staging"
        || manifest["identity"]["qualification"] != "Unqualified"
        || manifest["linked_or_serving_qualification"] != false
        || manifest["options"] != CANDIDATE_OPTIONS
        || build.profile != profile
        || layout.get("target") != Some(&build.target)
        || layout.get("compiler") != Some(&build.compiler_verbose)
        || environment.get("TARGET") != Some(&build.target)
        || environment.get("OUT_DIR").map(Path::new) != Some(out_dir)
        || environment
            .get("CARGO_ENCODED_RUSTFLAGS")
            .map_or("", String::as_str)
            != build.encoded_rustflags
        || build.compiler_verbose.trim().is_empty()
    {
        return Err("current staging build identity mismatch".into());
    }
    for key in ["TARGET", "HOST", "PROFILE", "OPT_LEVEL", "DEBUG", "RUSTC"] {
        if environment.get(key).is_none_or(String::is_empty) {
            return Err(format!("missing current build environment: {key}"));
        }
    }
    check_features(build, environment)?;
    Ok(
        json!({"arch":consts::ARCH,"pointer_width":width,"endian":endian,
        "os":consts::OS,"debug_assertions":cfg!(debug_assertions)}),
    )
}

fn measurement_identity(
    workspace: &Path,
    manifest: &Value,
    build: &Build,
    environment: &BTreeMap<String, String>,
) -> Result<Value, String> {
    recheck_build_outputs()?;
    let abi = check_build(manifest, build, environment)?;
    let sources = compiled_sources(workspace)?;
    let compiler = schema_vendor::verify(workspace)?;
    let regex = schema_regex_vendor::verify(workspace).map_err(|error| error.to_string())?;
    let features = schema_build::resolved_engine_features(&regex)?;
    let compiler_inputs = manifest["identity"]["inputs"]
        .as_array()
        .ok_or("missing compiler inputs")?
        .iter()
        .filter(|input| {
            input["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("vendor/jsonschema-macros-core/"))
        })
        .collect::<Vec<_>>();
    if compiler_inputs.is_empty() {
        return Err("missing current compiler source identity".into());
    }
    Ok(json!({"build":build,"environment":environment,"abi":abi,
        "measurement_lock_sha256":expanded::digest(&read_regular(&workspace.join("Cargo.lock"))?),
        "measurement_sources":sources,"regex_source":regex,"regex_features":features,
        "compiler_source":{"root":compiler,"inputs":compiler_inputs},
        "generator_options":CANDIDATE_OPTIONS,
        "build_outputs":{"schema-staging.json":file_witness(CURRENT_MANIFEST),
            "schema-staging.rs":file_witness(CURRENT_RUST),
            "schema-layout-build.toml":file_witness(CURRENT_LAYOUT)},
        "workload":"observations::observe: fresh per-owner LazyLock<Regex>, cold/repeat/prefix/late-size-limit-failure/release",
        "non_catalog":{"metadata_registry":"not yet observed","common_C_I":"not qualified"}}))
}

fn current_measurement(workspace: &Path) -> Result<Measurement, String> {
    let manifest: Value =
        serde_json::from_slice(CURRENT_MANIFEST).map_err(|error| error.to_string())?;
    let build: Build =
        serde_json::from_value(manifest["build"].clone()).map_err(|error| error.to_string())?;
    let environment = serde_json::from_value(manifest["build_environment"].clone())
        .map_err(|error| error.to_string())?;
    let identity = measurement_identity(workspace, &manifest, &build, &environment)?;
    Ok(Measurement {
        manifest,
        build,
        environment,
        identity,
    })
}

fn declarations(discovery: &Discovery) -> Result<Vec<BidderMetadataDeclaration>, String> {
    let rows = discovery
        .manifests()
        .values()
        .flat_map(|manifest| {
            manifest.definitions.keys().map(move |name| {
                (
                    name.as_str(),
                    manifest.key.as_str(),
                    manifest.required_feature.as_deref(),
                )
            })
        })
        .collect::<Vec<_>>();
    intern_rows(&rows)
}

fn generate_current(
    crate_root: &Path,
    out_dir: &Path,
    discovery: &Discovery,
    metadata: &[BidderMetadataDeclaration],
    measurement: &Measurement,
) -> Result<GeneratedStaging, String> {
    let mut environment = measurement.environment.clone();
    environment.insert(
        "OUT_DIR".into(),
        out_dir.to_str().ok_or("non UTF-8 output path")?.into(),
    );
    let features = measurement
        .build
        .enabled_features
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    generate(BuildRequest {
        crate_dir: crate_root,
        out_dir,
        discovery,
        metadata,
        context: BuildContext {
            target: &measurement.build.target,
            profile: &measurement.build.profile,
            enabled_features: &features,
            compiler_verbose: &measurement.build.compiler_verbose,
            encoded_rustflags: &measurement.build.encoded_rustflags,
        },
        build_environment: &environment,
    })
}

fn check_current_artifact(crate_root: &Path, measurement: &Measurement) -> Result<(), String> {
    let discovery = bidder_discovery::scan(crate_root)?;
    let metadata = declarations(&discovery)?;
    let published = auctioneer_config_data::bidder_metadata_declarations();
    let declared_set = metadata
        .iter()
        .map(|row| (row.bidder, row.implementation, row.required_feature))
        .collect::<BTreeSet<_>>();
    let published_set = published
        .iter()
        .map(|row| (row.bidder, row.implementation, row.required_feature))
        .collect::<BTreeSet<_>>();
    if declared_set != published_set
        || declared_set.len() != metadata.len()
        || published_set.len() != published.len()
    {
        return Err("current compiled metadata declarations mismatch".into());
    }
    let out_dir = Path::new(env!("OUT_DIR"));
    let generated = generate_current(crate_root, out_dir, &discovery, &metadata, measurement)?;
    if generated.manifest_json.as_bytes() != CURRENT_MANIFEST
        || generated.rust.to_string().as_bytes() != CURRENT_RUST
    {
        return Err("current build artifact does not match current writer/source".into());
    }
    let seed = CapturedSeed {
        identity_sha256: generated.identity.sha256,
        manifest_bytes: CURRENT_MANIFEST.to_vec(),
        rust: CURRENT_RUST.to_vec(),
        build_root: out_dir.to_owned(),
        out_dir: out_dir.to_owned(),
        snapshots: generated.snapshots,
    };
    seed.recheck_snapshots()
}

fn rows_by(value: &Value, key: &str) -> Result<BTreeMap<String, Value>, String> {
    let mut rows = BTreeMap::new();
    for row in value.as_array().ok_or(SCHEMAS)? {
        let name = row[key]
            .as_str()
            .filter(|text| !text.is_empty())
            .ok_or(SCHEMAS)?;
        if rows.insert(name.to_owned(), row.clone()).is_some() {
            return Err(SCHEMAS.into());
        }
    }
    Ok(rows)
}

fn raw_schema_witnesses(
    crate_root: &Path,
    discovery: &Discovery,
    selected: &BTreeSet<String>,
) -> Result<BTreeMap<String, Value>, String> {
    let mut witnesses = BTreeMap::new();
    for manifest in discovery.manifests().values() {
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
            let owner = Path::new(&artifact.source_path)
                .file_stem()
                .and_then(|name| name.to_str())
                .ok_or(SCHEMAS)?;
            if !selected.contains(owner) {
                continue;
            }
            let retained = artifact.retained.as_ref().ok_or(SCHEMAS)?;
            let origin = sources.get(&artifact.source).ok_or(SCHEMAS)?;
            let path = format!("{}/{}", manifest.key, retained.path);
            let bytes = read_regular(&crate_root.join("src/bidders").join(&path))?;
            if artifact.source_path != format!("static/bidder-params/{owner}.json")
                || u64::try_from(bytes.len()).map_err(|error| error.to_string())? != retained.bytes
                || expanded::digest(&bytes) != retained.sha256
            {
                return Err(SCHEMAS.into());
            }
            let witness = json!({"name":owner,"path":path,"source_path":artifact.source_path,
                "source_blob":artifact.source_blob,"bytes":retained.bytes,"sha256":retained.sha256,
                "repository":origin.repository,"revision":origin.revision});
            if witnesses.insert(owner.to_owned(), witness).is_some() {
                return Err(SCHEMAS.into());
            }
        }
    }
    if witnesses.keys().cloned().collect::<BTreeSet<_>>() != *selected {
        return Err(SCHEMAS.into());
    }
    Ok(witnesses)
}

fn schema_owners(
    crate_root: &Path,
    discovery: &Discovery,
    independent: &Value,
    manifest: &Value,
    seed: &CapturedSeed,
    graph: &Value,
) -> Result<BTreeMap<String, String>, String> {
    let selected = string_set(&independent["effective_schema_owners"], SCHEMAS)?;
    let witnesses = raw_schema_witnesses(crate_root, discovery, &selected)?;
    let source_rows = rows_by(&manifest["schemas"], "name")?;
    let declaration_rows = rows_by(&manifest["declarations"], "owner")?;
    let expanded_rows = rows_by(&graph["declarations"], "owner")?;
    let snapshot_value =
        serde_json::to_value(&seed.snapshots).map_err(|error| error.to_string())?;
    let snapshot_rows = rows_by(&snapshot_value, "owner")?;
    let manifest_snapshots = rows_by(&manifest["compiled_schema_snapshots"], "owner")?;
    for rows in [
        &source_rows,
        &declaration_rows,
        &expanded_rows,
        &snapshot_rows,
    ] {
        if rows.keys().cloned().collect::<BTreeSet<_>>() != selected {
            return Err(SCHEMAS.into());
        }
    }
    if source_rows != witnesses || snapshot_rows != manifest_snapshots {
        return Err(SCHEMAS.into());
    }
    let mut schema_inputs = BTreeMap::new();
    for input in manifest["identity"]["inputs"].as_array().ok_or(SCHEMAS)? {
        if input["kind"] == "Schema" {
            let name = input["id"].as_str().ok_or(SCHEMAS)?;
            let sha256 = input["sha256"].as_str().ok_or(SCHEMAS)?;
            if schema_inputs
                .insert(name.to_owned(), sha256.to_owned())
                .is_some()
            {
                return Err(SCHEMAS.into());
            }
        }
    }
    let mut result = BTreeMap::new();
    for (owner, witness) in &witnesses {
        let declaration = declaration_rows.get(owner).ok_or(SCHEMAS)?;
        let expanded = expanded_rows.get(owner).ok_or(SCHEMAS)?;
        let snapshot = snapshot_rows.get(owner).ok_or(SCHEMAS)?;
        if declaration["source"] != *witness
            || expanded["source"] != *witness
            || declaration["marker"] != expanded["marker"]
            || declaration["snapshot"] != *snapshot
            || expanded["snapshot"] != *snapshot
            || snapshot["sha256"] != witness["sha256"]
        {
            return Err(SCHEMAS.into());
        }
        result.insert(
            owner.clone(),
            witness["sha256"].as_str().ok_or(SCHEMAS)?.to_owned(),
        );
    }
    if result != schema_inputs {
        return Err(SCHEMAS.into());
    }
    Ok(result)
}

struct Capture {
    workspace: PathBuf,
    paths: BTreeSet<PathBuf>,
    snapshot_paths: BTreeSet<PathBuf>,
    seed: CapturedSeed,
    graph: Value,
    view: View,
}

fn private_directory(parent: &Path, name: &str) -> Result<PathBuf, String> {
    let path = parent.join(name);
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&path)
        .map_err(|error| error.to_string())?;
    Ok(path)
}

fn capture_seed(
    out_dir: &Path,
    generated: GeneratedStaging,
) -> Result<(CapturedSeed, BTreeSet<PathBuf>), String> {
    write_snapshots(out_dir, &generated.snapshots)?;
    let manifest_bytes = generated.manifest_json.into_bytes();
    let rust = generated.rust.to_string().into_bytes();
    fs::write(out_dir.join("schema-staging.json"), &manifest_bytes)
        .map_err(|error| error.to_string())?;
    fs::write(out_dir.join("schema-staging.rs"), &rust).map_err(|error| error.to_string())?;
    let seed = CapturedSeed {
        identity_sha256: generated.identity.sha256,
        manifest_bytes,
        rust,
        build_root: out_dir.to_owned(),
        out_dir: out_dir.to_owned(),
        snapshots: generated.snapshots,
    };
    seed.recheck_snapshots()?;
    Ok((seed, generated.rerun_paths))
}

fn reconstruct(
    workspace: &Path,
    out_dir: &Path,
    measurement: &Measurement,
    mut source: Value,
    extra_paths: &BTreeSet<PathBuf>,
) -> Result<Capture, String> {
    let crate_root = workspace.join("crates/auctioneer-bidder");
    let discovery = bidder_discovery::scan(&crate_root)?;
    let metadata = declarations(&discovery)?;
    let generated = generate_current(&crate_root, out_dir, &discovery, &metadata, measurement)?;
    let (seed, mut paths) = capture_seed(out_dir, generated)?;
    paths.extend(extra_paths.iter().cloned());
    let snapshot_paths = seed
        .snapshots
        .iter()
        .map(|snapshot| out_dir.join(&snapshot.relative_path))
        .collect::<BTreeSet<_>>();
    let manifest: Value =
        serde_json::from_slice(&seed.manifest_bytes).map_err(|error| error.to_string())?;
    let lane = if cfg!(feature = "insecure-bidders") {
        Lane::Development
    } else {
        Lane::Secure
    };
    let independent = independent_source_membership(&crate_root, lane)?;
    membership(&manifest, &independent)?;
    let graph = expanded::expand(&seed)?;
    let owners = schema_owners(
        &crate_root,
        &discovery,
        &independent,
        &manifest,
        &seed,
        &graph,
    )?;
    let programs = schedule(&graph, &owners)?;
    source["files"] = source_files(workspace, &paths, &snapshot_paths)?;
    source["lock_sha256"] = json!(expanded::digest(&read_regular(
        &workspace.join("Cargo.lock")
    )?));
    let view = View {
        source,
        measurement: measurement.identity.clone(),
        membership: independent,
        owners,
        programs,
        seed_sha256: expanded::digest(&seed.manifest_bytes),
        expanded_sha256: expanded::digest(
            &serde_json::to_vec(&graph).map_err(|error| error.to_string())?,
        ),
        generated_source_bytes: u64::try_from(seed.rust.len())
            .map_err(|error| error.to_string())?,
    };
    Ok(Capture {
        workspace: workspace.to_owned(),
        paths,
        snapshot_paths,
        seed,
        graph,
        view,
    })
}

fn reference_identity(snapshot: &catalog_reference::SourceSnapshot) -> Value {
    let witnesses = snapshot
        .witnesses()
        .map(|witness| (&witness.path, witness))
        .collect::<BTreeMap<_, _>>();
    json!({"kind":"pinned-git-object-data","commit":snapshot.commit(),
        "tree":snapshot.tree(),"git_files":witnesses})
}

fn check_materialized_reference(
    snapshot: &catalog_reference::SourceSnapshot,
    capture: &Capture,
) -> Result<(), String> {
    for file in snapshot.files() {
        let record = &capture.view.source["files"][&file.witness.path];
        if record["bytes"] != file.witness.bytes
            || record["sha256"] != file.witness.sha256
            || file_witness(&file.contents) != *record
        {
            return Err("materialized reference differs from pinned Git data".into());
        }
    }
    Ok(())
}

fn rechecked_view(capture: &Capture, measurement: &Value) -> Result<View, String> {
    capture.seed.recheck_snapshots()?;
    if read_regular(&capture.seed.out_dir.join("schema-staging.json"))?
        != capture.seed.manifest_bytes
        || read_regular(&capture.seed.out_dir.join("schema-staging.rs"))? != capture.seed.rust
    {
        return Err("frozen inputs changed".into());
    }
    let mut view = capture.view.clone();
    view.source["files"] =
        source_files(&capture.workspace, &capture.paths, &capture.snapshot_paths)?;
    view.source["lock_sha256"] = json!(expanded::digest(&read_regular(
        &capture.workspace.join("Cargo.lock")
    )?));
    view.measurement = measurement.clone();
    Ok(view)
}

fn run_real() -> Result<Value, String> {
    let reference_root = reference_path(env::var_os("AUCTIONEER__TEST__CATALOG_REFERENCE_DIR"))?;
    let reference_data = catalog_reference::read(&reference_root)
        .map_err(|error| format!("catalog reference refused: {error:?}"))?;
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let workspace = crate_root
        .parent()
        .and_then(Path::parent)
        .ok_or("missing growth workspace")?;
    native_format_first(NATIVE_STAGING)?;
    let measurement = current_measurement(workspace)?;
    check_current_artifact(&crate_root, &measurement)?;
    let temporary = tempfile::tempdir().map_err(|error| error.to_string())?;
    let private_root = temporary
        .path()
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if regular_metadata(&private_root)?.permissions().mode() & 0o777 != 0o700 {
        return Err("catalog reconstruction requires private tempfile0700".into());
    }
    let reference_source = private_root.join("source");
    reference_data
        .materialize(&reference_source)
        .map_err(|error| format!("catalog reference materialization refused: {error:?}"))?;
    let reference_output = private_directory(&private_root, "reference-out")?;
    let candidate_output = private_directory(&private_root, "candidate-out")?;
    let reference_paths = reference_data
        .witnesses()
        .map(|witness| reference_source.join(&witness.path))
        .collect::<BTreeSet<_>>();
    let reference = reconstruct(
        &reference_source,
        &reference_output,
        &measurement,
        reference_identity(&reference_data),
        &reference_paths,
    )?;
    check_materialized_reference(&reference_data, &reference)?;
    let candidate = reconstruct(
        workspace,
        &candidate_output,
        &measurement,
        json!({"kind":"current-source-content-snapshot"}),
        &BTreeSet::new(),
    )?;
    // Independently reviewed declaration, never inferred from the measured pair.
    let reviewed_delta = Delta::default();
    let token = match_views(&reference.view, &candidate.view, &reviewed_delta)?;
    let mut report = observe_matched(
        &reference.view,
        &candidate.view,
        &reviewed_delta,
        observations::observe,
    )?;
    let after = measurement_identity(
        workspace,
        &measurement.manifest,
        &measurement.build,
        &measurement.environment,
    )?;
    check_current_artifact(&crate_root, &measurement)?;
    let reference_after = rechecked_view(&reference, &after)?;
    let candidate_after = rechecked_view(&candidate, &after)?;
    verify_frozen(&token, &reference_after, &candidate_after)?;
    check_materialized_reference(&reference_data, &reference)?;
    report["reference"]["expanded_owner_graph"] = reference.graph;
    report["candidate"]["expanded_owner_graph"] = candidate.graph;
    report["current_build_identity"] = measurement.manifest["identity"].clone();
    report["frozen_sources_and_snapshots_rechecked"] = json!(true);
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use auctioneer_config::schema_staging_build::catalog_reference::{
        REFERENCE_COMMIT, REFERENCE_TREE,
    };
    use quote::ToTokens as _;
    use serde_json::json;
    use std::cell::Cell;
    use std::{fs, str};

    fn view(reference: bool) -> View {
        View {
            source: if reference {
                json!({"kind":"pinned-git-object-data","commit":REFERENCE_COMMIT,"tree":REFERENCE_TREE,
                    "lock_sha256":"historical-lock","files":{"Cargo.lock":"historical-lock"}})
            } else {
                json!({"kind":"current-source-content-snapshot","lock_sha256":"candidate-lock",
                    "files":{"Cargo.lock":"candidate-lock"}})
            },
            measurement: json!({
                "build":{"target":"fixture-target","profile":"fixture-profile",
                    "compiler_verbose":"fixture compiler","encoded_rustflags":"",
                    "enabled_features":["fixture-feature"]},
                "environment":{"TARGET":"fixture-target","OPT_LEVEL":"0","DEBUG":"true"},
                "abi":{"arch":"fixture-arch","pointer_width":"64","endian":"little"},
                "measurement_lock_sha256":"measurement-lock",
                "measurement_sources":{"fake/current-generator.rs":"generator-digest"},
                "regex_source":"fixture-reviewed-source","regex_features":["meta","syntax"],
                "generator_options":"fixture-options","workload":"fixture-workload",
                "non_catalog":"fixture-unchanged-non-catalog-scope"
            }),
            membership: json!({"roots":["fake_a"],"effective_schema_owners":["fake_a"]}),
            owners: BTreeMap::from([("fake_a".into(), "schema-a".into())]),
            programs: vec![
                ("shared::hostname".into(), HOSTNAME_SOURCE.into()),
                ("fake_a::StagedSchema0::Regex0".into(), "^x$".into()),
            ],
            seed_sha256: if reference {
                "reference-seed"
            } else {
                "candidate-seed"
            }
            .into(),
            expanded_sha256: if reference {
                "reference-expanded"
            } else {
                "candidate-expanded"
            }
            .into(),
            generated_source_bytes: 19,
        }
    }

    fn seed_membership() -> (Value, Value) {
        let seed = json!({
            "implemented_roots_all_features":["fake_a","fake_alias","fake_local"],
            "inventory_dispositions":[{"name":"fake_a","implementation":"fake_impl","disposition":"implemented"}],
            "callable_bindings":[["fake_a","fake_impl"],["fake_alias","fake_impl"],["fake_local","fake_local"]],
            "selection":{"schema_owners":["fake_a"],"members":[
                {"name":"fake_a","implementation":"fake_impl","is_root":true,"ancestry":[],"schema_owner":"fake_a"},
                {"name":"fake_alias","implementation":"fake_impl","is_root":true,"ancestry":["fake_a"],"schema_owner":"fake_a"},
                {"name":"fake_local","implementation":"fake_local","is_root":true,"ancestry":[],"schema_owner":null}]}
        });
        let independent = json!({
            "implemented_roots_all_features":["fake_a","fake_alias","fake_local"],
            "inventory_dispositions":[{"name":"fake_a","implementation":"fake_impl","disposition":"implemented"}],
            "callable_bindings":[["fake_a","fake_impl"],["fake_alias","fake_impl"],["fake_local","fake_local"]],
            "roots":["fake_a","fake_alias","fake_local"],"effective_schema_owners":["fake_a"],
            "members":[
                {"name":"fake_a","implementation":"fake_impl","is_root":true,"implemented":true,"ancestry":[],"effective_schema_owner":"fake_a"},
                {"name":"fake_alias","implementation":"fake_impl","is_root":true,"implemented":true,"ancestry":["fake_a"],"effective_schema_owner":"fake_a"},
                {"name":"fake_local","implementation":"fake_local","is_root":true,"implemented":true,"ancestry":[],"effective_schema_owner":null}]
        });
        (seed, independent)
    }

    fn graph() -> Value {
        json!({
            "auxiliary":{"dispatch_edges":[
                {"from":"initialize_native","to":"StagedSchema0","role":"initialize-regexes","ownership":"callee-owned"},
                {"from":"initialize_native","to":"StagedSchema1","role":"initialize-regexes","ownership":"callee-owned"}
            ]},
            "declarations":[
                {"owner":"fake_a","marker":"StagedSchema0","callbacks":[],
                    "initialization":["SECOND_REGEX","FIRST_REGEX"],
                    "programs":[
                        {"identity":"fake_a::StagedSchema0::FIRST_REGEX","symbol":"FIRST_REGEX","pattern":"^x$"},
                        {"identity":"fake_a::StagedSchema0::SECOND_REGEX","symbol":"SECOND_REGEX","pattern":"^y$"}
                    ]},
                {"owner":"fake_b","marker":"StagedSchema1","callbacks":[],
                    "initialization":["FIRST_REGEX"],
                    "programs":[
                        {"identity":"fake_b::StagedSchema1::FIRST_REGEX","symbol":"FIRST_REGEX","pattern":"^x$"}
                    ]}
            ]
        })
    }

    #[test]
    fn runtime_reference_input_is_required_without_global_environment_mutation() {
        assert_eq!(
            reference_path(None).unwrap_err(),
            "catalog reference directory is required"
        );
        assert_eq!(
            reference_path(Some(OsString::new())).unwrap_err(),
            "catalog reference directory is empty"
        );
        assert_eq!(
            reference_path(Some("/fixture/reference".into())).unwrap(),
            PathBuf::from("/fixture/reference")
        );
    }

    #[test]
    fn raw_metadata_rows_preserve_alias_implementation_and_disabled_feature_declarations() {
        let rows = intern_rows(&[
            ("fake_a", "fake_impl", None),
            ("fake_alias", "fake_impl", None),
            ("fake_dev", "fake_dev", Some("insecure-bidders")),
        ])
        .unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].bidder, "fake_a");
        assert_eq!(rows[1].bidder, "fake_alias");
        assert_eq!(rows[1].implementation, "fake_impl");
        assert_eq!(rows[2].required_feature, Some("insecure-bidders"));
        let oversized = "x".repeat(INPUT_BYTES + 1);
        assert_eq!(
            intern_rows(&[(&oversized, "fake_impl", None)]).unwrap_err(),
            "metadata interning bound"
        );
        assert_eq!(
            intern_rows(&[
                ("fake_a", "fake_impl", None),
                ("fake_a", "fake_other", None)
            ])
            .unwrap_err(),
            "duplicate metadata declaration"
        );
    }

    #[test]
    fn seed_membership_matches_the_independent_alias_walk_exactly() {
        let (seed, independent) = seed_membership();
        membership(&seed, &independent).unwrap();
        for pointer in [
            "/implemented_roots_all_features",
            "/inventory_dispositions",
            "/callable_bindings",
            "/selection/schema_owners",
            "/selection/members",
        ] {
            let mut changed = seed.clone();
            *changed.pointer_mut(pointer).unwrap() = json!([]);
            assert_eq!(
                membership(&changed, &independent).unwrap_err(),
                "independent membership mismatch",
                "{pointer}"
            );
        }
        let mut changed = seed;
        changed["selection"]["members"][1]["ancestry"] = json!([]);
        assert_eq!(
            membership(&changed, &independent).unwrap_err(),
            "independent membership mismatch"
        );
    }

    #[test]
    fn actual_initialization_edges_keep_hostname_first_and_preserve_helper_order() {
        let owners = BTreeMap::from([("fake_a".into(), "a".into()), ("fake_b".into(), "b".into())]);
        let expected = vec![
            ("shared::hostname".into(), HOSTNAME_SOURCE.into()),
            ("fake_a::StagedSchema0::SECOND_REGEX".into(), "^y$".into()),
            ("fake_a::StagedSchema0::FIRST_REGEX".into(), "^x$".into()),
            ("fake_b::StagedSchema1::FIRST_REGEX".into(), "^x$".into()),
        ];
        assert_eq!(schedule(&graph(), &owners).unwrap(), expected);
        let mut reordered = graph();
        reordered["declarations"].as_array_mut().unwrap().reverse();
        assert_eq!(schedule(&reordered, &owners).unwrap(), expected);
        let mut changed = graph();
        changed["declarations"][1]["owner"] = json!("fake_a");
        assert_eq!(
            schedule(&changed, &owners).unwrap_err(),
            "expanded schedule mismatch"
        );
        let mut changed = graph();
        changed["declarations"][0]["marker"] = json!("StagedSchema99");
        assert_eq!(
            schedule(&changed, &owners).unwrap_err(),
            "expanded schedule mismatch"
        );
        let mut changed = graph();
        let duplicate = changed["declarations"][0]["programs"][0].clone();
        changed["declarations"][0]["programs"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert_eq!(
            schedule(&changed, &owners).unwrap_err(),
            "expanded schedule mismatch"
        );
        for initialization in [
            json!(["FIRST_REGEX"]),
            json!(["SECOND_REGEX", "FIRST_REGEX", "FIRST_REGEX"]),
            json!(["SECOND_REGEX", "FIRST_REGEX", "UNKNOWN_REGEX"]),
        ] {
            let mut changed = graph();
            changed["declarations"][0]["initialization"] = initialization;
            assert_eq!(
                schedule(&changed, &owners).unwrap_err(),
                "expanded schedule mismatch"
            );
        }
        for edges in [
            json!([]),
            json!([
                {"from":"initialize_native","to":"StagedSchema0","role":"initialize-regexes","ownership":"callee-owned"},
                {"from":"initialize_native","to":"StagedSchema0","role":"initialize-regexes","ownership":"callee-owned"}
            ]),
            json!([
                {"from":"initialize_native","to":"StagedSchema0","role":"initialize-regexes","ownership":"callee-owned"},
                {"from":"initialize_native","to":"StagedSchema99","role":"initialize-regexes","ownership":"callee-owned"}
            ]),
        ] {
            let mut changed = graph();
            changed["auxiliary"]["dispatch_edges"] = edges;
            assert_eq!(
                schedule(&changed, &owners).unwrap_err(),
                "expanded schedule mismatch"
            );
        }
    }

    #[test]
    fn native_formats_initialize_unconditionally_before_generated_initializers() {
        native_format_first(NATIVE_STAGING).unwrap();
        let mut syntax: syn::File =
            syn::parse_file(str::from_utf8(NATIVE_STAGING).unwrap()).unwrap();
        let function = syntax
            .items
            .iter_mut()
            .find_map(|item| match item {
                syn::Item::Fn(function) if function.sig.ident == "initialize_native" => {
                    Some(function)
                }
                _ => None,
            })
            .unwrap();
        function.block = Box::new(syn::parse_quote!({
            generated::retain_for_inspection();
            generated::initialize_native() && formats::initialize()
        }));
        assert_eq!(
            native_format_first(syntax.to_token_stream().to_string().as_bytes()).unwrap_err(),
            "native format initialization order mismatch"
        );
    }

    #[test]
    fn source_capture_only_excludes_explicit_generated_snapshot_paths() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let workspace = root.join("workspace");
        fs::create_dir(&workspace).unwrap();
        let source = workspace.join("source.rs");
        fs::write(&source, b"source data").unwrap();
        let snapshot = root.join("known-generated-schema.json");
        let paths = BTreeSet::from([source.clone(), snapshot.clone()]);
        let captured = source_files(&workspace, &paths, &BTreeSet::from([snapshot])).unwrap();
        assert_eq!(captured.as_object().unwrap().len(), 1);
        assert_eq!(captured["source.rs"]["bytes"], 11);
        let unexpected = root.join("unexpected.rs");
        fs::write(&unexpected, b"unaccounted source").unwrap();
        assert_eq!(
            source_files(
                &workspace,
                &BTreeSet::from([source, unexpected]),
                &BTreeSet::new()
            )
            .unwrap_err(),
            "unexpected source path outside workspace"
        );
    }

    #[test]
    fn identical_owner_sets_allow_zero_growth_without_zero_absolute_observations() {
        let reference = view(true);
        let candidate = view(false);
        let calls = Cell::new(0);
        let report = observe_matched(&reference, &candidate, &Delta::default(), |programs| {
            calls.set(calls.get() + 1);
            assert_eq!(
                programs,
                &[
                    ("shared::hostname".into(), HOSTNAME_SOURCE.into()),
                    ("fake_a::StagedSchema0::Regex0".into(), "^x$".into())
                ]
            );
            Ok(json!({"programs":[{"identity":"shared::hostname",
                "cold":{"retained_bytes":2,"peak_live_bytes":3}},
                {"identity":"fake_a::StagedSchema0::Regex0",
                "cold":{"retained_bytes":13,"peak_live_bytes":17}}],
                "repeat":{"allocated_bytes":0},
                "late_size_limit_failure":{"retained_bytes":0},
                "physical_or_generated_singleton_qualification":false}))
        })
        .unwrap();
        assert_eq!(calls.get(), 2);
        assert_eq!(report["label"], LABEL);
        assert_eq!(
            report["owner_delta"],
            json!({"added":[],"removed":[],"changed":[]})
        );
        assert_eq!(
            report["reference"]["constructor_observation"]["programs"][1]["cold"]["retained_bytes"],
            13
        );
        assert_eq!(
            report["candidate"]["constructor_observation"]["programs"][1]["cold"]["peak_live_bytes"],
            17
        );
        assert_eq!(report["component_deltas"]["generated_source_bytes"], 0);
        assert_eq!(report["common_C_I_qualified"], false);
        assert_eq!(report["task8_complete"], false);
    }

    #[test]
    fn declared_added_owner_is_exact_and_ordinal_changes_do_not_hide_substitution() {
        let reference = view(true);
        let mut candidate = view(false);
        candidate
            .owners
            .insert("fake_0".into(), "schema-new".into());
        candidate.programs = vec![
            ("shared::hostname".into(), HOSTNAME_SOURCE.into()),
            ("fake_0::StagedSchema0::Regex0".into(), "^new$".into()),
            ("fake_a::StagedSchema1::Regex0".into(), "^x$".into()),
        ];
        let declared = Delta {
            added: BTreeSet::from(["fake_0".into()]),
            ..Delta::default()
        };
        assert_eq!(owner_delta(&reference.owners, &candidate.owners), declared);
        match_views(&reference, &candidate, &declared).unwrap();
        assert_eq!(
            match_views(&reference, &candidate, &Delta::default()).unwrap_err(),
            "undeclared owner delta"
        );
        candidate.programs[2].1 = "^substituted$".into();
        assert_eq!(
            match_views(&reference, &candidate, &declared).unwrap_err(),
            "undeclared program substitution"
        );
    }

    #[test]
    fn changed_and_removed_schema_owners_cannot_be_hidden_by_an_added_owner() {
        let reference = view(true);
        let mut candidate = view(false);
        candidate
            .owners
            .insert("fake_a".into(), "changed-schema".into());
        assert_eq!(
            owner_delta(&reference.owners, &candidate.owners),
            Delta {
                changed: BTreeSet::from(["fake_a".into()]),
                ..Delta::default()
            }
        );
        assert_eq!(
            match_views(&reference, &candidate, &Delta::default()).unwrap_err(),
            "undeclared owner delta"
        );
        candidate.owners = BTreeMap::from([("fake_b".into(), "schema-b".into())]);
        candidate.programs = vec![
            ("shared::hostname".into(), HOSTNAME_SOURCE.into()),
            ("fake_b::StagedSchema0::Regex0".into(), "^x$".into()),
        ];
        assert_eq!(
            owner_delta(&reference.owners, &candidate.owners),
            Delta {
                added: BTreeSet::from(["fake_b".into()]),
                removed: BTreeSet::from(["fake_a".into()]),
                ..Delta::default()
            }
        );
    }

    #[test]
    fn reviewed_real_catalog_delta_is_frozen_zero_before_any_observation() {
        let reference = view(true);
        let mut candidate = view(false);
        candidate
            .owners
            .insert("fake_b".into(), "unexpected schema".into());
        candidate
            .programs
            .push(("fake_b::StagedSchema1::Regex0".into(), "^new$".into()));
        let calls = Cell::new(0);
        let reviewed_delta = Delta::default();
        assert_eq!(
            observe_matched(&reference, &candidate, &reviewed_delta, |_| {
                calls.set(calls.get() + 1);
                Ok(json!({}))
            })
            .unwrap_err(),
            "undeclared owner delta"
        );
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn each_measurement_context_change_refuses_before_observing() {
        let reference = view(true);
        for pointer in [
            "/build/target",
            "/build/profile",
            "/build/compiler_verbose",
            "/build/encoded_rustflags",
            "/build/enabled_features",
            "/environment/OPT_LEVEL",
            "/abi/pointer_width",
            "/measurement_lock_sha256",
            "/measurement_sources/fake~1current-generator.rs",
            "/regex_source",
            "/regex_features",
            "/generator_options",
            "/workload",
            "/non_catalog",
        ] {
            let mut candidate = view(false);
            *candidate.measurement.pointer_mut(pointer).unwrap() = json!("independent change");
            let calls = Cell::new(0);
            assert_eq!(
                observe_matched(&reference, &candidate, &Delta::default(), |_| {
                    calls.set(calls.get() + 1);
                    Ok(json!({}))
                })
                .unwrap_err(),
                "measurement context mismatch",
                "{pointer}"
            );
            assert_eq!(calls.get(), 0);
        }
    }

    #[test]
    fn reference_pin_and_post_freeze_source_substitution_refuse() {
        for key in ["commit", "tree"] {
            let mut reference = view(true);
            reference.source[key] = json!("wrong");
            assert_eq!(
                match_views(&reference, &view(false), &Delta::default()).unwrap_err(),
                "reference identity mismatch"
            );
        }
        let reference = view(true);
        let candidate = view(false);
        let token = match_views(&reference, &candidate, &Delta::default()).unwrap();
        let mut changed = reference.clone();
        changed.source["files"]["Cargo.lock"] = json!("substituted bytes");
        assert_eq!(
            verify_frozen(&token, &changed, &candidate).unwrap_err(),
            "frozen inputs changed"
        );
        verify_frozen(&token, &reference, &candidate).unwrap();
    }

    #[test]
    #[cfg(not(feature = "insecure-bidders"))]
    fn matched_catalog_growth_reconstructs_actual_reference_and_candidate() {
        let report = run_real().unwrap();
        assert_eq!(report["reference"]["schema_owner_count"], 16);
        assert_eq!(report["reference"]["public_definition_count"], 18);
        assert_eq!(report["physical_or_serving_qualification"], false);
        eprintln!("{}", serde_json::to_string(&report).unwrap());
    }

    #[test]
    #[cfg(feature = "insecure-bidders")]
    fn development_metadata_matched_catalog_growth_reconstructs_actual_reference_and_candidate() {
        let report = run_real().unwrap();
        assert_eq!(report["reference"]["schema_owner_count"], 16);
        assert_eq!(report["reference"]["public_definition_count"], 18);
        assert_eq!(report["common_C_I_qualified"], false);
        eprintln!("{}", serde_json::to_string(&report).unwrap());
    }
}
