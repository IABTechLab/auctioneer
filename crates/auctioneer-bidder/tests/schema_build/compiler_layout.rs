//! Compiler-emitted static layout evidence, never execution or resource qualification.

use super::native_dwarf::{Reader as NativeReader, Relocations};
use flate2::read::GzDecoder;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::str::from_utf8;
use std::time::Duration;

const MAX_BYTES: u64 = 64 << 20;
const MAX_FILES: usize = 512;
const MAX_MESSAGES: usize = 4 << 20_u32;
const TIMEOUT: Duration = Duration::from_secs(600);

/// Features must equal the normal graph's leaf receipt and its replayed cfgs.
pub struct Request<'a> {
    pub workspace: &'a Path,
    pub package: &'a str,
    /// `native` or an explicit target triple.
    pub target: &'a str,
    pub features: &'a [&'a str],
    pub qualified_types: &'a [&'a str],
}

/// The callback is invoked only after compilation and source receipts reconcile.
pub fn inspect(
    requested: &Request<'_>,
    parse: impl FnOnce(&[u8], &[&str]) -> Result<Value, String>,
) -> Result<Value, String> {
    validate_request(requested)?;
    let workspace = fs::canonicalize(requested.workspace).map_err(|error| error.to_string())?;
    let canonical_request = request_at(requested, &workspace);
    let request = &canonical_request;
    let target_dir = workspace.join("target");
    let _lease = super::acquire_lease(&target_dir)?;
    let workspace_before = workspace_inputs(&workspace)?;
    let (compiler, compiler_version) = super::selected::probe_compiler()?;
    let tool_versions = read_file(&workspace.join(".tool-versions"), 16 << 10)?;
    let compiler_pin = verify_compiler_pin(
        from_utf8(&tool_versions).map_err(|error| error.to_string())?,
        &compiler_version,
    )?;
    let (target, host) = build_target(request.target, &compiler_version)?;
    let compiler_hash = digest(&read_file(&compiler, MAX_BYTES)?);
    let (metadata, metadata_output) = metadata_for(request, &compiler, target)?;
    let package = selected_package(&metadata, request.package)?;
    let cargo_target = library_target(package)?;
    verify_library_source(package, cargo_target)?;
    let (source, source_before) = source_inputs(&workspace, package)?;
    let syntax = if request.package == "regex-automata" {
        Some(selected_package(&metadata, "regex-syntax")?)
    } else {
        None
    };
    let syntax_before = syntax
        .map(|row| source_inputs(&workspace, row))
        .transpose()?;
    let temporary = tempfile::tempdir().map_err(|error| error.to_string())?;
    let object = temporary.path().join("layout.fresh.o");
    let prepared = prepare_graph(request, &compiler, target, &metadata, package)?;
    let externs_before = extern_inputs(&prepared.externs)?;
    let mut command = direct_command(
        request,
        &compiler,
        target,
        cargo_target,
        &object,
        &prepared.deps_dir,
        &prepared.externs,
    )?;
    verify_command(&command, &compiler, target, &object)?;
    if fs::symlink_metadata(&object).is_ok() {
        return Err("compiler layout output is not fresh".into());
    }
    let argv = command_args(&command)?;
    let direct_output = super::successful(&mut command, TIMEOUT)?;
    let profile = profile(target);
    check_outputs(temporary.path())?;
    let object_bytes = read_file(&object, MAX_BYTES)?;
    let object_hash = digest(&object_bytes);
    let (payload, link) = if target.starts_with("wasm32-") {
        link_wasm(temporary.path(), &object, &compiler, host)?
    } else {
        (object_bytes, Value::Null)
    };
    let abi = payload_identity(&payload, target, &compiler_version)?;
    check_outputs(temporary.path())?;
    unchanged(&workspace_before, &workspace_inputs(&workspace)?)?;
    recheck_source(&workspace, package, &source, &source_before)?;
    if let (Some(syntax_package), Some((identity, files))) = (syntax, &syntax_before) {
        recheck_source(&workspace, syntax_package, identity, files)?;
    }
    if compiler_hash != digest(&read_file(&compiler, MAX_BYTES)?)
        || object_hash != digest(&read_file(&object, MAX_BYTES)?)
    {
        return Err("compiler layout compiler or output changed".into());
    }
    // The parser owns only DWARF facts; the driver owns their build correspondence.
    let layouts = parse(&payload, request.qualified_types)?;
    unchanged(&externs_before, &extern_inputs(&prepared.externs)?)?;
    unchanged(&workspace_before, &workspace_inputs(&workspace)?)?;
    Ok(static_report(
        request,
        &json!({
            "workspace":workspace, "workspace_inputs":workspace_before,
            "package_id":package["id"], "source":source,
            "metadata_sha256":digest(&metadata_output.stdout),
            "syntax_dependency":syntax_before.as_ref().map(|(identity, _)| identity),
            "compiler":compiler, "compiler_verbose":compiler_version, "compiler_sha256":compiler_hash,
            "repository_rust_pin":compiler_pin,
            "target":target, "profile":profile, "features":request.features,
            "argv":argv, "cargo_receipt":prepared.receipt, "preparation_argv":prepared.argv,
            "cargo_stdout_sha256":digest(&prepared.output.stdout),
            "cargo_stderr_sha256":digest(&prepared.output.stderr), "object_sha256":object_hash,
            "externs":prepared.externs, "extern_inputs":externs_before,
            "direct_stdout_sha256":digest(&direct_output.stdout),
            "direct_stderr_sha256":digest(&direct_output.stderr),
            "payload_sha256":digest(&payload), "payload_bytes":payload.len(),
            "payload_kind":if link.is_null() {"native_relocatable_object"} else {"linked_core_wasm"},
            "abi_and_producers":abi, "link":link,
            "driver_source_sha256":digest(include_bytes!("compiler_layout.rs")),
            "native_dwarf_source_sha256":digest(include_bytes!("native_dwarf.rs")),
            "application_artifact_correspondence":"not_established"
        }),
        &layouts,
    ))
}

fn request_at<'a>(requested: &Request<'a>, workspace: &'a Path) -> Request<'a> {
    Request {
        workspace,
        package: requested.package,
        target: requested.target,
        features: requested.features,
        qualified_types: requested.qualified_types,
    }
}

fn build_target<'a>(requested: &'a str, compiler: &'a str) -> Result<(&'a str, &'a str), String> {
    let mut hosts = compiler
        .lines()
        .filter_map(|line| line.strip_prefix("host: "));
    let host = hosts
        .next()
        .filter(|value| !value.is_empty())
        .ok_or("compiler layout compiler host missing")?;
    if hosts.next().is_some() {
        return Err("ambiguous compiler layout compiler host".into());
    }
    let target = if requested == "native" {
        host
    } else {
        requested
    };
    if target != host
        && !matches!(
            target,
            "wasm32-unknown-unknown" | "wasm32-wasip1" | "wasm32-wasip2"
        )
    {
        return Err("unsupported compiler layout target".into());
    }
    Ok((target, host))
}

fn metadata_for(
    request: &Request<'_>,
    compiler: &Path,
    target: &str,
) -> Result<(Value, super::Output), String> {
    let mut command = super::cargo(request.workspace);
    sanitize(&mut command, compiler, &request.workspace.join("target"));
    command.args([
        "metadata",
        "--locked",
        "--offline",
        "--format-version",
        "1",
        "--filter-platform",
        target,
        "--features",
        "auctioneer-bidder/schema-artifact-staging",
    ]);
    let output = super::successful(&mut command, Duration::from_secs(60))?;
    let metadata: Value =
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    if metadata["workspace_root"].as_str().map(Path::new) != Some(request.workspace) {
        return Err("compiler layout workspace mismatch".into());
    }
    Ok((metadata, output))
}

fn verify_library_source(package: &Value, library: &Value) -> Result<(), String> {
    let root = Path::new(
        package["manifest_path"]
            .as_str()
            .ok_or("compiler layout manifest missing")?,
    )
    .parent()
    .ok_or("compiler layout source root missing")?;
    let path = Path::new(
        library["src_path"]
            .as_str()
            .ok_or("compiler layout library path missing")?,
    );
    if !path.starts_with(root) {
        return Err("compiler layout library escapes verified source".into());
    }
    Ok(())
}

fn recheck_source(
    workspace: &Path,
    package: &Value,
    identity: &Value,
    files: &BTreeMap<PathBuf, String>,
) -> Result<(), String> {
    let (after, after_files) = source_inputs(workspace, package)?;
    unchanged(files, &after_files)?;
    if *identity != after {
        return Err("compiler layout source identity changed".into());
    }
    Ok(())
}

struct ExpectedProduct<'a> {
    package_id: &'a str,
    target: &'a Value,
    features: &'a [&'a str],
    profile: &'a str,
    debuginfo: u64,
    require_fresh: bool,
    deps_dir: Option<&'a Path>,
}

