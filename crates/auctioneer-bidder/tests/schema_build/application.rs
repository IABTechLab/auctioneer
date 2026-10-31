//! Actual application artifacts, never isolated probes or resource bounds.

use crate::schema_qualification::SchemaInventory;
use auctioneer_config_schema::bidder_discovery::{Discovery, scan};
use auctioneer_config_schema::schema_staging::BuildContext;
use auctioneer_config_schema::schema_staging_build::{BuildRequest, SchemaSnapshot, generate};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Read as _;
use std::mem::replace;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::str::from_utf8;
use std::time::Duration;

const MAX_MESSAGES: usize = 4 << 20;
const MAX_ARTIFACT: u64 = 64 << 20;
const MAX_STAGING: u64 = 4 << 20;
const CHILD_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Clone, Copy)]
struct Application {
    package: &'static str,
    target_name: &'static str,
    kind: &'static str,
    target: Option<&'static str>,
    profile: &'static str,
}

const APPLICATIONS: [Application; 4] = [
    Application {
        package: "auctioneer-adapter-axum",
        target_name: "auctioneer-adapter-axum",
        kind: "bin",
        target: None,
        profile: "release",
    },
    Application {
        package: "auctioneer-adapter-cloudflare",
        target_name: "auctioneer_adapter_cloudflare",
        kind: "cdylib",
        target: Some("wasm32-unknown-unknown"),
        profile: "wasm-release",
    },
    Application {
        package: "auctioneer-adapter-fastly",
        target_name: "auctioneer-adapter-fastly",
        kind: "bin",
        target: Some("wasm32-wasip1"),
        profile: "wasm-release",
    },
    Application {
        package: "auctioneer-adapter-spin",
        target_name: "auctioneer_adapter_spin",
        kind: "cdylib",
        target: Some("wasm32-wasip2"),
        profile: "wasm-release",
    },
];

struct ExpectedBuild<'a> {
    target: &'a str,
    host: &'a str,
    profile: &'a str,
    compiler: &'a str,
    compiler_path: &'a Path,
    out_dir: &'a Path,
    development: bool,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn enabled_features(development: bool) -> Vec<&'static str> {
    if development {
        vec!["insecure-bidders", "schema-artifact-staging"]
    } else {
        vec!["schema-artifact-staging"]
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct BuildProducts {
    pub artifact: PathBuf,
    pub bidder_output: PathBuf,
}

pub fn products(
    messages: &[u8],
    application: &str,
    bidder: &str,
    target_name: &str,
    development: bool,
    guest: bool,
) -> Result<BuildProducts, &'static str> {
    if messages.len() > MAX_MESSAGES {
        return Err("Cargo build messages exceed byte cap");
    }
    let text = from_utf8(messages).map_err(|_error| "invalid Cargo build messages")?;
    let mut artifact = None;
    let mut bidder_output = None;
    let mut features = None;
    let mut finished = None;
    for line in text.lines() {
        let row: Value =
            serde_json::from_str(line).map_err(|_error| "invalid Cargo build messages")?;
        let package = row["package_id"].as_str();
        match row["reason"].as_str() {
            Some("compiler-artifact") => {
                let kinds = row["target"]["kind"]
                    .as_array()
                    .ok_or("invalid Cargo target")?;
                if package == Some(bidder)
                    && kinds.iter().any(|kind| kind == "lib" || kind == "rlib")
                {
                    let values = row["features"]
                        .as_array()
                        .ok_or("application bidder feature mismatch")?;
                    let parsed = values
                        .iter()
                        .map(|value| value.as_str().map(str::to_owned))
                        .collect::<Option<BTreeSet<_>>>()
                        .ok_or("application bidder feature mismatch")?;
                    if parsed.len() != values.len() {
                        return Err("application bidder feature mismatch");
                    }
                    unique(&mut features, parsed)?;
                }
                if package == Some(application)
                    && row["target"]["name"] == target_name
                    && kinds
                        .iter()
                        .any(|kind| kind == "bin" || (guest && kind == "cdylib"))
                {
                    unique(&mut artifact, artifact_path(&row, guest)?)?;
                }
            }
            Some("build-script-executed") if package == Some(bidder) => {
                unique(&mut bidder_output, absolute_path(&row["out_dir"])?)?;
            }
            Some("build-finished") => {
                let success = row["success"]
                    .as_bool()
                    .ok_or("application build did not complete")?;
                if !success {
                    return Err("application build did not complete");
                }
                unique(&mut finished, success)?;
            }
            _ => {}
        }
    }
    if finished != Some(true) {
        return Err("application build did not complete");
    }
    let artifact_file = artifact.ok_or("application artifact missing")?;
    let bidder_out_dir = bidder_output.ok_or("application staging output missing")?;
    let mut expected = BTreeSet::from(["schema-artifact-staging".to_owned()]);
    if development {
        expected.insert("insecure-bidders".to_owned());
    }
    if features.as_ref() != Some(&expected) {
        return Err("application bidder feature mismatch");
    }
    Ok(BuildProducts {
        artifact: artifact_file,
        bidder_output: bidder_out_dir,
    })
}

fn unique<T>(slot: &mut Option<T>, value: T) -> Result<(), &'static str> {
    if slot.replace(value).is_some() {
        return Err("ambiguous application build products");
    }
    Ok(())
}

fn absolute_path(value: &Value) -> Result<PathBuf, &'static str> {
    let path = Path::new(value.as_str().ok_or("invalid application artifact path")?);
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err("invalid application artifact path");
    }
    Ok(path.to_owned())
}

fn artifact_path(row: &Value, guest: bool) -> Result<PathBuf, &'static str> {
    if !guest {
        let executable = absolute_path(&row["executable"])?;
        if row["filenames"] != json!([executable]) {
            return Err("application executable mismatch");
        }
        return Ok(executable);
    }
    let names = row["filenames"]
        .as_array()
        .ok_or("invalid application artifact path")?;
    let mut artifact = None;
    for name in names {
        let path = absolute_path(name)?;
        if path
            .extension()
            .is_some_and(|extension| extension == "wasm")
        {
            unique(&mut artifact, path)?;
        }
    }
    let artifact_file = artifact.ok_or("application artifact missing")?;
    if row["target"]["kind"]
        .as_array()
        .is_some_and(|kinds| kinds.iter().any(|kind| kind == "bin"))
        && absolute_path(&row["executable"])? != artifact_file
    {
        return Err("application executable mismatch");
    }
    Ok(artifact_file)
}

fn contained(root: &Path, path: &Path, directory: bool) -> Result<(), String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_error| "application path escapes build root")?;
    if !path.is_absolute()
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err("application path escapes build root".into());
    }
    let mut current = root.to_owned();
    for part in relative.components() {
        current.push(part);
        if fs::symlink_metadata(&current)
            .map_err(|error| error.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("symlink in application build path".into());
        }
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if if directory {
        !metadata.is_dir()
    } else {
        !metadata.is_file()
    } {
        return Err("nonregular application build product".into());
    }
    Ok(())
}

fn read_bounded(root: &Path, path: &Path, cap: u64) -> Result<Vec<u8>, String> {
    contained(root, path, false)?;
    let file = File::open(path).map_err(|error| error.to_string())?;
    let length = file.metadata().map_err(|error| error.to_string())?.len();
    if length == 0 || length > cap {
        return Err("application build product exceeds byte cap or is empty".into());
    }
    let mut bytes = Vec::new();
    let read_limit = cap.checked_add(1).ok_or("application read cap overflow")?;
    file.take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if u64::try_from(bytes.len()).map_err(|error| error.to_string())? != length {
        return Err("application build product changed while reading".into());
    }
    Ok(bytes)
}

fn package<'a>(metadata: &'a Value, name: &str, workspace: &Path) -> Result<&'a Value, String> {
    let packages = metadata["packages"]
        .as_array()
        .ok_or("invalid application Cargo metadata")?;
    let mut matching = packages.iter().filter(|row| row["name"] == name);
    let row = matching.next().ok_or("application Cargo package missing")?;
    if matching.next().is_some()
        || row["id"].as_str().is_none_or(str::is_empty)
        || !row["source"].is_null()
        || row["manifest_path"].as_str().map(Path::new)
            != Some(
                workspace
                    .join("crates")
                    .join(name)
                    .join("Cargo.toml")
                    .as_path(),
            )
    {
        return Err("application Cargo package identity mismatch".into());
    }
    Ok(row)
}

