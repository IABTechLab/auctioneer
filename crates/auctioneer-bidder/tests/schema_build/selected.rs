//! Selected explicit-cache probe artifacts, not application or resource qualification.

use super::{SchemaInventory, SourceMode};
use crate::schema_reference;
use proc_macro2::TokenStream;
use quote::quote;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;
use std::path::{Component, PathBuf};
use std::process::Command;
use std::str;
use std::time::Duration;
use std::{fs, path::Path};

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn membership(schemas: &[SchemaInventory], binding: &Value) -> Result<String, String> {
    if schemas.is_empty() {
        return Err("empty selected schema cohort".to_owned());
    }
    let mut names = BTreeSet::new();
    for schema in schemas {
        if !names.insert(&schema.witness.name) {
            return Err("duplicate selected schema identity".to_owned());
        }
    }
    let expected = binding
        .pointer("/selection/schemas")
        .and_then(Value::as_array)
        .ok_or("selected schema membership mismatch")?;
    let witnesses = schemas
        .iter()
        .map(|row| serde_json::to_value(&row.witness))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let claimed = expected
        .iter()
        .map(|row| row.get("witness").cloned())
        .collect::<Option<Vec<_>>>()
        .ok_or("selected schema membership mismatch")?;
    if witnesses != claimed
        || binding["schema_version"] != 1_u8
        || binding.pointer("/source/linked_or_serving_qualification") != Some(&Value::Bool(false))
    {
        return Err("selected schema membership mismatch".to_owned());
    }
    let binding_bytes = serde_json::to_vec(binding).map_err(|error| error.to_string())?;
    serde_json::to_string(&json!({"schema_version": 2,
        "kind": "selected_explicit_cache_probe",
        "binding_sha256": digest(&binding_bytes), "schemas": witnesses}))
    .map_err(|error| error.to_string())
}