fn products(messages: &[u8], expected: &ExpectedProduct<'_>) -> Result<Value, String> {
    if messages.len() > MAX_MESSAGES {
        return Err("compiler layout messages exceed byte cap".into());
    }
    let text = from_utf8(messages).map_err(|error| error.to_string())?;
    let mut product = None;
    let mut finished = false;
    for line in text.lines() {
        let row: Value = serde_json::from_str(line).map_err(|error| error.to_string())?;
        match row["reason"].as_str() {
            Some("compiler-artifact")
                if row["package_id"] == expected.package_id
                    && expected
                        .deps_dir
                        .is_none_or(|directory| artifact_in(&row, directory)) =>
            {
                if product.is_some() {
                    return Err("ambiguous compiler layout receipt".into());
                }
                if row["target"] != *expected.target {
                    return Err("compiler layout target mismatch".into());
                }
                let actual = row["features"]
                    .as_array()
                    .and_then(|values| values.iter().map(Value::as_str).collect::<Option<Vec<_>>>())
                    .ok_or_else(|| feature_mismatch(&row, expected))?;
                if actual.iter().copied().collect::<BTreeSet<_>>().len() != actual.len()
                    || actual.iter().copied().collect::<BTreeSet<_>>()
                        != expected.features.iter().copied().collect()
                    || expected
                        .features
                        .iter()
                        .copied()
                        .collect::<BTreeSet<_>>()
                        .len()
                        != expected.features.len()
                {
                    return Err(feature_mismatch(&row, expected));
                }
                if !matches!(expected.profile, "release" | "wasm-release")
                    || row["profile"]["opt_level"] != "3"
                    || row["profile"]["test"] != false
                    || row["profile"]["debug_assertions"] != false
                    || row["profile"]["overflow_checks"] != false
                    || row["profile"]["debuginfo"] != expected.debuginfo
                {
                    return Err("compiler layout profile mismatch".into());
                }
                if row["fresh"].as_bool().is_none()
                    || (expected.require_fresh && row["fresh"] != false)
                {
                    return Err("compiler layout was not freshly compiled".into());
                }
                product = Some(row);
            }
            Some("build-finished") => {
                if finished || row["success"] != true {
                    return Err("compiler layout build did not complete".into());
                }
                finished = true;
            }
            _ => {}
        }
    }
    if !finished {
        return Err("compiler layout build did not complete".into());
    }
    product.ok_or_else(|| "compiler layout receipt missing".into())
}

fn feature_mismatch(row: &Value, expected: &ExpectedProduct<'_>) -> String {
    let mut actual: Vec<_> = row["features"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    actual.sort_unstable();
    let mut oracle = expected.features.to_vec();
    oracle.sort_unstable();
    let mut error = format!(
        "compiler layout feature mismatch: {}",
        json!({
            "package_id":expected.package_id, "actual_features":actual, "expected_features":oracle
        })
    );
    if error.len() > 4096 {
        let end = error.floor_char_boundary(4096_usize.saturating_sub(" [truncated]".len()));
        error.truncate(end);
        error.push_str(" [truncated]");
    }
    error
}

fn prepare_command(
    request: &Request<'_>,
    compiler: &Path,
    target: &str,
) -> Result<Command, String> {
    validate_request(request)?;
    let mut command = super::cargo(request.workspace);
    sanitize(&mut command, compiler, &request.workspace.join("target"));
    command.args([
        "build",
        "--locked",
        "--offline",
        "-j",
        "2",
        "-p",
        "auctioneer-bidder",
        "--features",
        "schema-artifact-staging",
        "--lib",
        "--profile",
        profile(target),
        "--target",
        target,
        "--message-format=json-render-diagnostics",
    ]);
    Ok(command)
}

struct PreparedGraph {
    receipt: Value,
    output: super::Output,
    argv: Vec<String>,
    deps_dir: PathBuf,
    externs: BTreeMap<String, PathBuf>,
}

fn artifact_in(row: &Value, directory: &Path) -> bool {
    row["filenames"].as_array().is_some_and(|names| {
        names
            .iter()
            .filter_map(Value::as_str)
            .map(Path::new)
            .any(|path| {
                path.parent() == Some(directory)
                    && path
                        .extension()
                        .is_some_and(|extension| extension == "rlib")
            })
    })
}

fn prepare_graph(
    request: &Request<'_>,
    compiler: &Path,
    target: &str,
    metadata: &Value,
    package: &Value,
) -> Result<PreparedGraph, String> {
    let deps_dir = fs::canonicalize(request.workspace)
        .map_err(|error| error.to_string())?
        .join("target")
        .join(target)
        .join(profile(target))
        .join("deps");
    let mut command = prepare_command(request, compiler, target)?;
    let argv = command_args(&command)?;
    let output = super::successful(&mut command, TIMEOUT)?;
    let receipt = products(
        &output.stdout,
        &ExpectedProduct {
            package_id: package["id"]
                .as_str()
                .ok_or("compiler layout package ID missing")?,
            target: library_target(package)?,
            features: request.features,
            profile: profile(target),
            debuginfo: profile_debug(request.workspace, metadata, package, profile(target))?,
            require_fresh: false,
            deps_dir: Some(&deps_dir),
        },
    )?;
    let mut receipts = Vec::new();
    for line in from_utf8(&output.stdout)
        .map_err(|error| error.to_string())?
        .lines()
    {
        let row: Value = serde_json::from_str(line).map_err(|error| error.to_string())?;
        if row["reason"] == "compiler-artifact" && artifact_in(&row, &deps_dir) {
            receipts.push(row);
        }
    }
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .ok_or("compiler layout resolve nodes missing")?;
    let mut matches = nodes.iter().filter(|node| node["id"] == package["id"]);
    let node = matches
        .next()
        .ok_or("compiler layout resolve node missing")?;
    if matches.next().is_some() {
        return Err("ambiguous compiler layout resolve node".into());
    }
    let externs = prepared_externs(package, request.features, node, &receipts, &deps_dir)?;
    Ok(PreparedGraph {
        receipt,
        output,
        argv,
        deps_dir,
        externs,
    })
}

fn direct_command(
    request: &Request<'_>,
    compiler: &Path,
    target: &str,
    library: &Value,
    object: &Path,
    deps_dir: &Path,
    externs: &BTreeMap<String, PathBuf>,
) -> Result<Command, String> {
    validate_request(request)?;
    if !compiler.is_absolute()
        || !object.is_absolute()
        || !deps_dir.is_absolute()
        || object
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
    {
        return Err("compiler layout requires an absolute fresh object path".into());
    }
    let name = library["name"]
        .as_str()
        .ok_or("compiler layout crate name missing")?;
    let edition = library["edition"]
        .as_str()
        .ok_or("compiler layout edition missing")?;
    let source = Path::new(
        library["src_path"]
            .as_str()
            .ok_or("compiler layout source path missing")?,
    );
    if !source.is_absolute()
        || !matches!(edition, "2015" | "2018" | "2021" | "2024")
        || library["kind"] != json!(["lib"])
        || library["crate_types"] != json!(["lib"])
    {
        return Err("compiler layout library target refused".into());
    }
    let mut command = Command::new(compiler);
    command.current_dir(request.workspace);
    sanitize(&mut command, compiler, &request.workspace.join("target"));
    command
        .args([
            "--crate-name",
            name,
            "--edition",
            edition,
            "--crate-type",
            "rlib",
            "--target",
            target,
            "-Copt-level=3",
            "-Ccodegen-units=1",
            "-Cdebug-assertions=no",
            "-Coverflow-checks=no",
        ])
        .arg(source)
        .arg("-L")
        .arg(format!("dependency={}", deps_dir.display()));
    for feature in request.features.iter().copied().collect::<BTreeSet<_>>() {
        command.arg("--cfg").arg(format!("feature=\"{feature}\""));
    }
    for (alias, path) in externs {
        if path.parent() != Some(deps_dir)
            || path.extension().is_none_or(|extension| extension != "rlib")
        {
            return Err("compiler layout extern artifact escapes target deps".into());
        }
        command
            .arg("--extern")
            .arg(format!("{alias}={}", path.display()));
    }
    command
        .args(["-Cdebuginfo=2", "-Clinker-plugin-lto=no"])
        .arg(format!(
            "--emit=obj={}",
            object.to_str().ok_or("non UTF-8 compiler layout output")?
        ));
    Ok(command)
}

fn active_dependencies(package: &Value, features: &[&str]) -> Result<BTreeSet<String>, String> {
    let mut declared = BTreeMap::new();
    let mut active = BTreeSet::new();
    for dependency in package["dependencies"]
        .as_array()
        .ok_or("compiler layout dependencies missing")?
    {
        if !dependency["kind"].is_null() {
            continue;
        }
        let alias = dependency["rename"]
            .as_str()
            .or_else(|| dependency["name"].as_str())
            .ok_or("compiler layout dependency name missing")?;
        if declared.insert(alias, dependency).is_some() {
            return Err("ambiguous compiler layout dependency declaration".into());
        }
        if !dependency["optional"]
            .as_bool()
            .ok_or("compiler layout dependency optional state missing")?
        {
            active.insert(alias);
        }
    }
    let table = package["features"]
        .as_object()
        .ok_or("compiler layout feature declarations missing")?;
    let mut pending = features.to_vec();
    let mut visited = BTreeSet::new();
    while let Some(feature) = pending.pop() {
        if !visited.insert(feature) {
            continue;
        }
        let edges = table
            .get(feature)
            .and_then(Value::as_array)
            .ok_or("compiler layout unknown feature declaration")?;
        for edge_value in edges {
            let edge = edge_value
                .as_str()
                .ok_or("compiler layout malformed feature edge")?;
            if let Some(alias) = edge.strip_prefix("dep:") {
                if !declared.contains_key(alias) {
                    return Err("compiler layout unknown dependency feature edge".into());
                }
                active.insert(alias);
            } else if let Some((owner, _)) = edge.split_once('/') {
                let alias = owner.strip_suffix('?').unwrap_or(owner);
                if !declared.contains_key(alias) {
                    return Err("compiler layout unknown dependency feature edge".into());
                }
                if !owner.ends_with('?') {
                    active.insert(alias);
                }
            } else {
                pending.push(edge);
            }
        }
    }
    let mut names = BTreeSet::new();
    for alias in active {
        if !declared[alias]["target"].is_null() {
            return Err("unsupported compiler layout target-specific dependency".into());
        }
        if !names.insert(alias.replace('-', "_")) {
            return Err("ambiguous compiler layout extern name".into());
        }
    }
    Ok(names)
}

fn prepared_externs(
    package: &Value,
    features: &[&str],
    node: &Value,
    receipts: &[Value],
    deps_dir: &Path,
) -> Result<BTreeMap<String, PathBuf>, String> {
    if node["id"] != package["id"] {
        return Err("compiler layout resolve package mismatch".into());
    }
    let edges = node["deps"]
        .as_array()
        .ok_or("compiler layout resolved dependencies missing")?;
    let mut selected = BTreeMap::new();
    for alias in active_dependencies(package, features)? {
        let mut matches = edges.iter().filter(|edge| {
            edge["name"]
                .as_str()
                .is_some_and(|name| name.replace('-', "_") == alias)
                && edge["dep_kinds"].as_array().is_some_and(|kinds| {
                    kinds
                        .iter()
                        .any(|kind| kind["kind"].is_null() && kind["target"].is_null())
                })
        });
        let edge = matches
            .next()
            .ok_or("compiler layout active resolve dependency missing")?;
        if matches.next().is_some() {
            return Err("ambiguous compiler layout active resolve dependency".into());
        }
        let mut matching_receipts = receipts.iter().filter(|row| {
            row["reason"] == "compiler-artifact"
                && row["package_id"] == edge["pkg"]
                && row["target"]["kind"]
                    .as_array()
                    .is_some_and(|kinds| kinds.iter().any(|kind| kind == "lib" || kind == "rlib"))
        });
        let receipt = matching_receipts
            .next()
            .ok_or("compiler layout active extern receipt missing")?;
        if matching_receipts.next().is_some() {
            return Err("ambiguous compiler layout active extern receipt".into());
        }
        let mut artifacts = receipt["filenames"]
            .as_array()
            .ok_or("compiler layout extern filenames missing")?
            .iter()
            .filter_map(Value::as_str)
            .map(Path::new)
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "rlib")
            });
        let artifact = artifacts
            .next()
            .ok_or("compiler layout active extern rlib missing")?;
        if artifacts.next().is_some() {
            return Err("ambiguous compiler layout active extern rlib".into());
        }
        if !artifact.is_absolute()
            || artifact.parent() != Some(deps_dir)
            || artifact
                .components()
                .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
        {
            return Err("compiler layout extern artifact escapes target deps".into());
        }
        selected.insert(alias, artifact.to_owned());
    }
    Ok(selected)
}