fn metadata_target<'a>(package: &'a Value, name: &str, kind: &str) -> Result<&'a Value, String> {
    let targets = package["targets"]
        .as_array()
        .ok_or("invalid application Cargo targets")?;
    let mut matching = targets.iter().filter(|row| {
        row["name"] == name
            && row["kind"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|value| value == kind))
    });
    let row = matching.next().ok_or("application Cargo target missing")?;
    if matching.next().is_some() {
        return Err("ambiguous application Cargo target".into());
    }
    Ok(row)
}

struct FeaturePackage<'a> {
    package_id: &'a str,
    target: &'a Value,
    features: BTreeSet<String>,
}

fn application_features(package: &Value, development: bool) -> Result<BTreeSet<String>, String> {
    let declarations = package["features"]
        .as_object()
        .ok_or("invalid application Cargo features")?;
    let mut pending = Vec::new();
    if declarations.contains_key("default") {
        pending.push("default");
    }
    if development {
        pending.push("insecure-bidders");
    }
    let mut features = BTreeSet::new();
    while let Some(name) = pending.pop() {
        if !features.insert(name.to_owned()) {
            continue;
        }
        let entries = declarations
            .get(name)
            .and_then(Value::as_array)
            .ok_or("invalid application Cargo features")?;
        for entry in entries {
            let feature = entry.as_str().ok_or("invalid application Cargo features")?;
            if !feature.starts_with("dep:") && !feature.contains('/') {
                pending.push(feature);
            }
        }
    }
    Ok(features)
}