fn schema_bytes(root: &Path, schema: &SchemaInventory) -> Result<Vec<u8>, String> {
    let relative = Path::new(&schema.witness.path);
    if relative.as_os_str().is_empty()
        || !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err("invalid selected schema path".to_owned());
    }
    let path = root.join(relative);
    let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() != schema.witness.bytes || metadata.len() > 16 * 1024 {
        return Err("selected schema bytes changed".to_owned());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(16 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if u64::try_from(bytes.len()).map_err(|error| error.to_string())? != schema.witness.bytes
        || digest(&bytes) != schema.witness.sha256
    {
        return Err("selected schema bytes changed".to_owned());
    }
    Ok(bytes)
}

fn verify_bytes(root: &Path, schemas: &[SchemaInventory]) -> Result<(), String> {
    for schema in schemas {
        schema_bytes(root, schema)?;
    }
    Ok(())
}

fn stage_inputs(
    root: &Path,
    schemas: &[SchemaInventory],
    destination: &Path,
) -> Result<(), String> {
    for schema in schemas {
        let bytes = schema_bytes(root, schema)?;
        let path = destination.join(&schema.witness.path);
        fs::create_dir_all(path.parent().ok_or("invalid selected schema path")?)
            .map_err(|error| error.to_string())?;
        fs::write(path, bytes).map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(crate) fn selected_workload_data(
    selected_names: &[&str],
    cases: &[schema_reference::Case],
) -> BTreeMap<String, Value> {
    let mut rows = BTreeMap::<String, Vec<Value>>::new();
    for case in cases {
        if !selected_names.contains(&case.schema.as_str()) {
            continue;
        }
        if serde_json::from_str::<Value>(&case.input).is_err() {
            assert!(
                !case.valid,
                "captured accepted input must be JSON: {} {:?} {}",
                case.schema, case.group, case.index
            );
            // Malformed JSON stays with the raw-input consumer, not cases().
            continue;
        }
        rows.entry(case.schema.clone()).or_default().push(json!({
            "id": format!("captured_{:?}_{}", case.group, case.index),
            "input_json": case.input,
            "valid": case.valid,
            "transforms": []
        }));
    }
    rows.into_iter()
        .map(|(name, schema_rows)| (name, json!({"rows": schema_rows})))
        .collect()
}

pub(crate) fn merge_selected_workloads(
    selected_names: &[&str],
    captured: &BTreeMap<String, Value>,
    authored: &BTreeMap<String, Value>,
) -> Result<BTreeMap<String, Value>, String> {
    let mut result = BTreeMap::<String, Vec<Value>>::new();
    for (origin, workloads) in [("captured", captured), ("authored", authored)] {
        for (name, data) in workloads {
            if !selected_names.contains(&name.as_str()) {
                continue;
            }
            let rows = data
                .get("rows")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("malformed {origin} workload rows for {name}"))?;
            for row in rows {
                let error = format!("malformed {origin} workload row for {name}");
                let id = row
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty())
                    .ok_or_else(|| error.clone())?;
                if row.get("valid").and_then(Value::as_bool).is_none()
                    || row.get("transforms").and_then(Value::as_array).is_none()
                {
                    return Err(error);
                }
                match row.get("input_json") {
                    Some(Value::String(input)) => {
                        serde_json::from_str::<Value>(input).map_err(|_error| error.clone())?;
                    }
                    None if row.get("input").is_some() => {}
                    _ => return Err(error),
                }
                let mut emitted = row.clone();
                if origin == "authored" {
                    emitted["id"] = Value::String(format!("authored_{id}"));
                }
                result.entry(name.clone()).or_default().push(emitted);
            }
        }
    }
    Ok(result
        .into_iter()
        .map(|(name, rows)| (name, json!({"rows": rows})))
        .collect())
}

fn source(
    root: &Path,
    schemas: &[SchemaInventory],
    binding: &Value,
    authored: &BTreeMap<String, Value>,
    semantics: Option<&str>,
) -> Result<(String, String), String> {
    let membership = membership(schemas, binding)?;
    verify_bytes(root, schemas)?;
    let mode = SourceMode::Explicit;
    let mut generated = super::source_prelude(mode);
    let captured = schema_reference::captured()
        .map_err(|error| format!("selected workload capture: {error:?}"))?;
    let names = schemas
        .iter()
        .map(|schema| schema.witness.name.as_str())
        .collect::<Vec<_>>();
    let bare_data = super::bare::captured_data(&names, &captured.cases)?;
    let workloads = merge_selected_workloads(
        &names,
        &selected_workload_data(&names, &captured.cases),
        authored,
    )?;
    let workload_rows = workloads.iter().map(|(name, data)| {
        let name_literal = super::literal(name);
        let data_literal = super::literal(&data.to_string());
        quote! { (#name_literal, #data_literal) }
    });
    // Captured and authored workloads are native-only; alternate schema-control
    // wrappers and unselected workloads remain excluded from this cohort.
    generated.extend(quote! {
        #[cfg(not(target_arch = "wasm32"))]
        const WORKLOAD_DATA: &[(&str, &str)] = &[#(#workload_rows,)*];
    });
    let mut rows = TokenStream::new();
    let mut initializers = TokenStream::new();
    let mut variants = TokenStream::new();
    let mut layouts = TokenStream::new();
    for (index, schema) in schemas.iter().enumerate() {
        let primary = super::primary_schema(root, schema, index, mode)?;
        generated.extend(primary.body);
        rows.extend(primary.row);
        initializers.extend(primary.initializer);
        variants.extend(primary.variant);
        layouts.extend(primary.layout);
    }
    generated.extend(super::dispatch_tables(
        mode,
        &super::literal(&membership),
        &rows,
        &variants,
        &layouts,
    ));
    generated.extend(super::source_epilogue(
        mode,
        &initializers,
        &TokenStream::new(),
        &TokenStream::new(),
        Some(&bare_data),
    )?);
    if let Some(data) = semantics {
        generated.extend(super::bare::semantic_source(data));
    }
    let text = generated.to_string();
    if text.len() > super::MAX_SOURCE_BYTES {
        return Err("generated source limit".to_owned());
    }
    Ok((text, membership))
}

pub(super) fn probe_compiler() -> Result<(PathBuf, String), String> {
    let output = super::successful(
        Command::new("rustc")
            .args(["--print", "sysroot"])
            .current_dir(super::root()),
        Duration::from_secs(10),
    )?;
    let sysroot = str::from_utf8(&output.stdout).map_err(|error| error.to_string())?;
    let compiler = fs::canonicalize(Path::new(sysroot.trim()).join("bin/rustc"))
        .map_err(|error| error.to_string())?;
    let version = super::successful(Command::new(&compiler).arg("-vV"), Duration::from_secs(10))?;
    let compiler_version = String::from_utf8(version.stdout).map_err(|error| error.to_string())?;
    Ok((compiler, compiler_version))
}

fn build_targets(
    directory: &Path,
    target_dir: &Path,
    compiler: &Path,
    marker: &str,
    count: usize,
) -> Result<(Vec<super::LinkReport>, Vec<Value>), String> {
    let engine =
        super::schema_regex_vendor::verify(&super::root()).map_err(|error| error.to_string())?;
    let mut reports = Vec::new();
    let mut targets = Vec::new();
    for target in super::TARGETS {
        let mut command = super::cargo(directory);
        command
            .env("RUSTC", compiler)
            .args([
                "build",
                "--locked",
                "--offline",
                "-j",
                "2",
                "--bin",
                super::PACKAGE,
                "--target-dir",
            ])
            .arg(target_dir);
        let profile = if target == "native" {
            "release"
        } else {
            "wasm-release"
        };
        let relative = if target == "native" {
            command.arg("--release");
            PathBuf::from(profile).join(super::PACKAGE)
        } else {
            command.args(["--profile", profile, "--target", target]);
            PathBuf::from(target)
                .join(profile)
                .join(format!("{}.wasm", super::PACKAGE))
        };
        super::successful(&mut command, super::BUILD_TIMEOUT)
            .map_err(|error| format!("{target}: {error}"))?;
        let artifact =
            super::artifact_with_count(&target_dir.join(relative), marker, target, count)
                .map_err(|error| format!("{target}: {error}"))?;
        let mut metadata = super::cargo(directory);
        metadata.env("RUSTC", compiler).args([
            "metadata",
            "--locked",
            "--offline",
            "--format-version",
            "1",
        ]);
        if target != "native" {
            metadata.args(["--filter-platform", target]);
        }
        let metadata_output = super::successful(&mut metadata, Duration::from_secs(60))?;
        let features = super::schema_regex_vendor::verify_metadata(
            &metadata_output.stdout,
            &engine.canonical_root,
        )?;
        targets.push(json!({"target": target, "profile": profile,
            "validators": artifact.validators, "artifact_bytes": artifact.artifact_bytes,
            "sha256": artifact.sha256, "regex_features": features,
            "wasm_static_declarations": artifact.wasm,
            "native_sections": (target == "native").then_some("not_measured"),
            "guest_execution_at_link_time": (target != "native").then_some("not_executed")}));
        reports.push(artifact);
    }
    Ok((reports, targets))
}

fn verified_bare_imports(
    target_dir: &Path,
    reports: &[super::LinkReport],
    bare_data: &str,
) -> Result<Vec<(String, String)>, String> {
    let bare_path = target_dir
        .join("wasm32-unknown-unknown/wasm-release")
        .join(format!("{}.wasm", super::PACKAGE));
    let mut bare_bytes = Vec::new();
    fs::File::open(bare_path)
        .map_err(|error| error.to_string())?
        .take(super::MAX_ARTIFACT_BYTES + 1)
        .read_to_end(&mut bare_bytes)
        .map_err(|error| error.to_string())?;
    let bare_artifact = reports
        .iter()
        .find(|row| row.target == "wasm32-unknown-unknown")
        .ok_or("missing bare artifact")?;
    if u64::try_from(bare_bytes.len()).map_err(|error| error.to_string())?
        != bare_artifact.artifact_bytes
        || digest(&bare_bytes) != bare_artifact.sha256
    {
        return Err("bare artifact identity changed".to_owned());
    }
    super::bare::verify_data(&bare_bytes, bare_data)?;
    super::bare::trapped_imports(&bare_bytes)
}

#[derive(Clone, Copy)]
pub(crate) struct SemanticInputs<'a> {
    pub data: &'a str,
    pub witness: &'a Value,
}

pub(crate) fn link(
    root: &Path,
    schemas: &[SchemaInventory],
    binding: &Value,
    semantics: Option<SemanticInputs<'_>>,
) -> Result<(super::Catalog, Value), String> {
    membership(schemas, binding)?;
    verify_bytes(root, schemas)?;
    let authored = super::workload_data_for(root, schemas)?;
    let target_dir = super::root().join("target/schema-selected-probe");
    let lease = super::acquire_lease(&target_dir)?;
    let directory = super::prepare_workspace()?;
    let staged_root = directory.path().join("schemas");
    stage_inputs(root, schemas, &staged_root)?;
    let (generated, marker) = source(
        &staged_root,
        schemas,
        binding,
        &authored,
        semantics.as_ref().map(|inputs| inputs.data),
    )?;
    let generated_sha256 = digest(generated.as_bytes());
    fs::write(directory.path().join("src/main.rs"), generated)
        .map_err(|error| error.to_string())?;
    super::verify_probe_workspace(directory.path())?;
    let (compiler, compiler_version) = probe_compiler()?;
    let (mut reports, targets) = build_targets(
        directory.path(),
        &target_dir,
        &compiler,
        &marker,
        schemas.len(),
    )?;
    let captured =
        schema_reference::captured().map_err(|error| format!("bare capture: {error:?}"))?;
    let names = schemas
        .iter()
        .map(|row| row.witness.name.as_str())
        .collect::<Vec<_>>();
    let bare_data = super::bare::captured_data(&names, &captured.cases)?;
    let trapped_imports = verified_bare_imports(&target_dir, &reports, &bare_data)?;
    if let Some(inputs) = &semantics {
        verified_bare_imports(&target_dir, &reports, inputs.data)?;
    }
    let native = target_dir.join("release").join(super::PACKAGE);
    let output = super::successful(
        Command::new(&native).arg("inventory"),
        Duration::from_secs(10),
    )?;
    let parsed: super::NativeInventory =
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    let expected: Vec<_> = schemas.iter().map(|row| row.witness.name.clone()).collect();
    if parsed.names != expected
        || parsed.results.len() != expected.len()
        || parsed.membership != marker
    {
        return Err("native dispatch membership mismatch".to_owned());
    }
    let native_report = reports.first_mut().ok_or("missing native probe artifact")?;
    native_report.names = parsed.names;
    native_report.results = parsed.results;
    // Recheck inputs after child builds: a concurrent edit must not produce a
    // report whose source witnesses describe different schema bytes.
    verify_bytes(root, schemas)?;
    let manifest =
        fs::read(directory.path().join("Cargo.toml")).map_err(|error| error.to_string())?;
    let lock = fs::read(directory.path().join("Cargo.lock")).map_err(|error| error.to_string())?;
    let mut probe_sources = BTreeMap::new();
    for entry in fs::read_dir(directory.path().join("src")).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        let relative = path
            .strip_prefix(directory.path())
            .map_err(|error| error.to_string())?;
        let bytes = fs::read(&path).map_err(|error| error.to_string())?;
        probe_sources.insert(relative.to_path_buf(), digest(&bytes));
    }
    probe_sources.insert(
        PathBuf::from("build.rs"),
        digest(&fs::read(directory.path().join("build.rs")).map_err(|error| error.to_string())?),
    );
    let report = json!({"schema_version": 1_u8, "kind": "selected_schema_link_probe",
        "application_or_resource_qualification": false,
        "binding": binding, "membership": serde_json::from_str::<Value>(&marker).map_err(|error| error.to_string())?,
        "generated_source_sha256": generated_sha256,
        "probe_sources_sha256": probe_sources,
        "probe_manifest_sha256": digest(&manifest), "probe_lock_sha256": digest(&lock),
        "bare_input_data": {"schema_version":1_u8,"sha256":digest(bare_data.as_bytes()),"bytes":bare_data.len()},
        "selected_semantic_inputs": semantics.as_ref().map(|inputs| inputs.witness),
        "bare_semantic_data": semantics.as_ref().map(|inputs| json!({"schema_version":1_u8,"sha256":digest(inputs.data.as_bytes()),"bytes":inputs.data.len()})),
        "bare_trapped_imports": trapped_imports,
        "feature_scope": "selected lane schema owners; not an adapter application build",
        "probe_overhead": "CLI, compiled captures, parsing and observation runtime included; not adapter residency",
        "compiler": {"executable": compiler, "version": compiler_version},
        "targets": targets,
        "pending_gaps": ["guest_execution", "native_sections", "serving_integration", "resident_resource_bounds", "all_target_qualification"]});
    Ok((
        super::Catalog {
            reports,
            directory,
            native,
            target_dir,
            _lease: lease,
            _inputs: None,
        },
        report,
    ))
}

#[cfg(test)]
mod tests {
    use super::{merge_selected_workloads, selected_workload_data, source};
    use crate::schema_qualification::{SchemaInventory, fixture};
    use crate::schema_reference::{self, Case, Group};
    use serde_json::{Value, json};
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;

    fn fake_case(schema: &str, group: Group, index: usize, input: &str, valid: bool) -> Case {
        Case {
            schema: schema.to_owned(),
            group,
            index,
            input: input.to_owned(),
            valid,
            source_offset: 100_usize.checked_add(index).unwrap(),
        }
    }

    #[test]
    fn fake_selected_workloads_preserve_captured_booleans_independently_of_group() {
        let cases = [
            fake_case("fake_selected", Group::Valid, 4, "{}", true),
            fake_case("fake_selected", Group::Valid, 5, "{}", false),
            fake_case("fake_selected", Group::Invalid, 6, "{}", true),
            fake_case("fake_selected", Group::Invalid, 7, "{}", false),
        ];
        let actual = selected_workload_data(&["fake_selected"], &cases);
        assert_eq!(
            actual,
            BTreeMap::from([(
                "fake_selected".to_owned(),
                json!({"rows": [
                    {"id": "captured_Valid_4", "input_json": "{}", "valid": true, "transforms": []},
                    {"id": "captured_Valid_5", "input_json": "{}", "valid": false, "transforms": []},
                    {"id": "captured_Invalid_6", "input_json": "{}", "valid": true, "transforms": []},
                    {"id": "captured_Invalid_7", "input_json": "{}", "valid": false, "transforms": []}
                ]})
            )])
        );
    }

    #[test]
    fn fake_selected_workloads_skip_rejected_malformed_json() {
        let cases = [
            fake_case("fake_selected", Group::Valid, 0, "null", true),
            fake_case("fake_selected", Group::Valid, 1, "{", false),
            fake_case("fake_selected", Group::Invalid, 2, "{} trailing", false),
            fake_case("fake_selected", Group::Invalid, 3, "false", false),
        ];
        let actual = selected_workload_data(&["fake_selected"], &cases);
        assert_eq!(
            actual,
            BTreeMap::from([(
                "fake_selected".to_owned(),
                json!({"rows": [
                    {"id": "captured_Valid_0", "input_json": "null", "valid": true, "transforms": []},
                    {"id": "captured_Invalid_3", "input_json": "false", "valid": false, "transforms": []}
                ]})
            )])
        );
    }

    #[test]
    #[should_panic(expected = "captured accepted input must be JSON")]
    fn fake_selected_workloads_refuse_accepted_malformed_json() {
        let cases = [fake_case("fake_selected", Group::Invalid, 0, "{", true)];
        selected_workload_data(&["fake_selected"], &cases);
    }

    #[test]
    fn fake_selected_workloads_filter_unselected_cases_independently_of_captured_boolean() {
        let cases = [
            fake_case("fake_unselected", Group::Valid, 0, "[]", true),
            fake_case("fake_selected", Group::Invalid, 1, "[]", false),
            fake_case("fake_unselected", Group::Invalid, 2, "[]", false),
            fake_case("fake_selected", Group::Valid, 3, "[]", true),
            fake_case("fake_unselected", Group::Valid, 4, "{", true),
            fake_case("fake_unselected", Group::Invalid, 5, "{} trailing", true),
        ];
        let actual = selected_workload_data(&["fake_selected"], &cases);
        assert_eq!(
            actual,
            BTreeMap::from([(
                "fake_selected".to_owned(),
                json!({"rows": [
                    {"id": "captured_Invalid_1", "input_json": "[]", "valid": false, "transforms": []},
                    {"id": "captured_Valid_3", "input_json": "[]", "valid": true, "transforms": []}
                ]})
            )])
        );
        assert!(selected_workload_data(&[], &cases).is_empty());
    }

    #[test]
    fn fake_selected_workloads_preserve_raw_bytes_order_and_repeated_cases() {
        let repeated = fake_case(
            "fake_zeta",
            Group::Invalid,
            9,
            " \n{ \"fake_key\": \"a\\nb\" } \t",
            false,
        );
        let cases = [
            repeated.clone(),
            fake_case("fake_alpha", Group::Valid, 8, "1", true),
            fake_case("fake_zeta", Group::Valid, 2, "\"fake_text\"", true),
            fake_case("fake_alpha", Group::Invalid, 1, "2", false),
            repeated,
        ];
        let actual = selected_workload_data(&["fake_zeta", "fake_alpha"], &cases);
        assert_eq!(
            actual,
            BTreeMap::from([
                (
                    "fake_alpha".to_owned(),
                    json!({"rows": [
                        {"id": "captured_Valid_8", "input_json": "1", "valid": true, "transforms": []},
                        {"id": "captured_Invalid_1", "input_json": "2", "valid": false, "transforms": []}
                    ]})
                ),
                (
                    "fake_zeta".to_owned(),
                    json!({"rows": [
                        {"id": "captured_Invalid_9", "input_json": " \n{ \"fake_key\": \"a\\nb\" } \t", "valid": false, "transforms": []},
                        {"id": "captured_Valid_2", "input_json": "\"fake_text\"", "valid": true, "transforms": []},
                        {"id": "captured_Invalid_9", "input_json": " \n{ \"fake_key\": \"a\\nb\" } \t", "valid": false, "transforms": []}
                    ]})
                )
            ])
        );
    }

    #[test]
    fn fake_selected_workloads_allow_selected_names_without_parseable_captures() {
        let cases = [
            fake_case("fake_present", Group::Valid, 0, "{}", true),
            fake_case("fake_malformed_only", Group::Invalid, 1, "{", false),
        ];
        let actual = selected_workload_data(
            &["fake_absent", "fake_malformed_only", "fake_present"],
            &cases,
        );
        assert_eq!(
            actual,
            BTreeMap::from([(
                "fake_present".to_owned(),
                json!({"rows": [
                    {"id": "captured_Valid_0", "input_json": "{}", "valid": true, "transforms": []}
                ]})
            )])
        );
        assert!(selected_workload_data(&["fake_absent"], &cases).is_empty());
        assert!(selected_workload_data(&["fake_malformed_only"], &cases).is_empty());
        assert!(selected_workload_data(&["fake_present"], &[]).is_empty());
    }

    #[test]
    fn fake_selected_authored_workloads_append_rows_with_distinct_origin_ids() {
        let captured = BTreeMap::from([(
            "fake_both".to_owned(),
            json!({"rows": [
                {"id": "captured_Valid_9", "input_json": "{}", "valid": false, "transforms": []},
                {"id": "captured_Invalid_1", "input_json": "[]", "valid": true, "transforms": []}
            ]}),
        )]);
        let authored = BTreeMap::from([(
            "fake_both".to_owned(),
            json!({"schema": "fake_both", "order": 7_u64, "rows": [
                {"id": "Valid_9", "input": {"fake_text": "a", "fake_values": []}, "valid": true,
                    "transforms": [{"op": "max_string", "pointer": "/fake_text", "character": "a", "invalid": false}]},
                {"id": "fake_tail", "input_json": " \n{ \"fake_values\": [] } \t", "valid": false,
                    "transforms": [{"op": "insert", "pointer": "", "key": "fake_flag", "value": false},
                        {"op": "max_collection", "pointer": "/fake_values", "invalid": true}]},
                {"id": "Valid_9", "input": {"fake_text": "a", "fake_values": []}, "valid": true,
                    "transforms": [{"op": "max_string", "pointer": "/fake_text", "character": "a", "invalid": false}]}
            ]}),
        )]);
        assert_eq!(
            merge_selected_workloads(&["fake_both"], &captured, &authored).unwrap(),
            BTreeMap::from([(
                "fake_both".to_owned(),
                json!({"rows": [
                    {"id": "captured_Valid_9", "input_json": "{}", "valid": false, "transforms": []},
                    {"id": "captured_Invalid_1", "input_json": "[]", "valid": true, "transforms": []},
                    {"id": "authored_Valid_9", "input": {"fake_text": "a", "fake_values": []}, "valid": true,
                        "transforms": [{"op": "max_string", "pointer": "/fake_text", "character": "a", "invalid": false}]},
                    {"id": "authored_fake_tail", "input_json": " \n{ \"fake_values\": [] } \t", "valid": false,
                        "transforms": [{"op": "insert", "pointer": "", "key": "fake_flag", "value": false},
                            {"op": "max_collection", "pointer": "/fake_values", "invalid": true}]},
                    {"id": "authored_Valid_9", "input": {"fake_text": "a", "fake_values": []}, "valid": true,
                        "transforms": [{"op": "max_string", "pointer": "/fake_text", "character": "a", "invalid": false}]}
                ]})
            )])
        );
    }

    #[test]
    fn fake_selected_authored_workloads_handle_independent_and_missing_origins() {
        let captured = BTreeMap::from([
            (
                "fake_capture_only".to_owned(),
                json!({"rows": [{"id": "captured_Invalid_2", "input_json": "null", "valid": false, "transforms": []}]}),
            ),
            ("fake_unselected".to_owned(), json!({"rows": false})),
        ]);
        let authored = BTreeMap::from([
            (
                "fake_authored_only".to_owned(),
                json!({"rows": [{"id": "fake_null", "input": null, "valid": true, "transforms": []}]}),
            ),
            ("fake_unselected".to_owned(), json!({"rows": false})),
        ]);
        let names = ["fake_missing", "fake_capture_only", "fake_authored_only"];
        assert_eq!(
            merge_selected_workloads(&names, &captured, &authored).unwrap(),
            BTreeMap::from([
                (
                    "fake_capture_only".to_owned(),
                    json!({"rows": [{"id": "captured_Invalid_2", "input_json": "null", "valid": false, "transforms": []}]}),
                ),
                (
                    "fake_authored_only".to_owned(),
                    json!({"rows": [{"id": "authored_fake_null", "input": null, "valid": true, "transforms": []}]}),
                )
            ])
        );
        assert!(
            merge_selected_workloads(&[], &captured, &authored)
                .unwrap()
                .is_empty()
        );
        assert!(
            merge_selected_workloads(&names, &BTreeMap::new(), &BTreeMap::new())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn fake_selected_authored_workloads_refuse_malformed_rows_without_dropping_them() {
        let good = json!({"id": "fake_good", "input": {}, "valid": true, "transforms": []});
        let malformed = [
            json!(false),
            json!({"input": {}, "valid": true, "transforms": []}),
            json!({"id": 1_u64, "input": {}, "valid": true, "transforms": []}),
            json!({"id": "", "input": {}, "valid": true, "transforms": []}),
            json!({"id": "fake_bad", "input": {}, "transforms": []}),
            json!({"id": "fake_bad", "input": {}, "valid": "false", "transforms": []}),
            json!({"id": "fake_bad", "input": {}, "valid": false}),
            json!({"id": "fake_bad", "input": {}, "valid": false, "transforms": {}}),
            json!({"id": "fake_bad", "valid": false, "transforms": []}),
            json!({"id": "fake_bad", "input_json": 1_u64, "valid": false, "transforms": []}),
            json!({"id": "fake_bad", "input_json": "{", "valid": false, "transforms": []}),
        ];
        assert_eq!(malformed.len(), 11);
        let empty = BTreeMap::new();
        for row in malformed {
            let data = BTreeMap::from([(
                "fake_selected".to_owned(),
                json!({"rows": [good.clone(), row.clone(), good.clone()]}),
            )]);
            for (captured, authored, expected) in [
                (
                    &data,
                    &empty,
                    "malformed captured workload row for fake_selected",
                ),
                (
                    &empty,
                    &data,
                    "malformed authored workload row for fake_selected",
                ),
            ] {
                assert_eq!(
                    merge_selected_workloads(&["fake_selected"], captured, authored).unwrap_err(),
                    expected,
                    "malformed selected row must fail: {row}"
                );
            }
        }
        for data in [json!({}), json!({"rows": false}), json!({"rows": {}})] {
            let authored = BTreeMap::from([("fake_selected".to_owned(), data)]);
            assert_eq!(
                merge_selected_workloads(&["fake_selected"], &BTreeMap::new(), &authored)
                    .unwrap_err(),
                "malformed authored workload rows for fake_selected"
            );
        }
    }

    #[test]
    fn fake_selected_authored_workloads_source_uses_explicit_data_with_staged_schemas() {
        let inputs = fixture::Fixture::new().unwrap();
        let snapshot = fixture::inventory(&inputs.bidders, &inputs.source).unwrap();
        let selected = [snapshot
            .schemas
            .iter()
            .find(|schema| schema.witness.name == "fake_numeric")
            .unwrap()
            .clone()];
        let staged = tempfile::tempdir().unwrap();
        super::stage_inputs(&snapshot.root, &selected, staged.path()).unwrap();
        let authored = BTreeMap::from([(
            "fake_numeric".to_owned(),
            json!({"rows": [{"id": "fake_explicit", "input": 7_u64, "valid": true, "transforms": []}]}),
        )]);
        let (generated, _) = source(
            staged.path(),
            &selected,
            &binding(&selected),
            &authored,
            None,
        )
        .unwrap();
        let syntax = syn::parse_file(&generated).unwrap();
        let table = syntax
            .items
            .iter()
            .find_map(|item| {
                if let syn::Item::Const(table) = item {
                    (table.ident == "WORKLOAD_DATA").then_some(table)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(table.attrs.len(), 1);
        let guard = &table.attrs[0];
        assert_eq!(
            quote::quote!(#guard).to_string(),
            quote::quote!(#[cfg(not(target_arch = "wasm32"))]).to_string()
        );
        let syn::Expr::Reference(borrow) = table.expr.as_ref() else {
            panic!("workloads must borrow a static array");
        };
        let syn::Expr::Array(rows) = borrow.expr.as_ref() else {
            panic!("workloads must be an array");
        };
        assert_eq!(rows.elems.len(), 1);
        let syn::Expr::Tuple(row) = &rows.elems[0] else {
            panic!("workload must be a tuple");
        };
        assert_eq!(row.elems.len(), 2);
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(name),
            ..
        }) = &row.elems[0]
        else {
            panic!("workload name must be a string literal");
        };
        assert_eq!(name.value(), "fake_numeric");
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(encoded),
            ..
        }) = &row.elems[1]
        else {
            panic!("workload data must be a string literal");
        };
        assert_eq!(
            serde_json::from_str::<Value>(&encoded.value()).unwrap(),
            json!({"rows": [{"id": "authored_fake_explicit", "input": 7_u64, "valid": true, "transforms": []}]})
        );
    }

    fn binding(schemas: &[SchemaInventory]) -> Value {
        json!({"schema_version": 1,
            "source": {"linked_or_serving_qualification": false},
            "selection": {"schemas": schemas.iter()
                .map(|row| json!({"witness": row.witness})).collect::<Vec<_>>()}})
    }

    #[test]
    fn fake_selected_generation_keeps_only_bound_validators_without_alternates() {
        let inputs = fixture::Fixture::new().unwrap();
        let snapshot = fixture::inventory(&inputs.bidders, &inputs.source).unwrap();
        let selected = &snapshot.schemas[..1];
        let record = binding(selected);
        let (generated, marker) =
            source(&snapshot.root, selected, &record, &BTreeMap::new(), None).unwrap();
        let syntax = syn::parse_file(&generated).unwrap();
        let validators = syntax
            .items
            .iter()
            .filter(|item| {
                matches!(item, syn::Item::Struct(value) if value.attrs.iter().any(|attr| {
                    attr.path().segments.last().is_some_and(|segment| segment.ident == "validator")
                }))
            })
            .count();
        assert_eq!(validators, 1);
        assert!(generated.contains("cache = explicit"));
        assert!(!generated.contains("formats_disabled"));
        assert!(!generated.contains("no_onepass"));
        let embedded = syntax
            .items
            .iter()
            .find_map(|item| {
                let syn::Item::Const(constant) = item else {
                    return None;
                };
                if constant.ident != "BARE_CASES" {
                    return None;
                }
                let syn::Expr::Lit(expression) = constant.expr.as_ref() else {
                    return None;
                };
                let syn::Lit::Str(literal) = &expression.lit else {
                    return None;
                };
                Some(literal.value())
            })
            .expect("bare captures compiled into selected probe");
        let cases = schema_reference::captured().unwrap();
        assert_eq!(
            embedded,
            super::super::bare::captured_data(
                &selected
                    .iter()
                    .map(|row| row.witness.name.as_str())
                    .collect::<Vec<_>>(),
                &cases.cases
            )
            .unwrap()
        );
        let membership: Value = serde_json::from_str(&marker).unwrap();
        assert_eq!(membership["schema_version"], 2_u8);
        assert_eq!(
            membership["binding_sha256"],
            super::digest(&serde_json::to_vec(&record).unwrap())
        );
        assert_eq!(membership["schemas"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn fake_selected_witness_mismatch_refuses_before_schema_reads() {
        let inputs = fixture::Fixture::new().unwrap();
        let snapshot = fixture::inventory(&inputs.bidders, &inputs.source).unwrap();
        let selected = &snapshot.schemas[..1];
        let mut record = binding(selected);
        record["selection"]["schemas"][0]["witness"]["sha256"] = json!("fake_wrong");
        assert_eq!(
            source(
                Path::new("/fake_unread_source"),
                selected,
                &record,
                &BTreeMap::new(),
                None
            ),
            Err("selected schema membership mismatch".to_owned())
        );
    }

    #[test]
    fn fake_selected_empty_and_duplicate_owners_refuse() {
        assert_eq!(
            source(
                Path::new("/fake_unread_source"),
                &[],
                &binding(&[]),
                &BTreeMap::new(),
                None
            ),
            Err("empty selected schema cohort".to_owned())
        );
        assert_eq!(
            super::link(Path::new("/fake_unread_source"), &[], &binding(&[]), None)
                .err()
                .as_deref(),
            Some("empty selected schema cohort")
        );
        let inputs = fixture::Fixture::new().unwrap();
        let snapshot = fixture::inventory(&inputs.bidders, &inputs.source).unwrap();
        let duplicate = vec![snapshot.schemas[0].clone(), snapshot.schemas[0].clone()];
        assert_eq!(
            source(
                &snapshot.root,
                &duplicate,
                &binding(&duplicate),
                &BTreeMap::new(),
                None
            ),
            Err("duplicate selected schema identity".to_owned())
        );
    }

    #[test]
    fn fake_selected_marker_binds_but_does_not_embed_unselected_source_metadata() {
        let inputs = fixture::Fixture::new().unwrap();
        let snapshot = fixture::inventory(&inputs.bidders, &inputs.source).unwrap();
        let selected = &snapshot.schemas[..1];
        let original = binding(selected);
        let mut changed = original.clone();
        changed["source"]["unselected_note"] = json!("fake_unselected_note");
        let first = source(&snapshot.root, selected, &original, &BTreeMap::new(), None)
            .unwrap()
            .1;
        let second = source(&snapshot.root, selected, &changed, &BTreeMap::new(), None)
            .unwrap()
            .1;
        assert_ne!(first, second);
        assert!(!second.contains("fake_unselected_note"));
    }

    #[test]
    fn fake_selected_staging_keeps_verified_inputs_independent_of_later_source_edits() {
        let inputs = fixture::Fixture::new().unwrap();
        let snapshot = fixture::inventory(&inputs.bidders, &inputs.source).unwrap();
        let selected = &snapshot.schemas[..1];
        let staged = tempfile::tempdir().unwrap();
        super::stage_inputs(&snapshot.root, selected, staged.path()).unwrap();
        let original = snapshot.root.join(&selected[0].witness.path);
        fs::write(
            &original,
            serde_json::to_vec(&json!({"type": "null"})).unwrap(),
        )
        .unwrap();
        let (generated, _) = source(
            staged.path(),
            selected,
            &binding(selected),
            &BTreeMap::new(),
            None,
        )
        .unwrap();
        let compiler_path = staged
            .path()
            .join(&selected[0].witness.path)
            .canonicalize()
            .unwrap();
        assert!(generated.contains(compiler_path.to_str().unwrap()));
        assert!(!generated.contains(original.to_str().unwrap()));
        assert_eq!(
            source(
                &snapshot.root,
                selected,
                &binding(selected),
                &BTreeMap::new(),
                None
            ),
            Err("selected schema bytes changed".to_owned())
        );
    }

    #[test]
    fn fake_selected_staging_refuses_changed_bytes_and_escaping_paths() {
        let inputs = fixture::Fixture::new().unwrap();
        let snapshot = fixture::inventory(&inputs.bidders, &inputs.source).unwrap();
        let mut selected = snapshot.schemas[..1].to_vec();
        let staged = tempfile::tempdir().unwrap();
        selected[0].witness.sha256 = "fake_changed".to_owned();
        assert_eq!(
            super::stage_inputs(&snapshot.root, &selected, staged.path()),
            Err("selected schema bytes changed".to_owned())
        );
        for path in ["../fake_escape.json", "/fake_absolute.json"] {
            selected[0].witness.path = path.to_owned();
            assert_eq!(
                super::stage_inputs(&snapshot.root, &selected, staged.path()),
                Err("invalid selected schema path".to_owned())
            );
        }
    }
}