fn extern_inputs(externs: &BTreeMap<String, PathBuf>) -> Result<BTreeMap<PathBuf, String>, String> {
    externs
        .values()
        .map(|path| {
            if fs::canonicalize(path).map_err(|error| error.to_string())? != *path {
                return Err("compiler layout extern symlink refused".into());
            }
            Ok((path.clone(), digest(&read_file(path, MAX_BYTES)?)))
        })
        .collect()
}

fn verify_command(
    command: &Command,
    compiler: &Path,
    target: &str,
    object: &Path,
) -> Result<(), String> {
    let args = command_args(command)?;
    let suffix = [
        "-Cdebuginfo=2".into(),
        "-Clinker-plugin-lto=no".into(),
        format!(
            "--emit=obj={}",
            object.to_str().ok_or("non UTF-8 compiler layout output")?
        ),
    ];
    let target_flags: Vec<_> = args
        .windows(2)
        .filter(|pair| pair[0] == "--target")
        .collect();
    let environment: BTreeMap<_, _> = command.get_envs().collect();
    if !compiler.is_absolute()
        || command.get_program() != compiler.as_os_str()
        || !args.ends_with(&suffix)
        || target_flags.len() != 1
        || target_flags[0][1] != target
        || environment.get(OsStr::new("RUSTC")) != Some(&Some(compiler.as_os_str()))
        || environment.get(OsStr::new("CARGO_ENCODED_RUSTFLAGS")) != Some(&Some(OsStr::new("")))
        || [
            "RUSTFLAGS",
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
            "RUSTC_BOOTSTRAP",
        ]
        .iter()
        .any(|key| environment.get(OsStr::new(key)) != Some(&None))
    {
        return Err("compiler layout command identity mismatch".into());
    }
    Ok(())
}

fn unchanged(
    before: &BTreeMap<PathBuf, String>,
    after: &BTreeMap<PathBuf, String>,
) -> Result<(), String> {
    if before != after {
        return Err("compiler layout inputs changed during build".into());
    }
    Ok(())
}

fn static_report(request: &Request<'_>, binding: &Value, layouts: &Value) -> Value {
    json!({"schema_version":1_u8, "kind":"compiler_static_layout_observation",
        "qualification":"Unqualified", "execution":"not_executed", "static_layout_only":true,
        "resource_qualification":false, "serving_qualification":false,
        "package":request.package, "requested_target":request.target,
        "qualified_types":request.qualified_types, "binding":binding, "layouts":layouts,
        "scope":"compiler type layout only; not heap, allocator, work, scratch, residency or application qualification"})
}

fn profile(target: &str) -> &'static str {
    if target.starts_with("wasm32-") {
        "wasm-release"
    } else {
        "release"
    }
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn verify_compiler_pin(tool_versions: &str, compiler: &str) -> Result<String, String> {
    let mut pin = None;
    for line in tool_versions.lines() {
        let mut words = line.split_whitespace();
        if words.next() != Some("rust") {
            continue;
        }
        let version = words
            .next()
            .ok_or("compiler layout repository Rust pin missing")?;
        if words.next().is_some() || pin.replace(version).is_some() {
            return Err("compiler layout ambiguous repository Rust pin".into());
        }
    }
    let expected = pin.ok_or("compiler layout repository Rust pin missing")?;
    let mut releases = compiler
        .lines()
        .filter_map(|line| line.strip_prefix("release: "));
    if compiler
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("rustc "))
        .and_then(|line| line.split_whitespace().next())
        != Some(expected)
        || releases.next() != Some(expected)
        || releases.next().is_some()
    {
        return Err("compiler layout compiler differs from repository Rust pin".into());
    }
    Ok(expected.to_owned())
}

fn validate_request(request: &Request<'_>) -> Result<(), String> {
    let token = |value: &str| {
        !value.is_empty()
            && !value.starts_with('-')
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    };
    if !request.workspace.is_absolute()
        || !token(request.package)
        || !token(request.target)
        || request.features.len() > 128
        || request.features.iter().any(|feature| !token(feature))
        || request.features.iter().collect::<BTreeSet<_>>().len() != request.features.len()
        || request.qualified_types.is_empty()
        || request.qualified_types.len() > 128
        || request
            .qualified_types
            .iter()
            .any(|name| name.is_empty() || name.len() > 4096 || !name.contains("::"))
        || request
            .qualified_types
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != request.qualified_types.len()
    {
        return Err("invalid compiler layout request".into());
    }
    Ok(())
}

fn sanitize(command: &mut Command, compiler: &Path, target_dir: &Path) {
    command
        .env("RUSTC", compiler)
        .env("CARGO_TARGET_DIR", target_dir)
        .env("CARGO_ENCODED_RUSTFLAGS", "")
        .env("CARGO_TERM_COLOR", "never")
        .env_remove("RUSTFLAGS")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("RUSTC_BOOTSTRAP");
}

fn command_args(command: &Command) -> Result<Vec<String>, String> {
    command
        .get_args()
        .map(|arg| {
            arg.to_str()
                .map(str::to_owned)
                .ok_or_else(|| "non UTF-8 compiler layout argument".into())
        })
        .collect()
}

fn read_file(path: &Path, cap: u64) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > cap {
        return Err("compiler layout file or byte cap refused".into());
    }
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(cap.saturating_add(1)).read_to_end(&mut bytes))
        .map_err(|error| error.to_string())?;
    if u64::try_from(bytes.len()).map_err(|error| error.to_string())? != metadata.len() {
        return Err("compiler layout file changed while reading".into());
    }
    Ok(bytes)
}

fn workspace_inputs(workspace: &Path) -> Result<BTreeMap<PathBuf, String>, String> {
    ["Cargo.toml", "Cargo.lock", ".tool-versions"]
        .into_iter()
        .map(|name| {
            let path = workspace.join(name);
            Ok((path.clone(), digest(&read_file(&path, 4 << 20)?)))
        })
        .collect()
}

fn selected_package<'a>(metadata: &'a Value, name: &str) -> Result<&'a Value, String> {
    let mut candidates = metadata["packages"]
        .as_array()
        .ok_or("compiler layout metadata missing packages")?
        .iter()
        .filter(|row| row["name"] == name);
    let selected = candidates.next().ok_or("compiler layout package missing")?;
    if candidates.next().is_some() || selected["id"].as_str().is_none_or(str::is_empty) {
        return Err("ambiguous compiler layout package".into());
    }
    Ok(selected)
}

fn library_target(package: &Value) -> Result<&Value, String> {
    let mut targets = package["targets"]
        .as_array()
        .ok_or("compiler layout targets missing")?
        .iter()
        .filter(|row| {
            row["kind"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|kind| kind == "lib"))
        });
    let target = targets.next().ok_or("compiler layout library missing")?;
    if targets.next().is_some() {
        return Err("ambiguous compiler layout library".into());
    }
    Ok(target)
}