fn compiled_feature_closure(
    messages: &[u8],
    packages: &[FeaturePackage<'_>],
) -> Result<(), String> {
    if messages.len() > MAX_MESSAGES {
        return Err("Cargo build messages exceed byte cap".into());
    }
    let text = from_utf8(messages).map_err(|_error| "invalid Cargo build messages")?;
    let mut seen = vec![false; packages.len()];
    for line in text.lines() {
        let row: Value =
            serde_json::from_str(line).map_err(|_error| "invalid Cargo build messages")?;
        if row["reason"] != "compiler-artifact" {
            continue;
        }
        let Some((index, expected)) = packages.iter().enumerate().find(|(_, expected)| {
            row["package_id"] == expected.package_id
                && row["target"]["name"] == expected.target["name"]
        }) else {
            continue;
        };
        if replace(&mut seen[index], true) {
            return Err("ambiguous application feature closure".into());
        }
        if ["name", "kind", "crate_types", "src_path"]
            .iter()
            .any(|field| row["target"][field] != expected.target[field])
        {
            return Err("application compiled target differs from Cargo metadata".into());
        }
        let values = row["features"]
            .as_array()
            .ok_or("application compiled feature closure mismatch")?;
        let actual = values
            .iter()
            .map(|value| value.as_str().map(str::to_owned))
            .collect::<Option<BTreeSet<_>>>()
            .ok_or("application compiled feature closure mismatch")?;
        if actual.len() != values.len() || actual != expected.features {
            return Err("application compiled feature closure mismatch".into());
        }
    }
    if seen.iter().any(|present| !present) {
        return Err("application compiled feature closure missing".into());
    }
    Ok(())
}

fn compilation_targets(
    messages: &[u8],
    app: &Value,
    bidder: &Value,
    app_target: &Value,
    bidder_target: &Value,
    guest: bool,
) -> Result<(), String> {
    let text = from_utf8(messages).map_err(|_error| "invalid Cargo build messages")?;
    for line in text.lines() {
        let row: Value =
            serde_json::from_str(line).map_err(|_error| "invalid Cargo build messages")?;
        if row["reason"] != "compiler-artifact" {
            continue;
        }
        let expected =
            if row["package_id"] == app["id"] && row["target"]["name"] == app_target["name"] {
                Some(app_target)
            } else if row["package_id"] == bidder["id"]
                && row["target"]["name"] == bidder_target["name"]
            {
                Some(bidder_target)
            } else {
                None
            };
        if let Some(target) = expected {
            if ["name", "kind", "crate_types", "src_path"]
                .iter()
                .any(|field| row["target"][field] != target[field])
            {
                return Err("application compiled target differs from Cargo metadata".into());
            }
            if row["profile"]["opt_level"] != "3"
                || row["profile"]["test"] != false
                || row["profile"]["debug_assertions"] != false
                || row["profile"]["overflow_checks"] != false
                || row["profile"]["debuginfo"] != json!(u8::from(!guest))
            {
                return Err("application compiled profile mismatch".into());
            }
        }
    }
    Ok(())
}

fn check_build(
    manifest: &Value,
    expected: &ExpectedBuild<'_>,
) -> Result<BTreeMap<String, String>, String> {
    if manifest["schema_version"] != 1_u32
        || manifest["kind"] != "unqualified_non_serving_schema_staging"
        || manifest["identity"]["qualification"] != "Unqualified"
        || manifest["linked_or_serving_qualification"] != false
        || manifest["artifact_inspection"] != "pending"
    {
        return Err("application staging must remain unqualified".into());
    }
    if manifest["build"]["enabled_features"] != json!(enabled_features(expected.development)) {
        return Err("application staging feature mismatch".into());
    }
    if manifest["build"]["target"] != expected.target {
        return Err("application staging target mismatch".into());
    }
    if manifest["build"]["profile"] != expected.profile {
        return Err("application staging profile mismatch".into());
    }
    if manifest["build"]["compiler_verbose"] != expected.compiler
        || manifest["build"]["encoded_rustflags"] != ""
    {
        return Err("application staging compiler or flags mismatch".into());
    }
    let environment: BTreeMap<String, String> =
        serde_json::from_value(manifest["build_environment"].clone())
            .map_err(|_error| "invalid application staging build environment")?;
    if environment.get("OUT_DIR").map(Path::new) != Some(expected.out_dir) {
        return Err("application staging OUT_DIR mismatch".into());
    }
    let features: BTreeMap<_, _> = enabled_features(expected.development)
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
        .collect();
    let actual: BTreeMap<_, _> = environment
        .iter()
        .filter(|(key, _)| key.starts_with("CARGO_FEATURE_"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    if actual != features
        || environment.get("CARGO_CFG_FEATURE")
            != Some(&enabled_features(expected.development).join(","))
    {
        return Err("application staging feature mismatch".into());
    }
    if environment.get("RUSTC").map(Path::new) != Some(expected.compiler_path)
        || [
            ("TARGET", expected.target),
            ("HOST", expected.host),
            ("PROFILE", "release"),
            ("OPT_LEVEL", "3"),
            (
                "DEBUG",
                if expected.profile == "release" {
                    "true"
                } else {
                    "false"
                },
            ),
            ("CARGO_ENCODED_RUSTFLAGS", ""),
        ]
        .iter()
        .any(|(key, value)| environment.get(*key).map(String::as_str) != Some(*value))
        || environment.keys().any(|key| {
            key.starts_with("CARGO_PROFILE_")
                || matches!(
                    key.as_str(),
                    "RUSTFLAGS" | "RUSTC_WRAPPER" | "RUSTC_WORKSPACE_WRAPPER"
                )
        })
    {
        return Err("application staging build environment mismatch".into());
    }
    Ok(environment)
}

fn check_cohort(
    manifest: &Value,
    schemas: &[SchemaInventory],
    cohort: &Value,
    development: bool,
) -> Result<(), String> {
    let lane = if development { "development" } else { "secure" };
    let selected = cohort["selection"]["schemas"]
        .as_array()
        .ok_or("application source cohort mismatch")?;
    let staged = manifest["schemas"]
        .as_array()
        .ok_or("application source cohort mismatch")?;
    let owners: Vec<_> = schemas
        .iter()
        .map(|row| row.witness.name.as_str())
        .collect();
    if schemas.is_empty()
        || schemas.len() > 512
        || owners.iter().any(|name| name.is_empty())
        || owners.iter().copied().collect::<BTreeSet<_>>().len() != owners.len()
        || selected.len() != schemas.len()
        || staged.len() != schemas.len()
        || cohort["schema_version"] != 1_u32
        || cohort["lane"] != lane
        || cohort["source"]["linked_or_serving_qualification"] != false
        || cohort["source"]["lanes"][lane] != cohort["selection"]
        || manifest["selection"]["schema_owners"] != json!(owners)
        || manifest["published_roots_all_features"]
            != cohort["source"]["published_roots_all_features"]
        || manifest["planned_roots"] != cohort["source"]["planned_roots"]
    {
        return Err("application source cohort mismatch".into());
    }
    let members = cohort["selection"]["members"]
        .as_array()
        .ok_or("application source cohort mismatch")?;
    let expected_members: Vec<_> = members.iter().map(|row| json!({
        "name":row["name"], "implementation":row["implementation"], "is_root":row["is_root"],
        "ancestry":row["ancestry"], "schema_owner":row["effective_schema_owner"]
    })).collect();
    if manifest["selection"]["members"] != json!(expected_members) {
        return Err("application source cohort mismatch".into());
    }
    for ((schema, binding), actual) in schemas.iter().zip(selected).zip(staged) {
        let witness = serde_json::to_value(&schema.witness).map_err(|error| error.to_string())?;
        if binding["witness"] != witness
            || witness
                .as_object()
                .ok_or("invalid application schema witness")?
                .iter()
                .any(|(key, value)| &actual[key] != value)
            || ["repository", "revision"].iter().any(|key| {
                binding["source"][key].as_str().is_none_or(str::is_empty)
                    || actual[key] != binding["source"][key]
            })
        {
            return Err("application schema witness mismatch".into());
        }
    }
    Ok(())
}

fn compare_generated(
    manifest: &Value,
    recomputed: &Value,
    rust: &[u8],
    expected_rust: &[u8],
) -> Result<(), String> {
    if manifest != recomputed {
        return Err("stale application staging manifest".into());
    }
    if rust != expected_rust || manifest["emitted_rust_sha256"] != digest(rust) {
        return Err("stale application staging Rust".into());
    }
    Ok(())
}

fn refusal_controls(
    manifest: &Value,
    schemas: &[SchemaInventory],
    cohort: &Value,
    expected: &ExpectedBuild<'_>,
    rust: &[u8],
) -> Result<Vec<Value>, String> {
    let mut controls = Vec::new();
    for (name, error) in [
        ("feature", "application staging feature mismatch"),
        ("profile", "application staging profile mismatch"),
        ("target", "application staging target mismatch"),
        ("compiler", "application staging compiler or flags mismatch"),
        ("source", "application schema witness mismatch"),
        ("out_dir", "application staging OUT_DIR mismatch"),
        ("stale_manifest", "stale application staging manifest"),
        ("stale_rust", "stale application staging Rust"),
    ] {
        let mut changed = manifest.clone();
        let result = match name {
            "feature" => {
                changed["build"]["enabled_features"] = json!([]);
                check_build(&changed, expected).map(|_| ())
            }
            "profile" | "target" => {
                changed["build"][name] = json!("fake_wrong_request");
                check_build(&changed, expected).map(|_| ())
            }
            "compiler" => {
                changed["build"]["compiler_verbose"] = json!("fake_wrong_compiler");
                check_build(&changed, expected).map(|_| ())
            }
            "source" => {
                changed["schemas"][0]["sha256"] = json!("fake_wrong_source");
                check_cohort(&changed, schemas, cohort, expected.development)
            }
            "out_dir" => {
                changed["build_environment"]["OUT_DIR"] = json!("/fake_wrong_out_dir");
                check_build(&changed, expected).map(|_| ())
            }
            "stale_manifest" => {
                changed["identity"]["sha256"] = json!("fake_stale_identity");
                compare_generated(&changed, manifest, rust, rust)
            }
            "stale_rust" => compare_generated(manifest, manifest, b"fake_stale_rust", rust),
            _ => return Err("unknown application refusal control".into()),
        };
        if result.as_ref().err().map(String::as_str) != Some(error) {
            return Err(format!(
                "application refusal control failed: {name}: {result:?}"
            ));
        }
        controls.push(json!({"mutation":name,"rejected":true,"error":error,"scope":"in_memory_evidence_mutation"}));
    }
    Ok(controls)
}

fn imports(bytes: &[u8], guest: bool) -> Result<Vec<Value>, String> {
    let mut imports = Vec::new();
    if guest {
        use wasmparser::{Parser, Payload};
        for payload in Parser::new(0).parse_all(bytes) {
            match payload.map_err(|_error| "invalid application WASM imports")? {
                Payload::ImportSection(section) => {
                    for entry in section.into_imports() {
                        let import = entry.map_err(|_error| "invalid application WASM import")?;
                        imports.push(json!({"encoding":"core","module":import.module,"name":import.name,"type":format!("{:?}", import.ty)}));
                    }
                }
                Payload::ComponentImportSection(section) => {
                    for entry in section {
                        let import =
                            entry.map_err(|_error| "invalid application component import")?;
                        imports.push(
                            json!({"encoding":"component","declaration":format!("{import:?}")}),
                        );
                    }
                }
                Payload::Version { .. }
                | Payload::TypeSection(_)
                | Payload::FunctionSection(_)
                | Payload::TableSection(_)
                | Payload::MemorySection(_)
                | Payload::TagSection(_)
                | Payload::GlobalSection(_)
                | Payload::ExportSection(_)
                | Payload::StartSection { .. }
                | Payload::ElementSection(_)
                | Payload::DataCountSection { .. }
                | Payload::DataSection(_)
                | Payload::CodeSectionStart { .. }
                | Payload::CodeSectionEntry(_)
                | Payload::ModuleSection { .. }
                | Payload::InstanceSection(_)
                | Payload::CoreTypeSection(_)
                | Payload::ComponentSection { .. }
                | Payload::ComponentInstanceSection(_)
                | Payload::ComponentAliasSection(_)
                | Payload::ComponentTypeSection(_)
                | Payload::ComponentCanonicalSection(_)
                | Payload::ComponentStartSection { .. }
                | Payload::ComponentExportSection(_)
                | Payload::CustomSection(_)
                | Payload::UnknownSection { .. }
                | Payload::End(_) => {}
                _ => return Err("unsupported application WASM import payload".into()),
            }
            if imports.len() > 0x4000 {
                return Err("application imports exceed count cap".into());
            }
        }
    } else {
        use object::Object as _;
        let file =
            object::File::parse(bytes).map_err(|_error| "invalid native application imports")?;
        for import in file
            .imports()
            .map_err(|_error| "invalid native application imports")?
        {
            imports.push(json!({"encoding":"native",
                "library":from_utf8(import.library()).map_err(|_error| "invalid native import library")?,
                "name":from_utf8(import.name()).map_err(|_error| "invalid native import name")?}));
            if imports.len() > 0x4000 {
                return Err("application imports exceed count cap".into());
            }
        }
    }
    if serde_json::to_vec(&imports)
        .map_err(|error| error.to_string())?
        .len()
        > MAX_MESSAGES
    {
        return Err("application imports exceed byte cap".into());
    }
    Ok(imports)
}

struct PreparedInspection {
    workspace: PathBuf,
    crate_dir: PathBuf,
    target_dir: PathBuf,
    compiler: PathBuf,
    compiler_version: String,
    host: String,
    metadata: Value,
    discovery: Discovery,
    development: bool,
}

fn feature_packages(
    prepared: &PreparedInspection,
    application: Application,
) -> Result<Vec<FeaturePackage<'_>>, String> {
    let mut packages = Vec::new();
    for (name, target_name, kind) in [
        (
            application.package,
            application.target_name,
            application.kind,
        ),
        ("auctioneer-core", "auctioneer_core", "lib"),
        ("auctioneer-exchange", "auctioneer_exchange", "lib"),
    ] {
        let package = package(&prepared.metadata, name, &prepared.workspace)?;
        let features = if name == application.package {
            application_features(package, prepared.development)?
        } else if prepared.development {
            BTreeSet::from(["insecure-bidders".to_owned()])
        } else {
            BTreeSet::new()
        };
        packages.push(FeaturePackage {
            package_id: package["id"]
                .as_str()
                .ok_or("invalid application package ID")?,
            target: metadata_target(package, target_name, kind)?,
            features,
        });
    }
    Ok(packages)
}

fn build_product_error(
    prepared: &PreparedInspection,
    application: Application,
    messages: &[u8],
    error: &str,
) -> String {
    let ids: Vec<_> = [
        application.package,
        "auctioneer-bidder",
        "auctioneer-core",
        "auctioneer-exchange",
    ]
    .iter()
    .filter_map(|name| package(&prepared.metadata, name, &prepared.workspace).ok())
    .filter_map(|package| package["id"].as_str())
    .collect();
    let rows: Vec<_> = from_utf8(messages).unwrap_or_default().lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|row| row["package_id"].as_str().is_some_and(|id| ids.contains(&id)))
        .filter(|row| row["reason"] == "compiler-artifact" || row["reason"] == "build-script-executed")
        .take(16)
        .map(|row| json!({"reason":row["reason"],"package_id":row["package_id"],"target":row["target"],
            "features":row["features"],"profile":row["profile"],"out_dir":row["out_dir"],"filenames":row["filenames"]}))
        .collect();
    let evidence = serde_json::to_string(&rows).unwrap_or_default();
    let bounded_evidence: String = evidence.chars().take(4096).collect();
    format!(
        "{} ({}, {}): {error}; Cargo evidence: {bounded_evidence}",
        application.package,
        application.target.unwrap_or(&prepared.host),
        application.profile
    )
}

fn source_workspace(root: &Path, cohort_manifest: &Value) -> Result<PathBuf, String> {
    let canonical_root = fs::canonicalize(root).map_err(|error| error.to_string())?;
    let workspace = if canonical_root.ends_with("crates/auctioneer-bidder/src/bidders") {
        canonical_root
            .ancestors()
            .nth(4)
            .ok_or("invalid application workspace root")?
            .to_owned()
    } else {
        canonical_root
    };
    let crate_dir = workspace.join("crates/auctioneer-bidder");
    contained(&workspace, &crate_dir, true)?;
    for (name, field) in [
        ("Cargo.toml", "workspace_manifest_sha256"),
        ("Cargo.lock", "lock_sha256"),
    ] {
        if cohort_manifest["source"][field]
            != digest(&read_bounded(
                &workspace,
                &workspace.join(name),
                MAX_STAGING,
            )?)
        {
            return Err("stale application source cohort".into());
        }
    }
    Ok(workspace)
}

fn application_cargo(workspace: &Path, target_dir: &Path, compiler: &Path) -> Command {
    let mut command = super::cargo(workspace);
    command
        .env("RUSTC", compiler)
        .env("CARGO_TARGET_DIR", target_dir)
        .env("CARGO_ENCODED_RUSTFLAGS", "");
    command
}

fn prepare(
    root: &Path,
    cohort_manifest: &Value,
    development: bool,
) -> Result<PreparedInspection, String> {
    let workspace = source_workspace(root, cohort_manifest)?;
    let crate_dir = workspace.join("crates/auctioneer-bidder");
    let (compiler, compiler_version) = super::selected::probe_compiler()?;
    let hosts: Vec<_> = compiler_version
        .lines()
        .filter_map(|line| line.strip_prefix("host: "))
        .collect();
    if hosts.len() != 1 || hosts[0].is_empty() {
        return Err("invalid application compiler host".into());
    }
    let host = hosts[0].to_owned();
    let target_dir = workspace.join("target");
    let mut command = application_cargo(&workspace, &target_dir, &compiler);
    command.args([
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
        "--offline",
        "--locked",
    ]);
    let metadata_output = super::successful(&mut command, Duration::from_secs(60))?;
    let metadata: Value =
        serde_json::from_slice(&metadata_output.stdout).map_err(|error| error.to_string())?;
    if metadata["workspace_root"].as_str().map(Path::new) != Some(workspace.as_path())
        || metadata["target_directory"].as_str().map(Path::new) != Some(target_dir.as_path())
    {
        return Err("application Cargo build root mismatch".into());
    }
    let bidder = package(&metadata, "auctioneer-bidder", &workspace)?;
    metadata_target(bidder, "auctioneer_bidder", "lib")?;
    let discovery = scan(&crate_dir)?;
    Ok(PreparedInspection {
        workspace,
        crate_dir,
        target_dir,
        compiler,
        compiler_version,
        host,
        metadata,
        discovery,
        development,
    })
}

fn build_arguments(application: Application, development: bool) -> Vec<&'static str> {
    let mut arguments = vec![
        "build",
        "--offline",
        "--locked",
        "-j",
        "2",
        "--message-format=json-render-diagnostics",
        "-p",
        application.package,
        "-p",
        "auctioneer-bidder",
        "--profile",
        application.profile,
        "--features",
        if development {
            "schema-artifact-staging,insecure-bidders"
        } else {
            "schema-artifact-staging"
        },
    ];
    if let Some(target) = application.target {
        arguments.extend(["--target", target]);
    }
    if application.kind == "bin" {
        arguments.extend(["--bin", application.target_name]);
    } else {
        arguments.push("--lib");
    }
    arguments
}