fn profile_debug(
    workspace: &Path,
    metadata: &Value,
    package: &Value,
    profile: &str,
) -> Result<u64, String> {
    let manifest: toml::Value = toml::from_str(
        from_utf8(&read_file(&workspace.join("Cargo.toml"), 4 << 20)?)
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let member = metadata["workspace_members"]
        .as_array()
        .ok_or("compiler layout workspace members missing")?
        .contains(&package["id"]);
    let name = package["name"]
        .as_str()
        .ok_or("compiler layout package name missing")?;
    let mut debug = 0;
    for level in ["release", profile] {
        let Some(table) = manifest.get("profile").and_then(|rows| rows.get(level)) else {
            continue;
        };
        let configured = table
            .get("package")
            .and_then(|rows| {
                rows.get(name)
                    .or_else(|| (!member).then(|| rows.get("*")).flatten())
            })
            .and_then(|row| row.get("debug"))
            .or_else(|| table.get("debug"));
        if let Some(setting) = configured {
            debug = match setting {
                toml::Value::Boolean(enabled) => {
                    if *enabled {
                        2
                    } else {
                        0
                    }
                }
                toml::Value::Integer(number @ 0..=2) => {
                    u64::try_from(*number).map_err(|error| error.to_string())?
                }
                toml::Value::String(_)
                | toml::Value::Integer(_)
                | toml::Value::Float(_)
                | toml::Value::Datetime(_)
                | toml::Value::Array(_)
                | toml::Value::Table(_) => {
                    return Err("unsupported compiler layout profile debug setting".into());
                }
            };
        }
    }
    Ok(debug)
}

fn source_tree(root: &Path) -> Result<BTreeMap<PathBuf, String>, String> {
    if !fs::symlink_metadata(root)
        .map_err(|error| error.to_string())?
        .is_dir()
    {
        return Err("compiler layout source directory refused".into());
    }
    let mut files = BTreeMap::new();
    let mut pending = vec![PathBuf::new()];
    let mut directories = 0_usize;
    let mut total = 0_u64;
    while let Some(parent) = pending.pop() {
        directories = directories
            .checked_add(1)
            .ok_or("compiler layout directory count overflow")?;
        if directories > 128 {
            return Err("compiler layout source directory cap".into());
        }
        for entry_result in fs::read_dir(root.join(&parent)).map_err(|error| error.to_string())? {
            let entry = entry_result.map_err(|error| error.to_string())?;
            let relative = parent.join(entry.file_name());
            let metadata = fs::symlink_metadata(entry.path()).map_err(|error| error.to_string())?;
            if metadata.is_dir() {
                pending.push(relative);
                continue;
            }
            if relative == Path::new(".cargo-ok") || relative == Path::new(".cargo-checksum.json") {
                continue;
            }
            let bytes = read_file(&entry.path(), MAX_BYTES)?;
            total = total
                .checked_add(u64::try_from(bytes.len()).map_err(|error| error.to_string())?)
                .ok_or("compiler layout source byte overflow")?;
            if total > MAX_BYTES || files.len() >= MAX_FILES {
                return Err("compiler layout source cap".into());
            }
            files.insert(relative, digest(&bytes));
        }
    }
    if files.is_empty() {
        return Err("compiler layout empty source tree".into());
    }
    Ok(files)
}

fn source_inputs(
    workspace: &Path,
    package: &Value,
) -> Result<(Value, BTreeMap<PathBuf, String>), String> {
    let manifest = Path::new(
        package["manifest_path"]
            .as_str()
            .ok_or("compiler layout manifest missing")?,
    );
    let root = manifest
        .parent()
        .ok_or("compiler layout source root missing")?;
    let files = source_tree(root)?;
    let identity = if package["name"] == "regex-automata" && package["source"].is_null() {
        let verified =
            super::schema_regex_vendor::verify(workspace).map_err(|error| error.to_string())?;
        if root != verified.canonical_root || package["version"] != "0.4.18" {
            return Err("compiler layout reviewed engine mismatch".into());
        }
        serde_json::to_value(verified).map_err(|error| error.to_string())?
    } else {
        verify_registry(workspace, package, root, &files)?
    };
    Ok((
        json!({"verified":identity, "source_root":root, "files":files}),
        files,
    ))
}

fn verify_registry(
    workspace: &Path,
    package: &Value,
    root: &Path,
    files: &BTreeMap<PathBuf, String>,
) -> Result<Value, String> {
    const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";
    let name = package["name"]
        .as_str()
        .ok_or("compiler layout package name missing")?;
    let version = package["version"]
        .as_str()
        .ok_or("compiler layout package version missing")?;
    let lock: toml::Value = toml::from_str(
        from_utf8(&read_file(&workspace.join("Cargo.lock"), 4 << 20)?)
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let mut rows = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or("compiler layout lock packages missing")?
        .iter()
        .filter(|row| {
            row.get("name").and_then(toml::Value::as_str) == Some(name)
                && row.get("version").and_then(toml::Value::as_str) == Some(version)
        });
    let row = rows
        .next()
        .ok_or("compiler layout locked package missing")?;
    let checksum = row
        .get("checksum")
        .and_then(toml::Value::as_str)
        .ok_or("compiler layout archive checksum missing")?;
    if rows.next().is_some()
        || package["source"] != REGISTRY
        || row.get("source").and_then(toml::Value::as_str) != Some(REGISTRY)
        || checksum.len() != 64
        || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("compiler layout unverified source identity".into());
    }
    let index = root
        .parent()
        .ok_or("compiler layout registry source path missing")?;
    let registry = index
        .parent()
        .and_then(Path::parent)
        .ok_or("compiler layout registry root missing")?;
    if index.parent().and_then(Path::file_name) != Some(OsStr::new("src")) {
        return Err("compiler layout registry source path refused".into());
    }
    let archive_path = registry
        .join("cache")
        .join(
            index
                .file_name()
                .ok_or("compiler layout registry index missing")?,
        )
        .join(format!("{name}-{version}.crate"));
    let archive_bytes = read_file(&archive_path, MAX_BYTES)?;
    if digest(&archive_bytes) != checksum {
        return Err("compiler layout archive checksum mismatch".into());
    }
    let prefix = format!("{name}-{version}");
    let expected = archive_tree(&archive_bytes, &prefix)?;
    if *files != expected {
        return Err("compiler layout source differs from locked crate archive".into());
    }
    Ok(
        json!({"kind":"locked_crate_archive_exact_tree", "archive":archive_path, "archive_sha256":checksum}),
    )
}

fn archive_tree(archive_bytes: &[u8], prefix: &str) -> Result<BTreeMap<PathBuf, String>, String> {
    let mut archive =
        tar::Archive::new(GzDecoder::new(archive_bytes).take(MAX_BYTES.saturating_add(1)));
    let mut expected = BTreeMap::new();
    let mut total = 0_u64;
    for entry_result in archive.entries().map_err(|error| error.to_string())? {
        let mut entry = entry_result.map_err(|error| error.to_string())?;
        let full = entry
            .path()
            .map_err(|error| error.to_string())?
            .into_owned();
        let path = full
            .strip_prefix(prefix)
            .map_err(|_error| "compiler layout archive prefix mismatch")?
            .to_owned();
        if path.as_os_str().is_empty()
            || path
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
            || !entry.header().entry_type().is_file()
            || entry.size() > MAX_BYTES
            || expected.len() >= MAX_FILES
        {
            return Err("compiler layout archive member refused".into());
        }
        total = total
            .checked_add(entry.size())
            .ok_or("compiler layout archive byte overflow")?;
        if total > MAX_BYTES {
            return Err("compiler layout archive byte cap".into());
        }
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if expected.insert(path, digest(&bytes)).is_some() {
            return Err("compiler layout duplicate archive member".into());
        }
    }
    Ok(expected)
}

fn check_outputs(directory: &Path) -> Result<(), String> {
    let mut total = 0_u64;
    for entry_result in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let entry = entry_result.map_err(|error| error.to_string())?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|error| error.to_string())?;
        if !metadata.is_file()
            || !matches!(
                entry.file_name().to_str(),
                Some("layout.fresh.o" | "layout.wasm")
            )
        {
            return Err("compiler layout unexpected output".into());
        }
        total = total
            .checked_add(metadata.len())
            .ok_or("compiler layout output size overflow")?;
        if metadata.len() == 0 || total > MAX_BYTES {
            return Err("compiler layout output byte cap".into());
        }
    }
    Ok(())
}

fn link_wasm(
    directory: &Path,
    object: &Path,
    compiler: &Path,
    host: &str,
) -> Result<(Vec<u8>, Value), String> {
    let sysroot = compiler
        .parent()
        .and_then(Path::parent)
        .ok_or("compiler layout sysroot missing")?;
    let lld_path = sysroot.join("lib/rustlib").join(host).join("bin/rust-lld");
    let lld = fs::canonicalize(lld_path)
        .map_err(|error| format!("compiler layout pinned rust-lld missing: {error}"))?;
    if !lld.starts_with(sysroot) {
        return Err("compiler layout rust-lld escapes verified sysroot".into());
    }
    let lld_hash = digest(&read_file(&lld, MAX_BYTES)?);
    let mut version_command = Command::new(&lld);
    version_command
        .args(["-flavor", "wasm", "--version"])
        .env("DYLD_LIBRARY_PATH", sysroot.join("lib"))
        .env_remove("RUSTC_BOOTSTRAP");
    let version_output = super::successful(&mut version_command, Duration::from_secs(10))?;
    let version = String::from_utf8(version_output.stdout).map_err(|error| error.to_string())?;
    if version.trim().is_empty() {
        return Err("compiler layout rust-lld version missing".into());
    }
    let wasm = directory.join("layout.wasm");
    let mut command = Command::new(&lld);
    command
        .args([
            "-flavor",
            "wasm",
            "--no-entry",
            "--no-gc-sections",
            "--allow-undefined",
        ])
        .arg(object)
        .arg("-o")
        .arg(&wasm)
        .env("DYLD_LIBRARY_PATH", sysroot.join("lib"))
        .env_remove("RUSTC_BOOTSTRAP");
    let argv = command_args(&command)?;
    super::successful(&mut command, TIMEOUT)?;
    check_outputs(directory)?;
    let bytes = read_file(&wasm, MAX_BYTES)?;
    if lld_hash != digest(&read_file(&lld, MAX_BYTES)?) {
        return Err("compiler layout rust-lld changed".into());
    }
    Ok((
        bytes,
        json!({"path":lld, "sha256":lld_hash, "version":version,
        "argv":argv, "sysroot":sysroot, "DYLD_LIBRARY_PATH":sysroot.join("lib")}),
    ))
}

type DebugSections<'a> = BTreeMap<String, (&'a [u8], Rc<Relocations>)>;

fn payload_identity(bytes: &[u8], target: &str, compiler: &str) -> Result<Value, String> {
    let (sections, abi) = if target.starts_with("wasm32-") {
        (
            wasm_debug_sections(bytes)?,
            json!({"architecture":"Wasm32", "pointer_bits":32_u8, "endian":"little"}),
        )
    } else {
        native_debug_sections(bytes, target)?
    };
    let producers = debug_producers(&sections, compiler)?;
    Ok(json!({"abi":abi, "dwarf_producers":producers}))
}

fn wasm_debug_sections(bytes: &[u8]) -> Result<DebugSections<'_>, String> {
    use wasmparser::Payload;
    let mut sections = BTreeMap::new();
    let mut module = false;
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        match payload.map_err(|error| error.to_string())? {
            Payload::Version {
                encoding: wasmparser::Encoding::Module,
                ..
            } if !module => module = true,
            Payload::Version { .. } => {
                return Err("compiler layout requires linked core Wasm".into());
            }
            Payload::CustomSection(section) => {
                if section.name().starts_with("reloc.") {
                    return Err("compiler layout unresolved Wasm relocation".into());
                }
                if section.name().starts_with(".debug_")
                    && sections
                        .insert(section.name().into(), (section.data(), Rc::default()))
                        .is_some()
                {
                    return Err("compiler layout duplicate debug section".into());
                }
            }
            Payload::MemorySection(memories) => {
                for memory in memories {
                    if memory.map_err(|error| error.to_string())?.memory64 {
                        return Err("compiler layout target ABI mismatch".into());
                    }
                }
            }
            Payload::TypeSection(_)
            | Payload::ImportSection(_)
            | Payload::FunctionSection(_)
            | Payload::TableSection(_)
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
            | Payload::ComponentImportSection(_)
            | Payload::ComponentExportSection(_)
            | Payload::UnknownSection { .. }
            | Payload::End(_) => {}
            _ => return Err("unsupported compiler layout WASM payload".into()),
        }
    }
    if !module {
        return Err("compiler layout core Wasm header missing".into());
    }
    Ok(sections)
}

fn native_debug_sections<'a>(
    bytes: &'a [u8],
    target: &str,
) -> Result<(DebugSections<'a>, Value), String> {
    use object::{Object as _, ObjectSection as _};
    let mut sections = BTreeMap::new();
    let file = object::File::parse(bytes).map_err(|error| error.to_string())?;
    let expected = match target.split('-').next() {
        Some("aarch64") => object::Architecture::Aarch64,
        Some("x86_64") => object::Architecture::X86_64,
        _ => return Err("unsupported native compiler layout ABI".into()),
    };
    if file.kind() != object::ObjectKind::Relocatable
        || file.architecture() != expected
        || !file.is_64()
        || !file.is_little_endian()
    {
        return Err("compiler layout target ABI mismatch".into());
    }
    for id in [
        gimli::SectionId::DebugAbbrev,
        gimli::SectionId::DebugInfo,
        gimli::SectionId::DebugLine,
        gimli::SectionId::DebugStr,
        gimli::SectionId::DebugLineStr,
        gimli::SectionId::DebugStrOffsets,
    ] {
        if let Some(section) = file.section_by_name(id.name()) {
            sections.insert(
                id.name().into(),
                (
                    section.data().map_err(|error| error.to_string())?,
                    Rc::new(Relocations::load(&file, &section)?),
                ),
            );
        }
    }
    let abi = json!({"architecture":format!("{:?}", file.architecture()), "pointer_bits":64_u8, "endian":"little"});
    Ok((sections, abi))
}