fn build_one(
    prepared: &PreparedInspection,
    application: Application,
) -> Result<BuildProducts, String> {
    let app = package(&prepared.metadata, application.package, &prepared.workspace)?;
    let app_target = metadata_target(app, application.target_name, application.kind)?;
    let bidder = package(&prepared.metadata, "auctioneer-bidder", &prepared.workspace)?;
    let bidder_target = metadata_target(bidder, "auctioneer_bidder", "lib")?;
    let guest = application.target.is_some();
    let target = application.target.unwrap_or(&prepared.host);
    let mut command = application_cargo(
        &prepared.workspace,
        &prepared.target_dir,
        &prepared.compiler,
    );
    command.args(build_arguments(application, prepared.development));
    let output = super::successful(&mut command, CHILD_TIMEOUT).map_err(|error| {
        format!(
            "{} ({target}, {}): {error}",
            application.package, application.profile
        )
    })?;
    let products = products(
        &output.stdout,
        app["id"].as_str().ok_or("invalid application package ID")?,
        bidder["id"].as_str().ok_or("invalid bidder package ID")?,
        application.target_name,
        prepared.development,
        guest,
    )
    .map_err(|error| build_product_error(prepared, application, &output.stdout, error))?;
    compilation_targets(
        &output.stdout,
        app,
        bidder,
        app_target,
        bidder_target,
        guest,
    )
    .map_err(|error| build_product_error(prepared, application, &output.stdout, &error))?;
    compiled_feature_closure(&output.stdout, &feature_packages(prepared, application)?)
        .map_err(|error| build_product_error(prepared, application, &output.stdout, &error))?;
    Ok(products)
}

fn check_product_paths(
    prepared: &PreparedInspection,
    application: Application,
    products: &BuildProducts,
) -> Result<(), String> {
    contained(&prepared.workspace, &prepared.target_dir, true)?;
    let profile_dir = application
        .target
        .map_or_else(
            || prepared.target_dir.clone(),
            |target| prepared.target_dir.join(target),
        )
        .join(application.profile);
    let filename = if application.target.is_some() {
        // Cargo bin filenames preserve hyphens; library filenames use underscores.
        let name = if application.kind == "bin" {
            application.target_name.to_owned()
        } else {
            application.target_name.replace('-', "_")
        };
        format!("{name}.wasm")
    } else {
        application.target_name.to_owned()
    };
    if products.artifact != profile_dir.join(filename) {
        return Err("application artifact filename or profile mismatch".into());
    }
    contained(&prepared.target_dir, &products.artifact, false)?;
    contained(&prepared.target_dir, &products.bidder_output, true)?;
    let output_parent = products
        .bidder_output
        .parent()
        .ok_or("invalid application staging OUT_DIR")?;
    if products
        .bidder_output
        .file_name()
        .is_none_or(|name| name != "out")
        || output_parent.parent() != Some(profile_dir.join("build").as_path())
        || output_parent
            .file_name()
            .and_then(|name| name.to_str())
            .is_none_or(|name| !name.starts_with("auctioneer-bidder-"))
    {
        return Err("application staging OUT_DIR profile mismatch".into());
    }
    Ok(())
}