fn debug_producers(
    sections: &DebugSections<'_>,
    compiler: &str,
) -> Result<BTreeSet<String>, String> {
    use gimli::Reader as _;
    let dwarf = gimli::Dwarf::load(|id| -> Result<_, String> {
        let (data, relocations) = sections.get(id.name()).cloned().unwrap_or_default();
        Ok(NativeReader::new(
            data,
            gimli::RunTimeEndian::Little,
            relocations,
        ))
    })?;
    let expected = compiler
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("rustc "))
        .ok_or("compiler layout compiler version missing")?;
    let mut units = dwarf.units();
    let mut producers = BTreeSet::new();
    let mut count = 0_usize;
    while let Some(header) = units.next().map_err(|error| error.to_string())? {
        count = count
            .checked_add(1)
            .ok_or("compiler layout producer unit count overflow")?;
        if count > 4096 {
            return Err("compiler layout producer unit cap".into());
        }
        let unit = dwarf.unit(header).map_err(|error| error.to_string())?;
        let mut entries = unit.entries();
        let root = entries
            .next_dfs()
            .map_err(|error| error.to_string())?
            .ok_or("compiler layout compile unit missing")?;
        let producer_attribute = root
            .attr_value(gimli::DW_AT_producer)
            .ok_or("compiler layout DWARF producer missing")?;
        let producer_reader = dwarf
            .attr_string(&unit, producer_attribute)
            .map_err(|error| error.to_string())?;
        let producer = producer_reader
            .to_string_lossy()
            .map_err(|error| error.to_string())?
            .into_owned();
        if producer.len() > 4096 || !producer.contains(&format!("rustc version {expected}")) {
            return Err("compiler layout DWARF producer mismatch".into());
        }
        producers.insert(producer);
    }
    if producers.is_empty() {
        return Err("compiler layout DWARF producer missing".into());
    }
    Ok(producers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gimli::LittleEndian;
    use gimli::write::{EndianVec, RelocateWriter, Relocation as DebugRelocation};
    use object::write::{Object as WriteObject, Relocation as ObjectRelocation};
    use std::os::unix::fs::symlink;

    const FEATURES: &[&str] = &["std", "unicode"];

    fn request() -> Request<'static> {
        Request {
            workspace: Path::new("/fake_workspace"),
            package: "regex-syntax",
            target: "native",
            features: FEATURES,
            qualified_types: &["regex_syntax::hir::Hir"],
        }
    }

    #[test]
    fn compiler_layout_inspect_refuses_invalid_request_before_build_or_parser() {
        let mut invalid = request();
        invalid.workspace = Path::new("relative_workspace");
        assert_eq!(
            inspect(&invalid, |_, _| panic!("invalid request reached parser")),
            Err("invalid compiler layout request".into())
        );
        let mut missing_types = request();
        missing_types.qualified_types = &[];
        assert_eq!(
            inspect(&missing_types, |_, _| panic!(
                "invalid request reached parser"
            )),
            Err("invalid compiler layout request".into())
        );
    }

    #[test]
    fn compiler_layout_canonical_request_keeps_parent_traversal_out_of_every_command() {
        let temporary = tempfile::tempdir().unwrap();
        fs::create_dir_all(temporary.path().join("crates/auctioneer-bidder")).unwrap();
        let caller_root = temporary.path().join("crates/auctioneer-bidder/../..");
        let mut requested = request();
        requested.workspace = &caller_root;
        let workspace = fs::canonicalize(&caller_root).unwrap();
        let normalized = request_at(&requested, &workspace);
        let compiler = Path::new("/fake_sysroot/bin/rustc");
        let deps = workspace.join("target/wasm32-wasip1/wasm-release/deps");
        let object = workspace.join("layout.fresh.o");
        let commands = [
            prepare_command(&normalized, compiler, "wasm32-wasip1").unwrap(),
            direct_command(
                &normalized,
                compiler,
                "wasm32-wasip1",
                &target(),
                &object,
                &deps,
                &BTreeMap::new(),
            )
            .unwrap(),
        ];
        for command in commands {
            assert_eq!(command.get_current_dir(), Some(workspace.as_path()));
            let environment: BTreeMap<_, _> = command.get_envs().collect();
            assert_eq!(
                environment.get(OsStr::new("CARGO_TARGET_DIR")),
                Some(&Some(workspace.join("target").as_os_str()))
            );
        }
    }

    fn target() -> Value {
        json!({"name":"regex_syntax", "kind":["lib"], "crate_types":["lib"],
            "src_path":"/fake_registry/regex-syntax/src/lib.rs", "edition":"2021"})
    }

    fn messages(guest: bool) -> Vec<Value> {
        vec![
            json!({"reason":"compiler-artifact", "package_id":"fake_exact_package",
                "target":target(), "features":FEATURES, "fresh":false,
                "profile":{"opt_level":"3", "test":false, "debug_assertions":false,
                    "overflow_checks":false, "debuginfo":u64::from(!guest)},
                "filenames":["/fake_target/libregex_syntax.rlib"], "executable":null}),
            json!({"reason":"build-finished", "success":true}),
        ]
    }

    fn receipt(rows: &[Value], guest: bool) -> Result<Value, String> {
        let mut bytes = Vec::new();
        for row in rows {
            serde_json::to_writer(&mut bytes, row).map_err(|error| error.to_string())?;
            bytes.push(b'\n');
        }
        products(
            &bytes,
            &ExpectedProduct {
                package_id: "fake_exact_package",
                target: &target(),
                features: FEATURES,
                profile: if guest { "wasm-release" } else { "release" },
                debuginfo: u64::from(!guest),
                require_fresh: true,
                deps_dir: None,
            },
        )
    }

    #[test]
    fn compiler_layout_products_require_one_exact_compiled_receipt() {
        for guest in [false, true] {
            let rows = messages(guest);
            assert_eq!(receipt(&rows, guest), Ok(rows[0].clone()));
            let mut duplicate = rows.clone();
            duplicate.insert(1, duplicate[0].clone());
            assert_eq!(
                receipt(&duplicate, guest).unwrap_err(),
                "ambiguous compiler layout receipt"
            );
            assert_eq!(
                receipt(&rows[1..], guest).unwrap_err(),
                "compiler layout receipt missing"
            );
        }
    }

    #[test]
    fn compiler_layout_products_refuse_wrong_features_target_profile_and_freshness() {
        for (pointer, value, error) in [
            (
                "/features",
                json!(["std"]),
                "compiler layout feature mismatch",
            ),
            (
                "/features",
                json!(["std", "unicode", "unicode"]),
                "compiler layout feature mismatch",
            ),
            (
                "/target/src_path",
                json!("/fake_other/lib.rs"),
                "compiler layout target mismatch",
            ),
            (
                "/target/kind",
                json!(["bin"]),
                "compiler layout target mismatch",
            ),
            (
                "/profile/opt_level",
                json!("0"),
                "compiler layout profile mismatch",
            ),
            (
                "/profile/test",
                json!(true),
                "compiler layout profile mismatch",
            ),
            (
                "/profile/debuginfo",
                json!(0_u8),
                "compiler layout profile mismatch",
            ),
            (
                "/fresh",
                json!(true),
                "compiler layout was not freshly compiled",
            ),
        ] {
            let mut rows = messages(false);
            *rows[0].pointer_mut(pointer).unwrap() = value;
            let observed = receipt(&rows, false).unwrap_err();
            if pointer == "/features" {
                let (prefix, detail_json) = observed.split_once(": ").unwrap();
                assert_eq!(prefix, error);
                let detail: Value = serde_json::from_str(detail_json).unwrap();
                assert_eq!(detail["package_id"], "fake_exact_package");
                assert_eq!(detail["expected_features"], json!(FEATURES));
                let mut actual = rows[0]["features"].as_array().unwrap().clone();
                actual.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
                assert_eq!(detail["actual_features"], json!(actual));
            } else {
                assert_eq!(observed, error, "{pointer}");
            }
        }
        let mut rows = messages(false);
        rows[0]["package_id"] = json!("fake_foreign_package");
        assert_eq!(
            receipt(&rows, false).unwrap_err(),
            "compiler layout receipt missing"
        );
        let mut failed_rows = messages(false);
        failed_rows[1]["success"] = json!(false);
        assert_eq!(
            receipt(&failed_rows, false).unwrap_err(),
            "compiler layout build did not complete"
        );
        assert_eq!(
            receipt(&messages(false)[..1], false).unwrap_err(),
            "compiler layout build did not complete"
        );
    }

    #[test]
    fn compiler_layout_commands_request_one_fresh_object_with_explicit_profile_and_flags() {
        for (triple, expected_profile) in [
            ("aarch64-apple-darwin", "release"),
            ("wasm32-unknown-unknown", "wasm-release"),
            ("wasm32-wasip1", "wasm-release"),
            ("wasm32-wasip2", "wasm-release"),
        ] {
            let mut library = target();
            library["name"] = json!("regex_syntax");
            let command = direct_command(
                &request(),
                Path::new("/fake_sysroot/bin/rustc"),
                triple,
                &library,
                Path::new("/fake_temp/layout.fresh.o"),
                Path::new("/fake_target/deps"),
                &BTreeMap::new(),
            )
            .unwrap();
            let args: Vec<_> = command
                .get_args()
                .map(|arg| arg.to_str().unwrap())
                .collect();
            assert_eq!(
                args.iter()
                    .filter(|arg| arg.starts_with("--emit="))
                    .copied()
                    .collect::<Vec<_>>(),
                ["--emit=obj=/fake_temp/layout.fresh.o"]
            );
            assert!(args.windows(2).any(|pair| pair == ["--target", triple]));
            let prepared =
                prepare_command(&request(), Path::new("/fake_sysroot/bin/rustc"), triple).unwrap();
            let preparation = command_args(&prepared).unwrap();
            assert!(
                preparation
                    .windows(2)
                    .any(|pair| pair == ["--profile", expected_profile])
            );
            assert_eq!(
                command.get_current_dir(),
                Some(Path::new("/fake_workspace"))
            );
            let environment: BTreeMap<_, _> = command.get_envs().collect();
            assert_eq!(
                environment.get(OsStr::new("RUSTC")),
                Some(&Some(OsStr::new("/fake_sysroot/bin/rustc")))
            );
        }
        direct_command(
            &request(),
            Path::new("/fake_sysroot/bin/rustc"),
            "wasm32-wasip1",
            &target(),
            Path::new("relative.o"),
            Path::new("/fake_target/deps"),
            &BTreeMap::new(),
        )
        .unwrap_err();
    }

    #[test]
    fn compiler_layout_checks_structured_command_with_spaces_and_final_flags() {
        let compiler = Path::new("/fake sysroot/bin/rustc");
        let object = Path::new("/fake temp/layout.fresh.o");
        let make = || {
            direct_command(
                &request(),
                compiler,
                "wasm32-wasip1",
                &target(),
                object,
                Path::new("/fake_target/deps"),
                &BTreeMap::new(),
            )
            .unwrap()
        };
        let verify = |command: &Command| verify_command(command, compiler, "wasm32-wasip1", object);
        assert_eq!(verify(&make()), Ok(()));
        let mut wrong_compiler = make();
        wrong_compiler.env("RUSTC", "/fake_other/rustc");
        verify(&wrong_compiler).unwrap_err();
        for flag in [
            "-Cdebuginfo=0",
            "-Clinker-plugin-lto=yes",
            "--emit=obj=/fake_other.o",
        ] {
            let mut changed = make();
            changed.arg(flag);
            verify(&changed).unwrap_err();
        }
        let mut bootstrap = make();
        bootstrap.env("RUSTC_BOOTSTRAP", "1");
        verify(&bootstrap).unwrap_err();
        verify(
            &direct_command(
                &request(),
                compiler,
                "wasm32-wasip2",
                &target(),
                object,
                Path::new("/fake_target/deps"),
                &BTreeMap::new(),
            )
            .unwrap(),
        )
        .unwrap_err();
    }

    #[test]
    fn compiler_layout_feature_refusal_is_bounded_without_unrelated_stdout() {
        let mut rows = messages(false);
        rows[0]["features"] = json!(["x".repeat(0x4000)]);
        rows[1]["fake_unrelated_stdout"] = json!("must_not_appear_in_error");
        let error = receipt(&rows, false).unwrap_err();
        assert!(error.starts_with("compiler layout feature mismatch: "));
        assert!(error.len() <= 4096);
        assert!(!error.contains("must_not_appear_in_error"));
        assert!(error.contains("truncated"));
    }

    #[test]
    fn compiler_layout_input_binding_refuses_changed_missing_and_extra_sources() {
        let before = BTreeMap::from([
            (PathBuf::from("Cargo.toml"), "fake_manifest_hash".into()),
            (PathBuf::from("Cargo.lock"), "fake_lock_hash".into()),
            (
                PathBuf::from("source/src/lib.rs"),
                "fake_source_hash".into(),
            ),
        ]);
        assert_eq!(unchanged(&before, &before), Ok(()));
        for path in before.keys() {
            let mut after = before.clone();
            after.insert(path.clone(), "fake_changed_hash".into());
            assert_eq!(
                unchanged(&before, &after).unwrap_err(),
                "compiler layout inputs changed during build"
            );
            after.remove(path);
            unchanged(&before, &after).unwrap_err();
        }
        let mut after = before.clone();
        after.insert(
            PathBuf::from("source/src/extra.rs"),
            "fake_extra_hash".into(),
        );
        unchanged(&before, &after).unwrap_err();
    }

    #[test]
    fn compiler_layout_reports_static_facts_without_execution_or_resource_claims() {
        let report = static_report(
            &request(),
            &json!({"fake_binding":"fake_hash"}),
            &json!({"fake_parser_result":true}),
        );
        assert_eq!(report["kind"], "compiler_static_layout_observation");
        assert_eq!(report["qualification"], "Unqualified");
        assert_eq!(report["execution"], "not_executed");
        assert_eq!(report["static_layout_only"], true);
        assert_eq!(report["resource_qualification"], false);
        assert_eq!(report["serving_qualification"], false);
        assert_eq!(report["binding"]["fake_binding"], "fake_hash");
        assert_eq!(report["layouts"]["fake_parser_result"], true);
        assert_eq!(report["qualified_types"], json!(["regex_syntax::hir::Hir"]));
    }

    #[test]
    fn compiler_layout_archive_verification_refuses_corrupt_archive_and_source() {
        use flate2::{Compression, write::GzEncoder};
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary
            .path()
            .join("registry/src/fake_index/fake_crate-0.0.0");
        let cache = temporary.path().join("registry/cache/fake_index");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(&cache).unwrap();
        let source = b"fake archived source bytes";
        fs::write(root.join("src/lib.rs"), source).unwrap();
        let mut archive = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
        let mut header = tar::Header::new_gnu();
        header.set_size(u64::try_from(source.len()).unwrap());
        header.set_mode(0o644);
        header.set_cksum();
        archive
            .append_data(
                &mut header,
                "fake_crate-0.0.0/src/lib.rs",
                source.as_slice(),
            )
            .unwrap();
        let archive_bytes = archive.into_inner().unwrap().finish().unwrap();
        let path = cache.join("fake_crate-0.0.0.crate");
        fs::write(&path, &archive_bytes).unwrap();
        let package = json!({"name":"fake_crate", "version":"0.0.0",
            "source":"registry+https://github.com/rust-lang/crates.io-index"});
        let lock = json!({"version":4_u8, "package":[{"name":"fake_crate", "version":"0.0.0",
            "source":package["source"], "checksum":digest(&archive_bytes)}]});
        fs::write(
            temporary.path().join("Cargo.lock"),
            toml::to_string(&lock).unwrap(),
        )
        .unwrap();
        let verify = || {
            verify_registry(
                temporary.path(),
                &package,
                &root,
                &source_tree(&root).unwrap(),
            )
        };
        verify().unwrap();
        fs::write(root.join("src/lib.rs"), b"fake corrupt source").unwrap();
        assert_eq!(
            verify().unwrap_err(),
            "compiler layout source differs from locked crate archive"
        );
        fs::write(root.join("src/lib.rs"), source).unwrap();
        fs::write(&path, b"fake corrupt archive").unwrap();
        assert_eq!(
            verify().unwrap_err(),
            "compiler layout archive checksum mismatch"
        );
        fs::remove_file(&path).unwrap();
        verify().unwrap_err();
    }

    #[test]
    fn compiler_layout_output_policy_refuses_extra_symlink_and_oversized_files() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("layout.fresh.o");
        fs::write(&path, b"fake object").unwrap();
        check_outputs(temporary.path()).unwrap();
        let extra = temporary.path().join("extra.o");
        fs::write(&extra, b"fake extra").unwrap();
        assert_eq!(
            check_outputs(temporary.path()).unwrap_err(),
            "compiler layout unexpected output"
        );
        fs::remove_file(&extra).unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(MAX_BYTES)
            .unwrap();
        check_outputs(temporary.path()).unwrap();
        fs::write(temporary.path().join("layout.wasm"), [1]).unwrap();
        assert_eq!(
            check_outputs(temporary.path()).unwrap_err(),
            "compiler layout output byte cap"
        );
        fs::remove_file(temporary.path().join("layout.wasm")).unwrap();
        fs::remove_file(&path).unwrap();
        symlink("fake_missing", &path).unwrap();
        check_outputs(temporary.path()).unwrap_err();
        read_file(&path, MAX_BYTES).unwrap_err();
    }

    #[test]
    fn compiler_layout_native_dependency_profile_respects_existing_package_override() {
        let temporary = tempfile::tempdir().unwrap();
        fs::write(
            temporary.path().join("Cargo.toml"),
            toml::to_string(&json!({"profile":{
                "release":{"debug":1_u8, "package":{"*":{"debug":false}}},
                "wasm-release":{"inherits":"release", "debug":false}
            }}))
            .unwrap(),
        )
        .unwrap();
        let metadata = json!({"workspace_members":["fake_member"]});
        for (id, release) in [("fake_member", 1), ("fake_dependency", 0)] {
            let package = json!({"id":id, "name":"fake_crate"});
            assert_eq!(
                profile_debug(temporary.path(), &metadata, &package, "release").unwrap(),
                release
            );
            assert_eq!(
                profile_debug(temporary.path(), &metadata, &package, "wasm-release").unwrap(),
                0
            );
        }
    }

    #[derive(Clone, Debug)]
    struct FixtureSection {
        data: EndianVec<LittleEndian>,
        relocations: Vec<DebugRelocation>,
    }

    impl RelocateWriter for FixtureSection {
        type Writer = EndianVec<LittleEndian>;
        fn writer(&self) -> &Self::Writer {
            &self.data
        }
        fn writer_mut(&mut self) -> &mut Self::Writer {
            &mut self.data
        }
        fn relocate(&mut self, relocation: DebugRelocation) {
            self.relocations.push(relocation);
        }
    }

    fn native_stmt_list_fixture(producer: &str, format: object::BinaryFormat) -> Vec<u8> {
        use gimli::write::{
            AttributeValue, DwarfUnit, LineProgram, LineString, RelocationTarget, Sections,
        };
        use gimli::{Encoding, Format, LineEncoding};
        use std::convert::Infallible;

        let encoding = Encoding {
            format: Format::Dwarf32,
            version: 5,
            address_size: 8,
        };
        let mut dwarf = DwarfUnit::new(encoding);
        dwarf.strings.add(b"fake producer padding");
        let producer_id = dwarf.strings.add(producer.as_bytes());
        dwarf.unit.line_program = LineProgram::new(
            encoding,
            LineEncoding::default(),
            LineString::String(b"/fake/source".to_vec()),
            None,
            LineString::String(b"fake.rs".to_vec()),
            None,
        );
        let root = dwarf.unit.get_mut(dwarf.unit.root());
        root.set(gimli::DW_AT_stmt_list, AttributeValue::LineProgramRef);
        root.set(
            gimli::DW_AT_producer,
            AttributeValue::StringRef(producer_id),
        );
        let mut sections = Sections::new(FixtureSection {
            data: EndianVec::new(LittleEndian),
            relocations: Vec::new(),
        });
        dwarf.write(&mut sections).unwrap();
        let mut object = WriteObject::new(
            format,
            object::Architecture::Aarch64,
            object::Endianness::Little,
        );
        let mut ids = Vec::new();
        sections
            .for_each(|id, section| {
                if !section.data.slice().is_empty() {
                    let name = if format == object::BinaryFormat::MachO {
                        format!("__{}", id.name().trim_start_matches('.')).into_bytes()
                    } else {
                        id.name().as_bytes().to_vec()
                    };
                    let index =
                        object.add_section(b"__DWARF".to_vec(), name, object::SectionKind::Debug);
                    object.append_section_data(index, section.data.slice(), 1);
                    ids.push((id, index));
                }
                Ok::<_, Infallible>(())
            })
            .unwrap();
        sections
            .for_each(|id, section| {
                for relocation in &section.relocations {
                    let RelocationTarget::Section(target_id) = relocation.target else {
                        panic!("section relocation")
                    };
                    let source = ids
                        .iter()
                        .find(|(candidate, _)| *candidate == id)
                        .unwrap()
                        .1;
                    let target = ids
                        .iter()
                        .find(|(candidate, _)| *candidate == target_id)
                        .unwrap()
                        .1;
                    let symbol = object.section_symbol(target);
                    object
                        .add_relocation(
                            source,
                            ObjectRelocation {
                                offset: u64::try_from(relocation.offset).unwrap(),
                                symbol,
                                addend: relocation.addend,
                                flags: object::RelocationFlags::Generic {
                                    kind: object::RelocationKind::Absolute,
                                    encoding: object::RelocationEncoding::Generic,
                                    size: relocation.size.checked_mul(8).unwrap(),
                                },
                            },
                        )
                        .unwrap();
                }
                Ok::<_, Infallible>(())
            })
            .unwrap();
        object.write().unwrap()
    }

    #[test]
    fn compiler_layout_review_stmt_list_accepts_relocated_native_line_header() {
        let compiler = "rustc 1.98.1 (fakehash 2026-10-12)\nrelease: 1.98.1\n";
        let producer = "clang LLVM (rustc version 1.98.1 (fakehash 2026-10-12))";
        for format in [object::BinaryFormat::Elf, object::BinaryFormat::MachO] {
            let payload = native_stmt_list_fixture(producer, format);
            let observed = payload_identity(&payload, "aarch64-apple-darwin", compiler).unwrap();
            assert_eq!(observed["dwarf_producers"], json!([producer]));
        }
    }

    #[test]
    fn compiler_layout_review_stmt_list_still_refuses_wrong_producer() {
        let compiler = "rustc 1.98.1 (fakehash 2026-10-12)\nrelease: 1.98.1\n";
        for format in [object::BinaryFormat::Elf, object::BinaryFormat::MachO] {
            let payload = native_stmt_list_fixture("clang LLVM (rustc version 1.98.0)", format);
            assert_eq!(
                payload_identity(&payload, "aarch64-apple-darwin", compiler).unwrap_err(),
                "compiler layout DWARF producer mismatch"
            );
        }
    }

    #[test]
    fn compiler_layout_review_elf_tls_locations_do_not_hide_the_producer() {
        use super::super::native_dwarf::tests::{Site, elf_fixture};

        let compiler = "rustc 1.98.1 (fakehash 2026-10-12)\nrelease: 1.98.1\n";
        for site in [Site::TlsExpression, Site::Tls32Expression] {
            let bytes = elf_fixture(site);
            let observed = payload_identity(&bytes, "x86_64-unknown-linux-gnu", compiler).unwrap();
            assert_eq!(
                observed["dwarf_producers"],
                json!(["clang LLVM (rustc version 1.98.1 (fakehash 2026-10-12))"])
            );
        }
    }

    #[test]
    fn compiler_layout_review_elf_tls_cannot_supply_a_producer_offset() {
        use super::super::native_dwarf::tests::{Site, elf_fixture};

        let compiler = "rustc 1.98.1 (fakehash 2026-10-12)\nrelease: 1.98.1\n";
        let bytes = elf_fixture(Site::TlsProducer);
        assert_eq!(
            payload_identity(&bytes, "x86_64-unknown-linux-gnu", compiler).unwrap_err(),
            gimli::Error::UnsupportedOffset.to_string()
        );
    }

    #[test]
    fn compiler_layout_review_compiler_pin_is_independent_of_reported_producer() {
        let compiler = "rustc 1.98.1 (fakehash 2026-10-12)\nrelease: 1.98.1\n";
        assert_eq!(
            verify_compiler_pin("nodejs 24.20.0\nrust 1.98.1\n", compiler).unwrap(),
            "1.98.1"
        );
        for tools in [
            "",
            "rust 1.98.0\n",
            "rust 1.98.1 1.98.0\n",
            "rust 1.98.1\nrust 1.98.1\n",
        ] {
            verify_compiler_pin(tools, compiler).expect_err(tools);
        }
        verify_compiler_pin("rust 1.98.1\n", &compiler.replace("1.98.1", "1.98.0")).unwrap_err();
        verify_compiler_pin("rust 1.98.1\n", "rustc 1.98.10\nrelease: 1.98.10\n").unwrap_err();
    }

    fn engine_package() -> Value {
        let manifest: toml::Value =
            toml::from_str(include_str!("../../../../vendor/regex-automata/Cargo.toml")).unwrap();
        let dependencies: Vec<_> = manifest["dependencies"].as_table().unwrap().iter()
            .map(|(name, row)| json!({"name":name, "rename":null, "kind":null,
                "target":null, "optional":row.get("optional").and_then(toml::Value::as_bool).unwrap_or(false)}))
            .collect();
        json!({"id":"fake_engine_id", "name":"regex-automata",
            "features":manifest["features"], "dependencies":dependencies})
    }

    #[test]
    fn compiler_layout_replay_prepares_the_normal_staging_graph_without_leaf_features() {
        for (target, expected_profile) in [
            ("aarch64-apple-darwin", "release"),
            ("wasm32-wasip2", "wasm-release"),
        ] {
            let command =
                prepare_command(&request(), Path::new("/fake sysroot/bin/rustc"), target).unwrap();
            let args = command_args(&command).unwrap();
            assert_eq!(
                args,
                [
                    "build",
                    "--locked",
                    "--offline",
                    "-j",
                    "2",
                    "-p",
                    "auctioneer-bidder",
                    "--features",
                    "schema-artifact-staging",
                    "--lib",
                    "--profile",
                    expected_profile,
                    "--target",
                    target,
                    "--message-format=json-render-diagnostics"
                ]
            );
            assert!(
                !args
                    .iter()
                    .any(|arg| arg == "--no-default-features" || arg == "regex-syntax")
            );
            let env: BTreeMap<_, _> = command.get_envs().collect();
            assert_eq!(env.get(OsStr::new("RUSTC_BOOTSTRAP")), Some(&None));
        }
    }

    #[test]
    fn compiler_layout_replay_direct_rustc_has_only_verified_feature_cfgs_and_externs() {
        let compiler = Path::new("/fake sysroot/bin/rustc");
        let output = Path::new("/fake output/layout.fresh.o");
        let deps = Path::new("/fake target/wasm32-wasip1/wasm-release/deps");
        let externs = BTreeMap::from([
            (
                "aho_corasick".into(),
                deps.join("libaho_corasick-fake.rlib"),
            ),
            ("memchr".into(), deps.join("libmemchr-fake.rlib")),
            (
                "regex_syntax".into(),
                deps.join("libregex_syntax-fake.rlib"),
            ),
        ]);
        let mut request = request();
        request.package = "regex-automata";
        request.features = &super::super::schema_regex_vendor::FEATURES;
        let library = json!({"name":"regex_automata", "edition":"2021", "kind":["lib"],
            "crate_types":["lib"], "src_path":"/fake source/src/lib.rs"});
        let command = direct_command(
            &request,
            compiler,
            "wasm32-wasip1",
            &library,
            output,
            deps,
            &externs,
        )
        .unwrap();
        assert_eq!(command.get_program(), compiler.as_os_str());
        let args = command_args(&command).unwrap();
        for pair in [
            ["--crate-name", "regex_automata"],
            ["--edition", "2021"],
            ["--crate-type", "rlib"],
            ["--target", "wasm32-wasip1"],
            [
                "-L",
                "dependency=/fake target/wasm32-wasip1/wasm-release/deps",
            ],
        ] {
            assert_eq!(
                args.windows(2).filter(|values| *values == pair).count(),
                1,
                "{pair:?}"
            );
        }
        for flag in [
            "-Copt-level=3",
            "-Ccodegen-units=1",
            "-Cdebug-assertions=no",
            "-Coverflow-checks=no",
            "-Cdebuginfo=2",
            "-Clinker-plugin-lto=no",
            "--emit=obj=/fake output/layout.fresh.o",
            "/fake source/src/lib.rs",
        ] {
            assert_eq!(
                args.iter().filter(|arg| arg.as_str() == flag).count(),
                1,
                "{flag}"
            );
        }
        let cfgs = args
            .windows(2)
            .filter(|pair| pair[0] == "--cfg")
            .map(|pair| pair[1].clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            cfgs,
            request
                .features
                .iter()
                .map(|feature| format!("feature=\"{feature}\""))
                .collect()
        );
        assert!(!cfgs.contains("feature=\"default\""));
        assert!(!cfgs.contains("feature=\"dfa-build\""));
        let active = args
            .windows(2)
            .filter(|pair| pair[0] == "--extern")
            .map(|pair| pair[1].clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            active,
            externs
                .iter()
                .map(|(name, path)| format!("{name}={}", path.display()))
                .collect()
        );
        assert!(
            !args
                .iter()
                .any(|arg| arg == "--features" || arg == "--no-default-features")
        );
        let env: BTreeMap<_, _> = command.get_envs().collect();
        assert_eq!(env.get(OsStr::new("RUSTC_BOOTSTRAP")), Some(&None));
    }

    #[test]
    fn compiler_layout_replay_active_optional_dependencies_match_the_current_engine() {
        assert_eq!(
            active_dependencies(
                &engine_package(),
                &super::super::schema_regex_vendor::FEATURES
            )
            .unwrap(),
            BTreeSet::from([
                "aho_corasick".into(),
                "memchr".into(),
                "regex_syntax".into()
            ])
        );
        let syntax = json!({"dependencies":[{"name":"arbitrary", "rename":null,
            "kind":null, "target":null, "optional":true}], "features":{
            "std":[], "unicode":["unicode-age","unicode-bool","unicode-case","unicode-gencat",
                "unicode-perl","unicode-script","unicode-segment"],
            "unicode-age":[], "unicode-bool":[], "unicode-case":[], "unicode-gencat":[],
            "unicode-perl":[], "unicode-script":[], "unicode-segment":[], "arbitrary":["dep:arbitrary"]}});
        assert!(
            active_dependencies(
                &syntax,
                &[
                    "std",
                    "unicode",
                    "unicode-age",
                    "unicode-bool",
                    "unicode-case",
                    "unicode-gencat",
                    "unicode-perl",
                    "unicode-script",
                    "unicode-segment"
                ]
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn compiler_layout_replay_dependency_activation_distinguishes_weak_and_strong_edges() {
        let package = json!({"dependencies":[
            {"name":"always", "kind":null, "target":null, "rename":null, "optional":false},
            {"name":"original", "kind":null, "target":null, "rename":"alias-dep", "optional":true},
            {"name":"fake_dev", "kind":"dev", "target":null, "rename":null, "optional":false},
            {"name":"fake_build", "kind":"build", "target":null, "rename":null, "optional":false}],
            "features":{"weak":["alias-dep?/std"], "strong":["alias-dep/std"],
                "explicit":["dep:alias-dep"], "nested":["explicit"]}});
        assert_eq!(
            active_dependencies(&package, &["weak"]).unwrap(),
            BTreeSet::from(["always".into()])
        );
        for features in [&["strong"][..], &["explicit"], &["nested"]] {
            assert_eq!(
                active_dependencies(&package, features).unwrap(),
                BTreeSet::from(["always".into(), "alias_dep".into()])
            );
        }
        active_dependencies(&package, &["fake_unknown_feature"]).unwrap_err();
    }

    fn extern_receipts(deps: &Path) -> (Value, Vec<Value>) {
        let mut nodes = Vec::new();
        let mut receipts = Vec::new();
        for name in ["aho_corasick", "memchr", "regex_syntax", "log"] {
            let id = format!("fake_{name}_id");
            nodes.push(json!({"name":name, "pkg":id, "dep_kinds":[{"kind":null,"target":null}]}));
            receipts.push(json!({"reason":"compiler-artifact", "package_id":id,
                "target":{"name":name, "kind":["lib"]},
                "filenames":[deps.join(format!("lib{name}-fake.rlib"))]}));
        }
        (json!({"id":"fake_engine_id", "deps":nodes}), receipts)
    }

    #[test]
    fn compiler_layout_replay_externs_are_exact_active_package_receipts() {
        let deps = Path::new("/fake target/wasm32-wasip1/wasm-release/deps");
        let (node, receipts) = extern_receipts(deps);
        let features = &super::super::schema_regex_vendor::FEATURES;
        let selected =
            prepared_externs(&engine_package(), features, &node, &receipts, deps).unwrap();
        assert_eq!(
            selected.keys().map(String::as_str).collect::<Vec<_>>(),
            ["aho_corasick", "memchr", "regex_syntax"]
        );
        assert!(!selected.contains_key("log"));
        let mut missing = receipts.clone();
        missing.remove(1);
        assert_eq!(
            prepared_externs(&engine_package(), features, &node, &missing, deps).unwrap_err(),
            "compiler layout active extern receipt missing"
        );
        let mut duplicate = receipts.clone();
        duplicate.push(receipts[0].clone());
        assert_eq!(
            prepared_externs(&engine_package(), features, &node, &duplicate, deps).unwrap_err(),
            "ambiguous compiler layout active extern receipt"
        );
        let mut foreign = receipts.clone();
        foreign[0]["filenames"] = json!(["/fake_other/libaho_corasick-fake.rlib"]);
        assert_eq!(
            prepared_externs(&engine_package(), features, &node, &foreign, deps).unwrap_err(),
            "compiler layout extern artifact escapes target deps"
        );
        let mut rmeta = receipts.clone();
        rmeta[0]["filenames"] = json!([deps.join("libaho_corasick-fake.rmeta")]);
        assert_eq!(
            prepared_externs(&engine_package(), features, &node, &rmeta, deps).unwrap_err(),
            "compiler layout active extern rlib missing"
        );
    }
}