fn check_snapshots(
    target_dir: &Path,
    out_dir: &Path,
    snapshots: &[SchemaSnapshot],
) -> Result<(), String> {
    for snapshot in snapshots {
        if snapshot.relative_path.components().count() != 1
            || !matches!(
                snapshot.relative_path.components().next(),
                Some(Component::Normal(_))
            )
        {
            return Err("application snapshot escapes OUT_DIR".into());
        }
        let captured = read_bounded(target_dir, &out_dir.join(&snapshot.relative_path), 1 << 20)?;
        if captured != snapshot.bytes || digest(&captured) != snapshot.sha256 {
            return Err("stale application schema snapshot".into());
        }
    }
    Ok(())
}

fn verify_staging(
    prepared: &PreparedInspection,
    expected: &ExpectedBuild<'_>,
    manifest: &Value,
    schemas: &[SchemaInventory],
    cohort_manifest: &Value,
) -> Result<(String, Vec<u8>), String> {
    let environment = check_build(manifest, expected)?;
    check_cohort(manifest, schemas, cohort_manifest, prepared.development)?;
    let features = enabled_features(prepared.development);
    let generated = generate(BuildRequest {
        crate_dir: &prepared.crate_dir,
        out_dir: expected.out_dir,
        discovery: &prepared.discovery,
        metadata: auctioneer_config::bidder_metadata_declarations(),
        context: BuildContext {
            target: expected.target,
            profile: expected.profile,
            enabled_features: &features,
            compiler_verbose: &prepared.compiler_version,
            encoded_rustflags: "",
        },
        build_environment: &environment,
    })?;
    let recomputed: Value =
        serde_json::from_str(&generated.manifest_json).map_err(|error| error.to_string())?;
    let rust = read_bounded(
        &prepared.target_dir,
        &expected.out_dir.join("schema-staging.rs"),
        MAX_STAGING,
    )?;
    compare_generated(
        manifest,
        &recomputed,
        &rust,
        generated.rust.to_string().as_bytes(),
    )?;
    check_snapshots(&prepared.target_dir, expected.out_dir, &generated.snapshots)?;
    Ok((generated.identity.sha256, rust))
}

fn linked_observations(
    bytes: &[u8],
    guest: bool,
    identity: &str,
    owners: &[&str],
) -> Result<(Vec<Value>, Value), String> {
    let records = if guest {
        super::inspection::wasm_records(bytes, identity, owners)
    } else {
        super::inspection::native_records(bytes, identity, owners)
    }
    .map_err(|error| {
        if guest && let Err(detail) = super::schema_artifact::validator().validate_all(bytes) {
            return format!("{error}: {detail}");
        }
        error.to_owned()
    })?;
    let observations = if guest {
        serde_json::to_value(super::schema_artifact::measure(bytes)?)
            .map_err(|error| error.to_string())?
    } else {
        let mut witnesses = owners.to_vec();
        witnesses.push(identity);
        serde_json::to_value(super::schema_artifact::measure_native(bytes, &witnesses)?)
            .map_err(|error| error.to_string())?
    };
    let record_rows = records
        .iter()
        .map(|row| json!({"owner":row.owner,"functions":row.functions}))
        .collect();
    Ok((record_rows, observations))
}

fn verify_one(
    prepared: &PreparedInspection,
    application: Application,
    products: &BuildProducts,
    schemas: &[SchemaInventory],
    cohort_manifest: &Value,
) -> Result<Value, String> {
    check_product_paths(prepared, application, products)?;
    let guest = application.target.is_some();
    let target = application.target.unwrap_or(&prepared.host);
    let bytes = read_bounded(&prepared.target_dir, &products.artifact, MAX_ARTIFACT)?;
    let manifest_bytes = read_bounded(
        &prepared.target_dir,
        &products.bidder_output.join("schema-staging.json"),
        MAX_STAGING,
    )?;
    let manifest: Value =
        serde_json::from_slice(&manifest_bytes).map_err(|error| error.to_string())?;
    let expected = ExpectedBuild {
        target,
        host: &prepared.host,
        profile: application.profile,
        compiler: &prepared.compiler_version,
        compiler_path: &prepared.compiler,
        out_dir: &products.bidder_output,
        development: prepared.development,
    };
    let (identity, rust) =
        verify_staging(prepared, &expected, &manifest, schemas, cohort_manifest)?;
    let owners: Vec<_> = schemas
        .iter()
        .map(|row| row.witness.name.as_str())
        .collect();
    let (records, observations) = linked_observations(&bytes, guest, &identity, &owners)?;
    let controls = refusal_controls(&manifest, schemas, cohort_manifest, &expected, &rust)?;
    let app = package(&prepared.metadata, application.package, &prepared.workspace)?;
    Ok(
        json!({"package":application.package,"package_id":app["id"],"target_name":application.target_name,
            "target_kind":application.kind,"target":target,"profile":application.profile,"filename":products.artifact,
            "artifact_bytes":bytes.len(),"artifact_sha256":digest(&bytes),"bidder_out_dir":products.bidder_output,
            "manifest_sha256":digest(&manifest_bytes),"identity_sha256":identity,"manifest":manifest,
            "qualification":"Unqualified","resource_qualification":false,"final_admitted_artifact":false,
            "linked_records":records,
            "observations":observations,"actual_imports":imports(&bytes, guest)?,"refusal_controls":controls,
            "observation_scope":"linked code/data and declared imports only; not memory, heap, work or serving bounds"}),
    )
}

/// Inspect all four actual linked adapters in one requested feature lane.
/// `root` may be the workspace or the source cohort's `src/bidders` directory.
pub fn inspect(
    root: &Path,
    schemas: &[SchemaInventory],
    cohort_manifest: &Value,
    development: bool,
) -> Result<Value, String> {
    let prepared = prepare(root, cohort_manifest, development)?;
    let _lease = super::acquire_lease(&prepared.target_dir)?;
    let mut reports = Vec::new();
    for application in APPLICATIONS {
        let products = build_one(&prepared, application)?;
        reports.push(
            verify_one(&prepared, application, &products, schemas, cohort_manifest).map_err(
                |error| {
                    format!(
                        "{} ({}, {}): {error}",
                        application.package,
                        application.target.unwrap_or(&prepared.host),
                        application.profile
                    )
                },
            )?,
        );
    }
    Ok(
        json!({"schema_version":1_u32,"kind":"actual_application_schema_staging_inspection",
        "qualification":"Unqualified","linked_or_serving_qualification":false,"resource_qualification":false,
        "final_admitted_artifact":false,"final_admitted_artifact_recheck_required":true,
        "lane":if development {"development"} else {"secure"},"binding":cohort_manifest,
        "applications":reports,"child_timeout_seconds":CHILD_TIMEOUT.as_secs(),"stream_byte_cap":MAX_MESSAGES,
        "pending_gaps":["resource_bounds","serving_integration","final_admitted_artifact_recheck","hosted_execution"]}),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        BuildProducts, ExpectedBuild, check_build, check_cohort, compare_generated, digest,
        enabled_features, products, refusal_controls,
    };
    use crate::schema_qualification::{SchemaInventory, SchemaWitness, audit_schema};
    use serde_json::{Value, json};
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    fn messages(development: bool, guest: bool) -> Vec<Value> {
        let mut features = vec!["schema-artifact-staging"];
        if development {
            features.push("insecure-bidders");
        }
        vec![
            json!({"reason":"compiler-artifact","package_id":"fake_bidder_package",
                "target":{"name":"fake_bidder_library","kind":["lib"]},
                "features":features,"filenames":["/fake-target/libfake.rlib"],"executable":null}),
            json!({"reason":"build-script-executed","package_id":"fake_bidder_package",
                "out_dir":"/fake-target/build/fake_bidder/out"}),
            json!({"reason":"compiler-artifact","package_id":"fake_application_package",
                "target":{"name":"fake_application","kind":if guest {vec!["cdylib","rlib"]} else {vec!["bin"]}},
                "filenames":if guest {vec!["/fake-target/fake_application.wasm","/fake-target/fake_application.rlib"]} else {vec!["/fake-target/fake_application"]},
                "executable":if guest {Value::Null} else {json!("/fake-target/fake_application")}}),
            json!({"reason":"build-finished","success":true}),
        ]
    }

    fn inspect(
        rows: &[Value],
        development: bool,
        guest: bool,
    ) -> Result<BuildProducts, &'static str> {
        let mut bytes = Vec::new();
        for row in rows {
            serde_json::to_writer(&mut bytes, row)
                .map_err(|_error| "invalid fake Cargo build messages")?;
            bytes.push(b'\n');
        }
        products(
            &bytes,
            "fake_application_package",
            "fake_bidder_package",
            "fake_application",
            development,
            guest,
        )
    }

    #[test]
    fn application_products_accept_wasm_bin_artifacts() {
        for development in [false, true] {
            let mut rows = messages(development, true);
            rows[2]["target"]["kind"] = json!(["bin"]);
            rows[2]["filenames"] = json!(["/fake-target/fake_application.wasm"]);
            rows[2]["executable"] = json!("/fake-target/fake_application.wasm");
            assert_eq!(
                inspect(&rows, development, true),
                Ok(BuildProducts {
                    artifact: PathBuf::from("/fake-target/fake_application.wasm"),
                    bidder_output: PathBuf::from("/fake-target/build/fake_bidder/out"),
                })
            );
        }
    }

    #[test]
    fn application_products_select_the_exact_package_target_and_feature_lane() {
        for development in [false, true] {
            for guest in [false, true] {
                assert_eq!(
                    inspect(&messages(development, guest), development, guest),
                    Ok(BuildProducts {
                        artifact: PathBuf::from(if guest {
                            "/fake-target/fake_application.wasm"
                        } else {
                            "/fake-target/fake_application"
                        }),
                        bidder_output: PathBuf::from("/fake-target/build/fake_bidder/out"),
                    })
                );
            }
        }
    }

    #[test]
    fn application_products_refuse_normal_wrong_lane_and_failed_builds() {
        for features in [
            json!([]),
            json!(["insecure-bidders"]),
            json!(["schema-artifact-staging", "insecure-bidders"]),
        ] {
            let mut rows = messages(false, false);
            rows[0]["features"] = features;
            assert_eq!(
                inspect(&rows, false, false),
                Err("application bidder feature mismatch")
            );
        }
        let mut rows = messages(false, false);
        rows[3]["success"] = json!(false);
        assert_eq!(
            inspect(&rows, false, false),
            Err("application build did not complete")
        );
        rows.pop();
        assert_eq!(
            inspect(&rows, false, false),
            Err("application build did not complete")
        );
    }

    #[test]
    fn application_products_refuse_missing_duplicate_and_foreign_artifacts() {
        for (index, expected) in [
            "application bidder feature mismatch",
            "application staging output missing",
            "application artifact missing",
            "application build did not complete",
        ]
        .iter()
        .enumerate()
        {
            let mut missing = messages(false, false);
            missing.remove(index);
            assert_eq!(
                inspect(&missing, false, false),
                Err(*expected),
                "missing row {index}"
            );
            let mut duplicate = messages(false, false);
            duplicate.push(duplicate[index].clone());
            assert_eq!(
                inspect(&duplicate, false, false),
                Err("ambiguous application build products"),
                "duplicate row {index}"
            );
        }
        for field in ["package_id", "target"] {
            let mut rows = messages(false, false);
            rows[2][field] = if field == "target" {
                json!({"name":"fake_other_application","kind":["bin"]})
            } else {
                json!("fake_other_package")
            };
            assert_eq!(
                inspect(&rows, false, false),
                Err("application artifact missing")
            );
        }
    }

    fn fake_feature_packages(development: bool) -> (Vec<Value>, Vec<Value>) {
        let targets = [
            json!({"name":"fake_application","kind":["bin"],"crate_types":["bin"],"src_path":"/fake_src/main.rs"}),
            json!({"name":"fake_core","kind":["lib"],"crate_types":["lib"],"src_path":"/fake_core/lib.rs"}),
            json!({"name":"fake_exchange","kind":["lib"],"crate_types":["lib"],"src_path":"/fake_exchange/lib.rs"}),
        ];
        let packages = vec![
            json!({"id":"fake_application_package","targets":[targets[0]],"features":{
                "default":["fake_platform"],"fake_platform":["fake_dependency/fake_support"],
                "insecure-bidders":["fake_core/insecure-bidders"],"fake_unrequested":[]}}),
            json!({"id":"fake_core_package","targets":[targets[1]]}),
            json!({"id":"fake_exchange_package","targets":[targets[2]]}),
        ];
        let forwarded = if development {
            json!(["insecure-bidders"])
        } else {
            json!([])
        };
        let mut app_features = vec!["default", "fake_platform"];
        if development {
            app_features.push("insecure-bidders");
        }
        let mut rows = messages(development, false);
        rows[2]["target"] = targets[0].clone();
        rows[2]["features"] = json!(app_features);
        rows.insert(
            3,
            json!({"reason":"compiler-artifact","package_id":"fake_core_package",
            "target":targets[1],"features":forwarded}),
        );
        rows.insert(
            4,
            json!({"reason":"compiler-artifact","package_id":"fake_exchange_package",
            "target":targets[2],"features":forwarded}),
        );
        (packages, rows)
    }

    fn fake_check_feature_closure(
        packages: &[Value],
        rows: &[Value],
        development: bool,
    ) -> Result<(), String> {
        let mut app_features = BTreeSet::from(["default".to_owned(), "fake_platform".to_owned()]);
        let mut forwarded = BTreeSet::new();
        if development {
            app_features.insert("insecure-bidders".to_owned());
            forwarded.insert("insecure-bidders".to_owned());
        }
        let expected = [app_features, forwarded.clone(), forwarded];
        let checks: Vec<_> = packages
            .iter()
            .zip(expected)
            .map(|(package, features)| super::FeaturePackage {
                package_id: package["id"].as_str().unwrap(),
                target: &package["targets"][0],
                features,
            })
            .collect();
        let mut bytes = Vec::new();
        for row in rows {
            serde_json::to_writer(&mut bytes, row).map_err(|error| error.to_string())?;
            bytes.push(b'\n');
        }
        super::compiled_feature_closure(&bytes, &checks)
    }

    #[test]
    fn fake_compiled_feature_closure_accepts_exact_lane_features() {
        for development in [false, true] {
            let (packages, rows) = fake_feature_packages(development);
            inspect(&rows, development, false).expect("valid application build products");
            assert_eq!(
                fake_check_feature_closure(&packages, &rows, development),
                Ok(())
            );
            let mut expected = BTreeSet::from(["default".to_owned(), "fake_platform".to_owned()]);
            if development {
                expected.insert("insecure-bidders".to_owned());
            }
            assert_eq!(
                super::application_features(&packages[0], development),
                Ok(expected)
            );
        }
    }

    #[test]
    fn fake_compiled_feature_closure_refuses_each_missing_forward_with_bidder_enabled() {
        let (packages, original) = fake_feature_packages(true);
        for index in [2, 3, 4] {
            let mut rows = original.clone();
            rows[index]["features"]
                .as_array_mut()
                .unwrap()
                .retain(|feature| feature != "insecure-bidders");
            inspect(&rows, true, false).expect("bidder remains staged and insecure");
            assert_eq!(
                fake_check_feature_closure(&packages, &rows, true),
                Err("application compiled feature closure mismatch".to_owned()),
                "forwarding row {index}"
            );
        }
    }

    #[test]
    fn fake_compiled_feature_closure_refuses_missing_foreign_duplicate_and_wrong_target_rows() {
        let (packages, original) = fake_feature_packages(true);
        let mut rows = original.clone();
        rows.remove(3);
        assert_eq!(
            fake_check_feature_closure(&packages, &rows, true),
            Err("application compiled feature closure missing".to_owned())
        );
        let mut foreign_rows = original.clone();
        foreign_rows[3]["package_id"] = json!("fake_foreign_core_package");
        assert_eq!(
            fake_check_feature_closure(&packages, &foreign_rows, true),
            Err("application compiled feature closure missing".to_owned())
        );
        let mut duplicate_rows = original.clone();
        duplicate_rows.push(duplicate_rows[3].clone());
        assert_eq!(
            fake_check_feature_closure(&packages, &duplicate_rows, true),
            Err("ambiguous application feature closure".to_owned())
        );
        let mut wrong_target_rows = original;
        wrong_target_rows[3]["target"]["src_path"] = json!("/fake_foreign_core/lib.rs");
        assert_eq!(
            fake_check_feature_closure(&packages, &wrong_target_rows, true),
            Err("application compiled target differs from Cargo metadata".to_owned())
        );
    }

    #[test]
    fn fake_compiled_feature_closure_refuses_extra_duplicate_and_secure_insecure_flags() {
        for development in [false, true] {
            let (packages, original) = fake_feature_packages(development);
            for features in [
                json!(["insecure-bidders", "fake_unrequested"]),
                json!(["insecure-bidders", "insecure-bidders"]),
            ] {
                let mut rows = original.clone();
                rows[3]["features"] = features;
                assert_eq!(
                    fake_check_feature_closure(&packages, &rows, development),
                    Err("application compiled feature closure mismatch".to_owned())
                );
            }
        }
        let (packages, mut rows) = fake_feature_packages(false);
        rows[4]["features"] = json!(["insecure-bidders"]);
        assert_eq!(
            fake_check_feature_closure(&packages, &rows, false),
            Err("application compiled feature closure mismatch".to_owned())
        );
    }

    #[test]
    fn fake_application_features_distinguish_absent_and_declared_empty_defaults() {
        for (features, expected) in [
            (
                json!({"insecure-bidders":["fake_core/insecure-bidders"]}),
                Vec::<&str>::new(),
            ),
            (
                json!({"default":[],"insecure-bidders":["fake_core/insecure-bidders"]}),
                vec!["default"],
            ),
        ] {
            let package = json!({"features":features});
            assert_eq!(
                super::application_features(&package, false).unwrap(),
                expected
                    .iter()
                    .map(|feature| (*feature).to_owned())
                    .collect()
            );
            let mut development: BTreeSet<_> = expected
                .iter()
                .map(|feature| (*feature).to_owned())
                .collect();
            development.insert("insecure-bidders".to_owned());
            assert_eq!(super::application_features(&package, true), Ok(development));
        }
    }

    #[test]
    fn fake_compiled_feature_closure_requires_declared_empty_default() {
        for development in [false, true] {
            let (mut packages, mut rows) = fake_feature_packages(development);
            packages[0]["features"]["default"] = json!([]);
            let mut expected = BTreeSet::from(["default".to_owned()]);
            let mut forwarded = BTreeSet::new();
            if development {
                expected.insert("insecure-bidders".to_owned());
                forwarded.insert("insecure-bidders".to_owned());
            }
            assert_eq!(
                super::application_features(&packages[0], development),
                Ok(expected.clone())
            );
            rows[2]["features"] = json!(expected);
            let checks: Vec<_> = packages
                .iter()
                .zip([expected, forwarded.clone(), forwarded])
                .map(|(package, features)| super::FeaturePackage {
                    package_id: package["id"].as_str().unwrap(),
                    target: &package["targets"][0],
                    features,
                })
                .collect();
            for remove_default in [false, true] {
                let mut changed = rows.clone();
                if remove_default {
                    changed[2]["features"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|feature| feature != "default");
                }
                inspect(&changed, development, false)
                    .expect("bidder feature lane remains unchanged");
                let mut bytes = Vec::new();
                for row in &changed {
                    serde_json::to_writer(&mut bytes, row).unwrap();
                    bytes.push(b'\n');
                }
                assert_eq!(
                    super::compiled_feature_closure(&bytes, &checks),
                    if remove_default {
                        Err("application compiled feature closure mismatch".to_owned())
                    } else {
                        Ok(())
                    }
                );
            }
        }
    }

    #[test]
    fn fake_build_arguments_select_application_and_bidder_with_unqualified_features() {
        for application in super::APPLICATIONS {
            for development in [false, true] {
                let arguments = super::build_arguments(application, development);
                let packages: Vec<_> = arguments
                    .windows(2)
                    .filter_map(|pair| match pair {
                        ["-p", package] => Some(*package),
                        _ => None,
                    })
                    .collect();
                assert_eq!(packages, [application.package, "auctioneer-bidder"]);
                let features: Vec<_> = arguments
                    .windows(2)
                    .filter_map(|pair| match pair {
                        ["--features", features] => Some(*features),
                        _ => None,
                    })
                    .collect();
                assert_eq!(
                    features,
                    [if development {
                        "schema-artifact-staging,insecure-bidders"
                    } else {
                        "schema-artifact-staging"
                    }]
                );
                assert!(!features[0].contains('/'));
            }
        }
    }

    #[test]
    fn fake_application_products_refuse_ambiguous_wasm_rlib_and_bad_paths() {
        let mut rows = messages(false, true);
        rows[2]["filenames"] = json!([
            "/fake-target/fake_application.wasm",
            "/fake-target/fake_other.wasm"
        ]);
        assert_eq!(
            inspect(&rows, false, true),
            Err("ambiguous application build products")
        );
        rows[2]["filenames"] = json!(["/fake-target/fake_application.rlib"]);
        assert_eq!(
            inspect(&rows, false, true),
            Err("application artifact missing")
        );
        rows[2]["target"]["kind"] = json!(["rlib"]);
        assert_eq!(
            inspect(&rows, false, true),
            Err("application artifact missing")
        );
        for path in ["fake_relative", "/fake-target/../fake_escape"] {
            let mut out_dir_rows = messages(false, false);
            out_dir_rows[1]["out_dir"] = json!(path);
            assert_eq!(
                inspect(&out_dir_rows, false, false),
                Err("invalid application artifact path")
            );
            let mut executable_rows = messages(false, false);
            executable_rows[2]["executable"] = json!(path);
            assert_eq!(
                inspect(&executable_rows, false, false),
                Err("invalid application artifact path")
            );
        }
        let mut wrong_names = messages(false, false);
        wrong_names[2]["filenames"] = json!(["/fake-target/fake_other"]);
        assert_eq!(
            inspect(&wrong_names, false, false),
            Err("application executable mismatch")
        );
        let mut wrong_executable = messages(false, true);
        wrong_executable[2]["target"]["kind"] = json!(["bin"]);
        wrong_executable[2]["executable"] = json!("/fake-target/fake_other.wasm");
        assert_eq!(
            inspect(&wrong_executable, false, true),
            Err("application executable mismatch")
        );
        let mut duplicate_features = messages(false, false);
        duplicate_features[0]["features"] =
            json!(["schema-artifact-staging", "schema-artifact-staging"]);
        assert_eq!(
            inspect(&duplicate_features, false, false),
            Err("application bidder feature mismatch")
        );
        assert_eq!(
            products(
                b"not json",
                "fake_app",
                "fake_bidder",
                "fake_target",
                false,
                false
            ),
            Err("invalid Cargo build messages")
        );
        assert_eq!(
            products(
                &vec![b' '; super::MAX_MESSAGES + 1],
                "fake_app",
                "fake_bidder",
                "fake_target",
                false,
                false
            ),
            Err("Cargo build messages exceed byte cap")
        );
    }

    fn expected_build(development: bool, guest: bool) -> ExpectedBuild<'static> {
        ExpectedBuild {
            target: if guest {
                "wasm32-wasip1"
            } else {
                "fake_native_target"
            },
            host: "fake_native_target",
            profile: if guest { "wasm-release" } else { "release" },
            compiler: "fake rustc -vV",
            compiler_path: Path::new("/fake_compiler/rustc"),
            out_dir: Path::new("/fake_target/build/fake_bidder/out"),
            development,
        }
    }

    fn evidence(expected: &ExpectedBuild<'_>) -> (Value, Vec<SchemaInventory>, Value, Vec<u8>) {
        let schema = SchemaInventory {
            witness: SchemaWitness {
                name: "fake_owner".into(),
                path: "fake_owner/schema.json".into(),
                source_path: "static/bidder-params/fake_owner.json".into(),
                source_blob: "fake_blob".into(),
                bytes: 2,
                sha256: digest(b"{}"),
            },
            audit: audit_schema(&json!({})).unwrap(),
        };
        let source =
            json!({"repository":"https://fake.invalid/source", "revision":"fake_revision"});
        let member = json!({"name":"fake_alias", "implementation":"fake_owner", "is_root":true,
            "ancestry":["fake_owner"], "effective_schema_owner":"fake_owner"});
        let selection =
            json!({"schemas":[{"witness":schema.witness,"source":source}], "members":[member]});
        let lane = if expected.development {
            "development"
        } else {
            "secure"
        };
        let mut cohort = json!({"schema_version":1_u32,"lane":lane,"selection":selection,
            "source":{"linked_or_serving_qualification":false,"published_roots_all_features":["fake_alias"],
                "planned_roots":["fake_alias"],"lanes":{}}});
        cohort["source"]["lanes"][lane] = selection;
        let rust = quote::quote! { const FAKE_STAGING: bool = false; }
            .to_string()
            .into_bytes();
        let mut witness = serde_json::to_value(&schema.witness).unwrap();
        witness["repository"] = source["repository"].clone();
        witness["revision"] = source["revision"].clone();
        let mut manifest = json!({"schema_version":1_u32,"kind":"unqualified_non_serving_schema_staging",
            "identity":{"qualification":"Unqualified","sha256":digest(b"fake_identity")},
            "linked_or_serving_qualification":false,"artifact_inspection":"pending",
            "build":{"enabled_features":enabled_features(expected.development),"target":expected.target,
                "profile":expected.profile,"compiler_verbose":expected.compiler,"encoded_rustflags":""},
            "build_environment":{"OUT_DIR":expected.out_dir,"RUSTC":expected.compiler_path,
                "TARGET":expected.target,"HOST":expected.host,"PROFILE":"release","OPT_LEVEL":"3",
                "DEBUG":if expected.profile == "release" {"true"} else {"false"},"CARGO_ENCODED_RUSTFLAGS":"",
                "CARGO_CFG_FEATURE":enabled_features(expected.development).join(",")},
            "schemas":[witness],"selection":{"schema_owners":["fake_owner"],"members":[{
                "name":"fake_alias","implementation":"fake_owner","is_root":true,
                "ancestry":["fake_owner"],"schema_owner":"fake_owner"}]},
            "published_roots_all_features":["fake_alias"],"planned_roots":["fake_alias"],"emitted_rust_sha256":digest(&rust)});
        for feature in enabled_features(expected.development) {
            manifest["build_environment"][format!(
                "CARGO_FEATURE_{}",
                feature.replace('-', "_").to_ascii_uppercase()
            )] = json!("1");
        }
        (manifest, vec![schema], cohort, rust)
    }

    #[test]
    fn fake_evidence_checks_feature_lane_and_profile_fields() {
        for development in [false, true] {
            for guest in [false, true] {
                let expected = expected_build(development, guest);
                let (manifest, schemas, cohort, rust) = evidence(&expected);
                check_build(&manifest, &expected).expect("valid staging build evidence");
                assert_eq!(
                    check_cohort(&manifest, &schemas, &cohort, development),
                    Ok(())
                );
                assert_eq!(
                    compare_generated(&manifest, &manifest, &rust, &rust),
                    Ok(())
                );
                let controls =
                    refusal_controls(&manifest, &schemas, &cohort, &expected, &rust).unwrap();
                assert_eq!(controls.len(), 8);
                assert!(
                    controls.iter().all(|row| row["rejected"] == true
                        && row["scope"] == "in_memory_evidence_mutation")
                );
            }
        }
    }

    #[test]
    fn fake_evidence_refuses_wrong_captured_environment_and_qualification() {
        let expected = expected_build(false, false);
        let (manifest, _, _, _) = evidence(&expected);
        for (field, value, error) in [
            (
                "OUT_DIR",
                json!("/fake_other/out"),
                "application staging OUT_DIR mismatch",
            ),
            (
                "CARGO_FEATURE_TEST_SUPPORT",
                json!("1"),
                "application staging feature mismatch",
            ),
            (
                "CARGO_CFG_FEATURE",
                json!("insecure-bidders,schema-artifact-staging"),
                "application staging feature mismatch",
            ),
            (
                "OPT_LEVEL",
                json!("0"),
                "application staging build environment mismatch",
            ),
            (
                "DEBUG",
                json!("false"),
                "application staging build environment mismatch",
            ),
            (
                "HOST",
                json!("fake_other_host"),
                "application staging build environment mismatch",
            ),
            (
                "RUSTC",
                json!("/fake_other/rustc"),
                "application staging build environment mismatch",
            ),
            (
                "RUSTC_WRAPPER",
                json!("fake_wrapper"),
                "application staging build environment mismatch",
            ),
            (
                "CARGO_PROFILE_RELEASE_OPT_LEVEL",
                json!("0"),
                "application staging build environment mismatch",
            ),
        ] {
            let mut changed = manifest.clone();
            changed["build_environment"][field] = value;
            assert_eq!(
                check_build(&changed, &expected).unwrap_err(),
                error,
                "{field}"
            );
        }
        let mut changed = manifest.clone();
        changed["identity"]["qualification"] = json!("fake_qualified");
        assert_eq!(
            check_build(&changed, &expected).unwrap_err(),
            "application staging must remain unqualified"
        );
        changed = manifest;
        changed["linked_or_serving_qualification"] = json!(true);
        assert_eq!(
            check_build(&changed, &expected).unwrap_err(),
            "application staging must remain unqualified"
        );
    }

    #[test]
    fn fake_evidence_refuses_alias_witness_lane_and_rust_field_changes() {
        let expected = expected_build(false, false);
        let (manifest, schemas, cohort, rust) = evidence(&expected);
        for (pointer, value, error) in [
            (
                "/selection/members/0/name",
                json!("fake_other_alias"),
                "application source cohort mismatch",
            ),
            (
                "/selection/schema_owners/0",
                json!("fake_other_owner"),
                "application source cohort mismatch",
            ),
            (
                "/schemas/0/source_blob",
                json!("fake_other_blob"),
                "application schema witness mismatch",
            ),
            (
                "/schemas/0/revision",
                json!("fake_other_revision"),
                "application schema witness mismatch",
            ),
        ] {
            let mut changed = manifest.clone();
            *changed.pointer_mut(pointer).unwrap() = value;
            assert_eq!(
                check_cohort(&changed, &schemas, &cohort, false).unwrap_err(),
                error,
                "{pointer}"
            );
        }
        let mut wrong_lane = cohort.clone();
        wrong_lane["lane"] = json!("development");
        assert_eq!(
            check_cohort(&manifest, &schemas, &wrong_lane, false).unwrap_err(),
            "application source cohort mismatch"
        );
        let mut stale_source = cohort.clone();
        stale_source["selection"]["schemas"][0]["witness"]["sha256"] = json!("fake_stale_source");
        stale_source["source"]["lanes"]["secure"] = stale_source["selection"].clone();
        assert_eq!(
            check_cohort(&manifest, &schemas, &stale_source, false).unwrap_err(),
            "application schema witness mismatch"
        );
        assert_eq!(
            check_cohort(&manifest, &[], &cohort, false).unwrap_err(),
            "application source cohort mismatch"
        );
        assert_eq!(
            compare_generated(&manifest, &manifest, b"fake_stale", &rust).unwrap_err(),
            "stale application staging Rust"
        );
        let mut stale_rust = manifest.clone();
        stale_rust["emitted_rust_sha256"] = json!("fake_stale_hash");
        assert_eq!(
            compare_generated(&stale_rust, &stale_rust, &rust, &rust).unwrap_err(),
            "stale application staging Rust"
        );
    }

    #[test]
    fn application_inspection_refuses_stale_cohort_before_compiler_or_cargo() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        assert_eq!(
            super::inspect(&workspace, &[], &json!({}), false).unwrap_err(),
            "stale application source cohort"
        );
    }

    #[test]
    fn fake_application_paths_refuse_escape_before_reading_files() {
        for path in ["/fake_target/../fake_escape", "/fake_other/fake_artifact"] {
            assert_eq!(
                super::contained(Path::new("/fake_target"), Path::new(path), false).unwrap_err(),
                "application path escapes build root"
            );
        }
    }
}
