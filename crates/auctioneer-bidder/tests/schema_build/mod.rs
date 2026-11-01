#![cfg(test)]

use crate::schema_profile;
use crate::schema_qualification::{Draft, Inventory, SchemaInventory, fixture, inventory};
use crate::schema_reference;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use rustix::process::{Pid, Signal, kill_process_group};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{ErrorKind, Read};
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::str;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use syn::parse::Parser as _;
use tempfile::TempDir;
use toml::Value;

pub mod application;
pub mod bare;
pub mod compiler_layout;
pub mod guest;
pub mod inspection;
mod layout_api;
pub(crate) mod native_dwarf;
mod schema_artifact;
pub mod schema_regex_vendor;
pub mod selected;
use crate::schema_vendor;

const PACKAGE: &str = "auctioneer-schema-link-probe";
const MAX_STREAM_LOG_BYTES: u64 = 4 << 20;
const MAX_DEADLINE_DIAGNOSTIC_BYTES: usize = 4096;
const MAX_ARTIFACT_BYTES: u64 = 64 << 20;
const MAX_SOURCE_BYTES: usize = 512 << 10;
const BUILD_TIMEOUT: Duration = Duration::from_secs(300);
const TARGETS: [&str; 4] = [
    "native",
    "wasm32-unknown-unknown",
    "wasm32-wasip1",
    "wasm32-wasip2",
];

#[derive(Clone, Copy)]
enum SourceMode {
    Default,
    Explicit,
}

impl SourceMode {
    fn cache_option(self) -> TokenStream {
        match self {
            Self::Default => quote! {},
            Self::Explicit => quote! { cache = explicit, },
        }
    }
}

struct ExplicitRefusal {
    name: &'static str,
    body: &'static str,
    owner_items: &'static str,
    diagnostics: &'static [&'static str],
}

const EXPLICIT_POSITIVE_BODY: &str = r#"
    let value = serde_json::json!("prefix123suffix");
    let mut first = owners::First::new_search_cache().unwrap();
    let mut second = owners::Second::new_search_cache().unwrap();
    let mut empty = owners::Empty::new_search_cache().unwrap();
    let _ = owners::First::initialize_regexes();
    let _ = owners::First::is_valid(&value, &mut first);
    let _ = owners::Second::is_valid(&value, &mut second);
    let _ = owners::Empty::is_valid(&value, &mut empty);
    let _ = owners::Legacy::is_valid(&value);
"#;

const EXPLICIT_REFUSALS: [ExplicitRefusal; 7] = [
    ExplicitRefusal {
        name: "wrong-cache",
        body: "let mut cache = owners::Second::new_search_cache().unwrap(); let _ = owners::First::is_valid(&serde_json::Value::Null, &mut cache);",
        owner_items: "",
        diagnostics: &[
            "error[E0308]",
            "mismatched types",
            "FirstSearchCache",
            "SecondSearchCache",
        ],
    },
    ExplicitRefusal {
        name: "missing-context",
        body: "let _ = owners::First::is_valid(&serde_json::Value::Null);",
        owner_items: "",
        diagnostics: &["error[E0061]", "2 arguments", "FirstSearchCache"],
    },
    ExplicitRefusal {
        name: "default",
        body: "let _: owners::FirstSearchCache = Default::default();",
        owner_items: "",
        diagnostics: &[
            "error[E0277]",
            "FirstSearchCache",
            "Default",
            "not satisfied",
        ],
    },
    ExplicitRefusal {
        name: "clone",
        body: "let cache = owners::First::new_search_cache().unwrap(); let _ = cache.clone();",
        owner_items: "",
        diagnostics: &[
            "error[E0599]",
            "no method named `clone`",
            "FirstSearchCache",
        ],
    },
    ExplicitRefusal {
        name: "immutable",
        body: "let cache = owners::First::new_search_cache().unwrap(); let _ = owners::First::is_valid(&serde_json::Value::Null, &mut cache);",
        owner_items: "",
        diagnostics: &[
            "error[E0596]",
            "cannot borrow `cache` as mutable",
            "not declared as mutable",
        ],
    },
    ExplicitRefusal {
        name: "private-field",
        body: "let cache = owners::First::new_search_cache().unwrap(); let _ = &cache.__inner;",
        owner_items: "",
        diagnostics: &[
            "error[E0616]",
            "field `__inner`",
            "FirstSearchCache",
            "private",
        ],
    },
    ExplicitRefusal {
        name: "name-collision",
        body: "",
        owner_items: "pub struct FirstSearchCache;",
        diagnostics: &["error[E0428]", "FirstSearchCache", "defined multiple times"],
    },
];

const EXPLICIT_OPTION_REFUSALS: [ExplicitRefusal; 4] = [
    ExplicitRefusal {
        name: "incompatible-engine",
        body: "",
        owner_items: r#"#[jsonschema::validator(schema = "{}", pattern_options = { engine = fancy_regex, cache = explicit }, methods = { is_valid = true, validate = false, iter_errors = false })] pub struct Incompatible;"#,
        diagnostics: &["error: `cache = explicit` requires `engine = regex`"],
    },
    ExplicitRefusal {
        name: "incompatible-backend",
        body: "",
        owner_items: r#"#[jsonschema::validator(schema = "{}", backend = Pyo3, pattern_options = { engine = regex, cache = explicit }, methods = { is_valid = true, validate = false, iter_errors = false })] pub struct Incompatible;"#,
        diagnostics: &["error: `cache = explicit` requires `backend = SerdeJson`"],
    },
    ExplicitRefusal {
        name: "enabled-validation-errors",
        body: "",
        owner_items: r#"#[jsonschema::validator(schema = "{}", pattern_options = { engine = regex, cache = explicit }, methods = { is_valid = true, validate = true, iter_errors = false })] pub struct Incompatible;"#,
        diagnostics: &[
            "error: `cache = explicit` requires boolean-only methods: `is_valid = true, validate = false, iter_errors = false`",
        ],
    },
    ExplicitRefusal {
        name: "enabled-error-iterator",
        body: "",
        owner_items: r#"#[jsonschema::validator(schema = "{}", pattern_options = { engine = regex, cache = explicit }, methods = { is_valid = true, validate = false, iter_errors = true })] pub struct Incompatible;"#,
        diagnostics: &[
            "error: `cache = explicit` requires boolean-only methods: `is_valid = true, validate = false, iter_errors = false`",
        ],
    },
];

pub struct LinkReport {
    pub target: String,
    pub validators: usize,
    pub artifact_bytes: u64,
    pub sha256: String,
    pub names: Vec<String>,
    pub results: Vec<bool>,
    pub wasm: Option<schema_artifact::WasmArtifact>,
}

pub struct Catalog {
    reports: Vec<LinkReport>,
    directory: TempDir,
    native: PathBuf,
    target_dir: PathBuf,
    _lease: File,
    _inputs: Option<fixture::Fixture>,
}

pub struct Output {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum RunError {
    Io,
    Deadline { stderr: String },
    OutputLimit,
}

type PackageIdentity = (String, String, Option<String>, Option<String>);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeInventory {
    names: Vec<String>,
    results: Vec<bool>,
    membership: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeVerdict {
    valid: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CacheLayout {
    pub name: String,
    pub inline_bytes: usize,
    pub alignment: usize,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CacheLayouts {
    pub schema_version: u8,
    pub word_bytes: usize,
    pub cache_bytes: usize,
    pub cache_alignment: usize,
    pub rows: Vec<CacheLayout>,
}

impl CacheLayouts {
    pub fn validate(&self, word_bytes: usize, expected: &[(&str, usize)]) -> Result<(), String> {
        if self.schema_version != 2 {
            return Err("cache layout version mismatch".to_owned());
        }
        if !matches!(word_bytes, 4 | 8) || self.word_bytes != word_bytes {
            return Err("cache layout word mismatch".to_owned());
        }
        if self.cache_bytes == 0
            || !self.cache_alignment.is_power_of_two()
            || !self.cache_bytes.is_multiple_of(self.cache_alignment)
        {
            return Err("invalid cache base layout".to_owned());
        }
        let mut names = BTreeSet::new();
        if expected.is_empty()
            || expected
                .iter()
                .any(|(name, _)| name.is_empty() || !names.insert(*name))
        {
            return Err("invalid cache wrapper membership".to_owned());
        }
        if self.rows.len() != expected.len() {
            return Err("cache wrapper count mismatch".to_owned());
        }
        for (row, (name, count)) in self.rows.iter().zip(expected) {
            if row.name != *name {
                return Err("cache wrapper identity mismatch".to_owned());
            }
            let bytes = count
                .checked_mul(self.cache_bytes)
                .ok_or("cache wrapper size overflow")?;
            let alignment = if *count == 0 { 1 } else { self.cache_alignment };
            if row.inline_bytes != bytes || row.alignment != alignment {
                return Err("cache wrapper layout mismatch".to_owned());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RawVerdict {
    InvalidJson,
    Schema { valid: bool },
}

pub fn manifest(root_manifest: &Value) -> Result<String, String> {
    let declared_patches = root_manifest
        .get("patch")
        .and_then(|patch| patch.get("crates-io"))
        .and_then(Value::as_table)
        .ok_or("missing compiler patch")?;
    let declared_compiler = declared_patches
        .get("jsonschema-macros-core")
        .and_then(Value::as_table);
    if declared_compiler.is_none_or(|compiler| {
        compiler.len() != 1
            || compiler.get("path").and_then(Value::as_str) != Some("vendor/jsonschema-macros-core")
    }) {
        return Err("unreviewed compiler patch".into());
    }
    if declared_patches
        .get("regex-automata")
        .and_then(Value::as_table)
        .is_none_or(|engine| {
            engine.len() != 1
                || engine.get("path").and_then(Value::as_str) != Some("vendor/regex-automata")
        })
    {
        return Err("unreviewed engine patch".into());
    }
    if declared_patches.len() != 2 {
        return Err("unreviewed compiler patch".into());
    }
    let vendor = schema_vendor::verify(&root())?;
    let engine = schema_regex_vendor::verify(&root()).map_err(|error| error.to_string())?;
    let mut compiler = toml::Table::new();
    compiler.insert(
        "path".into(),
        Value::String(vendor.to_str().ok_or("non UTF-8 compiler path")?.into()),
    );
    let mut patches = toml::Table::new();
    patches.insert("jsonschema-macros-core".into(), Value::Table(compiler));
    let mut declaration = toml::Table::new();
    declaration.insert(
        "path".into(),
        Value::String(
            engine
                .canonical_root
                .to_str()
                .ok_or("non UTF-8 engine path")?
                .into(),
        ),
    );
    patches.insert("regex-automata".into(), Value::Table(declaration));
    let mut patch = toml::Table::new();
    patch.insert("crates-io".into(), Value::Table(patches));
    let original = root_manifest.get("workspace").ok_or("missing workspace")?;
    let edition = original
        .get("package")
        .and_then(|value| value.get("edition"))
        .ok_or("missing edition")?;
    let sources = original.get("dependencies").ok_or("missing dependencies")?;
    let mut package = toml::Table::new();
    package.insert("name".into(), Value::String(PACKAGE.into()));
    package.insert("version".into(), Value::String("0.0.0".into()));
    package.insert("edition".into(), edition.clone());
    package.insert("publish".into(), Value::Boolean(false));
    let mut dependencies = toml::Table::new();
    for name in [
        "jsonschema",
        "regex",
        "regex-automata",
        "serde_json",
        "uuid-simd",
    ] {
        dependencies.insert(
            name.into(),
            sources
                .get(name)
                .ok_or("missing candidate dependency")?
                .clone(),
        );
    }
    let mut workspace = toml::Table::new();
    workspace.insert(
        "resolver".into(),
        original.get("resolver").ok_or("missing resolver")?.clone(),
    );
    let mut document = toml::Table::new();
    document.insert("package".into(), Value::Table(package));
    document.insert("workspace".into(), Value::Table(workspace));
    document.insert("dependencies".into(), Value::Table(dependencies));
    document.insert("patch".into(), Value::Table(patch));
    document.insert(
        "profile".into(),
        root_manifest
            .get("profile")
            .ok_or("missing profiles")?
            .clone(),
    );
    toml::to_string(&document).map_err(|error| error.to_string())
}

fn packages(text: &str) -> Result<Vec<PackageIdentity>, String> {
    let document: Value = toml::from_str(text).map_err(|error| format!("invalid lock: {error}"))?;
    let rows = document
        .get("package")
        .and_then(Value::as_array)
        .ok_or("missing packages")?;
    rows.iter()
        .map(|row| {
            let name = row
                .get("name")
                .and_then(Value::as_str)
                .ok_or("missing name")?;
            let version = row
                .get("version")
                .and_then(Value::as_str)
                .ok_or("missing version")?;
            let optional = |key| -> Result<Option<String>, String> {
                row.get(key)
                    .map(|value| {
                        value
                            .as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| format!("invalid {key}"))
                    })
                    .transpose()
            };
            Ok((
                name.to_owned(),
                version.to_owned(),
                optional("source")?,
                optional("checksum")?,
            ))
        })
        .collect()
}

pub fn verify_lock(child: &str, workspace: &str) -> Result<(), String> {
    let reviewed: BTreeSet<_> = packages(workspace)?.into_iter().collect();
    let mut seen = BTreeSet::new();
    let mut own = 0_usize;
    let mut compiler = false;
    let mut engine = false;
    for identity in packages(child)? {
        if !seen.insert(identity.clone()) {
            return Err("duplicate child package".into());
        }
        if identity == (PACKAGE.into(), "0.0.0".into(), None, None) {
            own = own.checked_add(1).ok_or("package overflow")?;
            continue;
        }
        if identity == ("jsonschema-macros-core".into(), "0.58.6".into(), None, None)
            && reviewed.contains(&identity)
        {
            compiler = true;
            continue;
        }
        if identity.0 == "regex-automata" {
            if identity != ("regex-automata".into(), "0.4.18".into(), None, None)
                || !reviewed.contains(&identity)
            {
                return Err("unreviewed patched engine".into());
            }
            engine = true;
            continue;
        }
        if identity.2.as_deref() != Some("registry+https://github.com/rust-lang/crates.io-index")
            || !reviewed.contains(&identity)
        {
            return Err(format!(
                "unreviewed child package: {} {}",
                identity.0, identity.1
            ));
        }
    }
    if own != 1 || !compiler {
        return Err("missing standalone harness closure".into());
    }
    if !engine {
        return Err("missing patched engine".into());
    }
    Ok(())
}

pub fn resolved_engine_features(
    source: &schema_regex_vendor::VerifiedSource,
) -> Result<Vec<String>, String> {
    let metadata = successful(
        cargo(&root()).args(["metadata", "--offline", "--locked", "--format-version", "1"]),
        Duration::from_secs(60),
    )?;
    schema_regex_vendor::verify_metadata(&metadata.stdout, &source.canonical_root)
}

pub fn verify_compiler_metadata(bytes: &[u8], vendor: &Path) -> Result<(), String> {
    let document: Json = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let packages = document
        .get("packages")
        .and_then(Json::as_array)
        .ok_or("missing compiler packages")?;
    let mut matching = packages.iter().filter(|package| {
        package.get("name").and_then(Json::as_str) == Some("jsonschema-macros-core")
    });
    let package = matching.next().ok_or("missing patched compiler")?;
    if matching.next().is_some()
        || package.get("version").and_then(Json::as_str) != Some("0.58.6")
        || !package.get("source").is_some_and(Json::is_null)
        || package.get("manifest_path").and_then(Json::as_str) != vendor.join("Cargo.toml").to_str()
    {
        return Err("compiler metadata does not resolve the reviewed vendor".into());
    }
    Ok(())
}

fn read_pipe(mut pipe: impl Read, exceeded: &AtomicBool) -> Result<Vec<u8>, RunError> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let count = pipe.read(&mut chunk).map_err(|_error| RunError::Io)?;
        if count == 0 {
            return Ok(bytes);
        }
        let length = bytes
            .len()
            .checked_add(count)
            .ok_or(RunError::OutputLimit)?;
        if u64::try_from(length).map_err(|_error| RunError::OutputLimit)? > MAX_STREAM_LOG_BYTES {
            exceeded.store(true, Ordering::Release);
            return Err(RunError::OutputLimit);
        }
        bytes.extend_from_slice(chunk.get(..count).ok_or(RunError::Io)?);
    }
}

pub fn run(command: &mut Command, timeout: Duration) -> Result<Output, RunError> {
    command
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|_error| RunError::Io)?;
    let stdout = child.stdout.take().ok_or(RunError::Io)?;
    let stderr = child.stderr.take().ok_or(RunError::Io)?;
    let exceeded = AtomicBool::new(false);
    thread::scope(|scope| {
        let out = scope.spawn(|| read_pipe(stdout, &exceeded));
        let err = scope.spawn(|| read_pipe(stderr, &exceeded));
        let started = Instant::now();
        let result = loop {
            if exceeded.load(Ordering::Acquire) {
                break Err(RunError::OutputLimit);
            }
            if started.elapsed() >= timeout {
                break Err(RunError::Deadline {
                    stderr: String::new(),
                });
            }
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => thread::sleep(Duration::from_millis(20)),
                Err(_error) => break Err(RunError::Io),
            }
        };
        // Also close inherited pipes held by any surviving descendants.
        let _signal_result = kill_process_group(Pid::from_child(&child), Signal::KILL);
        drop(child.wait());
        let stdout_result = out.join().map_err(|_panic| RunError::Io)?;
        let stderr_result = err.join().map_err(|_panic| RunError::Io)?;
        if matches!(result, Err(RunError::Deadline { .. })) {
            // A reader refusal cannot turn the deadline into a different result.
            let stderr_bytes = stderr_result.unwrap_or_default();
            // Keep the last completed phase, not a potentially long launch log.
            let byte_start = stderr_bytes
                .len()
                .saturating_sub(MAX_DEADLINE_DIAGNOSTIC_BYTES);
            let diagnostic =
                String::from_utf8_lossy(stderr_bytes.get(byte_start..).ok_or(RunError::Io)?);
            let text_start = diagnostic.ceil_char_boundary(
                diagnostic
                    .len()
                    .saturating_sub(MAX_DEADLINE_DIAGNOSTIC_BYTES),
            );
            return Err(RunError::Deadline {
                stderr: diagnostic.get(text_start..).ok_or(RunError::Io)?.to_owned(),
            });
        }
        Ok(Output {
            status: result?,
            stdout: stdout_result?,
            stderr: stderr_result?,
        })
    })
}

fn cargo(directory: &Path) -> Command {
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(directory)
        .env_remove("CARGO_BUILD_TARGET")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER");
    for (key, _) in env::vars_os() {
        if key.to_str().is_some_and(|name| {
            name.starts_with("CARGO_PROFILE_") || name.starts_with("CARGO_TARGET_")
        }) {
            command.env_remove(key);
        }
    }
    command
}

pub(super) fn successful(command: &mut Command, timeout: Duration) -> Result<Output, String> {
    let output = run(command, timeout).map_err(|error| format!("child refused: {error:?}"))?;
    if !output.status.success() {
        return Err(format!(
            "child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output)
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn literal(value: &str) -> syn::LitStr {
    syn::LitStr::new(value, proc_macro2::Span::call_site())
}

fn explicit_control_source(body: &str, owner_items: &str) -> Result<String, String> {
    let body_statements = syn::Block::parse_within
        .parse_str(body)
        .map_err(|error| format!("explicit control body syntax: {error}"))?;
    let owner_syntax = syn::parse_file(owner_items)
        .map_err(|error| format!("explicit control owner syntax: {error}"))?;
    Ok(quote! {
        mod owners {
            #[jsonschema::validator(
                schema = "{\"type\":\"string\",\"pattern\":\"[0-9]+\"}", draft = Draft4,
                pattern_options = { engine = regex, cache = explicit, size_limit = 65_536, dfa_size_limit = 65_536 },
                methods = { is_valid = true, validate = false, iter_errors = false }
            )]
            pub struct First;
            #[jsonschema::validator(
                schema = "{\"type\":\"string\",\"pattern\":\"[a-z]+\"}", draft = Draft4,
                pattern_options = { engine = regex, cache = explicit, size_limit = 65_536, dfa_size_limit = 65_536 },
                methods = { is_valid = true, validate = false, iter_errors = false }
            )]
            pub struct Second;
            #[jsonschema::validator(
                schema = "{}", draft = Draft4,
                pattern_options = { engine = regex, cache = explicit, size_limit = 65_536, dfa_size_limit = 65_536 },
                methods = { is_valid = true, validate = false, iter_errors = false }
            )]
            pub struct Empty;
            #[jsonschema::validator(
                schema = "{\"type\":\"string\",\"pattern\":\"[0-9]+\"}", draft = Draft4,
                pattern_options = { engine = regex, size_limit = 65_536, dfa_size_limit = 65_536 },
                methods = { is_valid = true, validate = false, iter_errors = false }
            )]
            pub struct Legacy;
            #owner_syntax
        }
        fn main() {
            #(#body_statements)*
        }
    }
    .to_string())
}

fn source(snapshot: &Inventory) -> Result<(String, String), String> {
    source_for(snapshot, SourceMode::Default)
}

fn source_prelude(mode: SourceMode) -> TokenStream {
    let allocation_module = match mode {
        SourceMode::Default => quote! { schema_default_allocation },
        SourceMode::Explicit => quote! { schema_explicit_allocation },
    };
    let replay_module = match mode {
        SourceMode::Default => quote! {},
        SourceMode::Explicit => quote! { mod schema_replay; },
    };
    quote! {
        const _: fn(
            &regex_automata::meta::Regex,
            &regex_automata::util::prefilter::Prefilter,
        ) -> (core::alloc::Layout, core::alloc::Layout) = |regex, prefilter| (
            regex.resource_layout().strategy_payload(),
            prefilter.resource_layout().matcher_payload(),
        );
        mod schema_formats;
        #replay_module
        use schema_formats::{reference_hostname_format, reference_uuid_format};
        #[cfg(not(target_arch = "wasm32"))]
        mod schema_meter;
        #[cfg(not(target_arch = "wasm32"))]
        mod schema_allocation;
        #[cfg(not(target_arch = "wasm32"))]
        mod #allocation_module;
    }
}

fn workload_source(snapshot: &Inventory) -> Result<TokenStream, String> {
    let mut workloads = workload_data(snapshot)?.into_iter().collect::<Vec<_>>();
    workloads.sort_by_key(|(_, data)| data["order"].as_u64().unwrap());
    let orders = workloads
        .iter()
        .map(|(_, data)| data["order"].as_u64().unwrap())
        .collect::<BTreeSet<_>>();
    if orders.len() != workloads.len() {
        return Err("duplicate workload order".into());
    }
    let rows = workloads.iter().map(|(name, data)| {
        let name_literal = literal(name);
        let data_literal = literal(&data.to_string());
        quote! { (#name_literal, #data_literal) }
    });
    Ok(quote! {
        #[cfg(not(target_arch = "wasm32"))]
        const WORKLOAD_DATA: &[(&str, &str)] = &[#(#rows,)*];
    })
}

fn alternate_source(
    row: &SchemaInventory,
    index: usize,
    path_literal: &syn::LitStr,
    draft: &syn::Ident,
    mode: SourceMode,
    data: Option<&Json>,
) -> Result<(TokenStream, TokenStream), String> {
    let mut alternate_controls = TokenStream::new();
    let mut alternate_rows = TokenStream::new();
    let cache_option = mode.cache_option();
    let draft4 = format_ident!("Draft4");
    // Native contrasts reuse the captured path without joining resource membership.
    for (field, operation, marker, control_draft, validate_formats) in [
        (
            "formats_disabled_valid",
            "check-formats-disabled",
            format_ident!("Unformatted{index}"),
            draft,
            false,
        ),
        (
            "draft4_valid",
            "check-draft4",
            format_ident!("Draft4Control{index}"),
            &draft4,
            true,
        ),
    ] {
        if !data.is_some_and(|declaration| {
            declaration["cases"]
                .as_array()
                .unwrap()
                .iter()
                .any(|case| case.get(field).is_some())
        }) {
            continue;
        }
        if field == "draft4_valid" && row.audit.draft != Draft::Draft202012 {
            return Err("alternate draft requires a newer declared draft".into());
        }
        let body = match mode {
            SourceMode::Default => quote! { #marker::is_valid(input) },
            SourceMode::Explicit => quote! {
                let Some(mut cache) = #marker::new_search_cache() else { return false; };
                #marker::is_valid(input, &mut cache)
            },
        };
        let check = format_ident!("check_{index}_{field}");
        alternate_controls.extend(quote! {
            #[jsonschema::validator(
                path = #path_literal, draft = #control_draft,
                validate_formats = #validate_formats, ignore_unknown_formats = false,
                formats = {
                    "uuid" => crate::reference_uuid_format,
                    "hostname" => crate::reference_hostname_format
                },
                pattern_options = { engine = regex, #cache_option size_limit = 65_536, dfa_size_limit = 65_536 },
                methods = { is_valid = true, validate = false, iter_errors = false }
            )]
            struct #marker;
            fn #check(input: &serde_json::Value) -> bool { #body }
        });
        let operation_literal = literal(operation);
        let name = literal(&row.witness.name);
        alternate_rows.extend(quote! { (#operation_literal, #name, #check), });
    }
    Ok((alternate_controls, alternate_rows))
}

struct PrimarySchema {
    body: TokenStream,
    path: syn::LitStr,
    draft: syn::Ident,
    row: TokenStream,
    initializer: TokenStream,
    variant: TokenStream,
    layout: TokenStream,
}

fn primary_schema(
    root: &Path,
    schema: &SchemaInventory,
    index: usize,
    mode: SourceMode,
) -> Result<PrimarySchema, String> {
    let draft = match schema.audit.draft {
        Draft::Draft4 => format_ident!("Draft4"),
        Draft::Draft202012 => format_ident!("Draft202012"),
        Draft::Unspecified | Draft::Other(_) => return Err("unqualified schema draft".into()),
    };
    if !schema.audit.unknown_keywords.is_empty()
        || !schema.audit.local_references.is_empty()
        || !schema.audit.external_references.is_empty()
    {
        return Err("unqualified keyword or reference".into());
    }
    let path = root
        .join(&schema.witness.path)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let path_literal = literal(path.to_str().ok_or("non UTF-8 schema path")?);
    let marker = format_ident!("Schema{index}");
    let cache_option = mode.cache_option();
    let mut body = quote! {
        #[jsonschema::validator(
            path = #path_literal, draft = #draft, validate_formats = true,
            ignore_unknown_formats = false,
            formats = {
                "uuid" => crate::reference_uuid_format,
                "hostname" => crate::reference_hostname_format
            },
            pattern_options = { engine = regex, #cache_option size_limit = 65_536, dfa_size_limit = 65_536 },
            methods = { is_valid = true, validate = false, iter_errors = false }
        )]
        struct #marker;
    };
    let name = literal(&schema.witness.name);
    let (row, variant, layout) = match mode {
        SourceMode::Default => (quote! { (#name, #marker::is_valid), }, quote! {}, quote! {}),
        SourceMode::Explicit => {
            let cache = format_ident!("Schema{index}SearchCache");
            let factory = format_ident!("new_cache_{index}");
            let check = format_ident!("check_schema_{index}");
            body.extend(quote! {
                fn #factory() -> Option<CacheOwner> {
                    #marker::new_search_cache().map(CacheOwner::#marker)
                }
                fn #check(input: &serde_json::Value, owner: &mut CacheOwner) -> bool {
                    match owner {
                        CacheOwner::#marker(cache) => #marker::is_valid(input, cache),
                        _ => false
                    }
                }
            });
            (
                quote! { (#name, #factory, #check), },
                quote! { #marker(#cache), },
                quote! {
                    (#name, std::mem::size_of::<#cache>(), std::mem::align_of::<#cache>()),
                },
            )
        }
    };
    Ok(PrimarySchema {
        body,
        path: path_literal,
        draft,
        row,
        initializer: quote! { (#name, #marker::initialize_regexes), },
        variant,
        layout,
    })
}

fn source_for(snapshot: &Inventory, mode: SourceMode) -> Result<(String, String), String> {
    let mut declarations = source_prelude(mode);
    let literal_controls = schema_profile::declarations(snapshot, "literal-controls.toml");
    let mut controls = TokenStream::new();
    let mut entries = TokenStream::new();
    declarations.extend(workload_source(snapshot)?);
    let mut rows = TokenStream::new();
    let mut initializers = TokenStream::new();
    let mut cache_variants = TokenStream::new();
    let mut cache_layouts = TokenStream::new();
    let mut membership = String::from("AUCTIONEER_SCHEMA_MEMBERS_V1|");
    for (index, row) in snapshot.schemas.iter().enumerate() {
        let generated = primary_schema(&snapshot.root, row, index, mode)?;
        let (row_controls, row_entries) = alternate_source(
            row,
            index,
            &generated.path,
            &generated.draft,
            mode,
            literal_controls.get(&row.witness.name),
        )?;
        controls.extend(row_controls);
        entries.extend(row_entries);
        declarations.extend(generated.body);
        rows.extend(generated.row);
        cache_variants.extend(generated.variant);
        cache_layouts.extend(generated.layout);
        initializers.extend(generated.initializer);
        membership.push_str(&row.witness.name);
        membership.push('|');
    }
    let membership_literal = literal(&membership);
    declarations.extend(dispatch_tables(
        mode,
        &membership_literal,
        &rows,
        &cache_variants,
        &cache_layouts,
    ));
    let bare_data = match mode {
        SourceMode::Default => None,
        SourceMode::Explicit => {
            let names = snapshot
                .schemas
                .iter()
                .map(|row| row.witness.name.as_str())
                .collect::<Vec<_>>();
            let captured =
                schema_reference::captured().map_err(|error| format!("bare capture: {error:?}"))?;
            Some(bare::captured_data(&names, &captured.cases)?)
        }
    };
    declarations.extend(source_epilogue(
        mode,
        &initializers,
        &controls,
        &entries,
        bare_data.as_deref(),
    )?);
    let generated = declarations.to_string();
    if generated.len() > MAX_SOURCE_BYTES {
        return Err("generated source limit".into());
    }
    Ok((generated, membership))
}

fn dispatch_tables(
    mode: SourceMode,
    membership: &syn::LitStr,
    rows: &TokenStream,
    cache_variants: &TokenStream,
    cache_layouts: &TokenStream,
) -> TokenStream {
    match mode {
        SourceMode::Default => quote! {
            const MEMBERSHIP: &str = #membership;
            type Validator = fn(&serde_json::Value) -> bool;
            static VALIDATORS: &[(&str, Validator)] = &[#rows];
        },
        SourceMode::Explicit => quote! {
            enum CacheOwner { #cache_variants }
            static CACHE_LAYOUTS: &[(&str, usize, usize)] = &[#cache_layouts];
            const MEMBERSHIP: &str = #membership;
            type CacheFactory = fn() -> Option<CacheOwner>;
            type Validator = fn(&serde_json::Value, &mut CacheOwner) -> bool;
            static VALIDATORS: &[(&str, CacheFactory, Validator)] = &[#rows];
        },
    }
}

fn source_epilogue(
    mode: SourceMode,
    initializers: &TokenStream,
    alternate_controls: &TokenStream,
    alternate_rows: &TokenStream,
    bare_data: Option<&str>,
) -> Result<TokenStream, String> {
    let probe = syn::parse_file(match mode {
        SourceMode::Default => include_str!("../support/schema_probe.rs"),
        SourceMode::Explicit => include_str!("../support/schema_explicit_probe.rs"),
    })
    .map_err(|error| error.to_string())?;
    let bare = match mode {
        SourceMode::Default => quote! {},
        SourceMode::Explicit => bare::source(bare_data.ok_or("missing bare input table")?),
    };
    Ok(quote! {
        type Initializer = fn() -> bool;
        static INITIALIZERS: &[(&str, Initializer)] = &[#initializers];
        #[cfg(not(target_arch = "wasm32"))]
        mod alternate_controls {
            #alternate_controls
            type Validator = fn(&serde_json::Value) -> bool;
            static CHECKS: &[(&str, &str, Validator)] = &[#alternate_rows];
            pub(super) fn check(operation: &str, name: &str, input: &serde_json::Value) -> Option<bool> {
                CHECKS.iter().find(|(mode, candidate, _)| *mode == operation && *candidate == name)
                    .map(|(_, _, validate)| validate(input))
            }
        }
        #probe
        #bare
    })
}

fn artifact(path: &Path, membership: &str, target: &str) -> Result<LinkReport, String> {
    let validators = membership
        .split('|')
        .count()
        .checked_sub(2)
        .ok_or("invalid membership")?;
    artifact_with_count(path, membership, target, validators)
}

fn artifact_with_count(
    path: &Path,
    membership: &str,
    target: &str,
    validators: usize,
) -> Result<LinkReport, String> {
    let size = fs::metadata(path).map_err(|error| error.to_string())?.len();
    if size == 0 || size > MAX_ARTIFACT_BYTES {
        return Err("artifact size limit".into());
    }
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let wasm = if target == "native" {
        None
    } else {
        Some(verify_wasm(&bytes, target)?)
    };
    if target == "wasm32-unknown-unknown" {
        verify_export_refusals(&bytes)?;
    }
    if !bytes
        .windows(membership.len())
        .any(|window| window == membership.as_bytes())
    {
        return Err("linked artifact lost schema membership".into());
    }
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    Ok(LinkReport {
        target: target.into(),
        validators,
        artifact_bytes: size,
        sha256,
        names: Vec::new(),
        results: Vec::new(),
        wasm,
    })
}

fn acquire_lease(target_dir: &Path) -> Result<File, String> {
    fs::create_dir_all(target_dir).map_err(|error| error.to_string())?;
    let lease = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root().join("target/schema-compiler-probe.lock"))
        .map_err(|error| error.to_string())?;
    let waiting = Instant::now();
    loop {
        match lease.try_lock() {
            Ok(()) => break,
            Err(TryLockError::WouldBlock) if waiting.elapsed() < Duration::from_mins(20) => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(error) => return Err(format!("schema build lease: {error}")),
        }
    }
    Ok(lease)
}

pub fn linked_catalog() -> Result<Catalog, String> {
    linked_catalog_for(SourceMode::Default, crate::SCHEMA_SOURCE_CONFORMANCE)
}

pub fn linked_explicit_catalog() -> Result<Catalog, String> {
    linked_catalog_for(SourceMode::Explicit, crate::SCHEMA_SOURCE_CONFORMANCE)
}

pub fn workload_data(snapshot: &Inventory) -> Result<BTreeMap<String, Json>, String> {
    workload_data_for(&snapshot.root, &snapshot.schemas)
}

pub fn workload_data_for(
    root: &Path,
    schemas: &[SchemaInventory],
) -> Result<BTreeMap<String, Json>, String> {
    let mut result = BTreeMap::new();
    let owners: BTreeSet<_> = schemas
        .iter()
        .filter_map(|row| row.witness.path.split('/').next())
        .collect();
    for owner in owners {
        let path = root.join(owner).join("tests/schema/workload.toml");
        if !path.try_exists().map_err(|error| error.to_string())? {
            continue;
        }
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 64 * 1024 {
            return Err("workload data path or size".into());
        }
        let bytes = fs::read(path).map_err(|error| error.to_string())?;
        if bytes.len() > 64 * 1024 {
            return Err("workload data limit".into());
        }
        let data: Json = serde_json::to_value(
            toml::from_str::<Value>(str::from_utf8(&bytes).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let name = data["schema"]
            .as_str()
            .ok_or("missing workload schema")?
            .to_owned();
        if !schemas.iter().any(|row| {
            row.witness.name == name && row.witness.path.split('/').next() == Some(owner)
        }) || result.insert(name, data).is_some()
        {
            return Err("workload ownership".into());
        }
    }
    Ok(result)
}

fn verify_probe_workspace(directory: &Path) -> Result<(), String> {
    // Seed versions, then let Cargo materialize a valid standalone closure.
    // The copied workspace lock alone is never accepted as a child lockfile.
    let reviewed =
        fs::read_to_string(root().join("Cargo.lock")).map_err(|error| error.to_string())?;
    fs::write(directory.join("Cargo.lock"), &reviewed).map_err(|error| error.to_string())?;
    let metadata = successful(
        cargo(directory).args(["metadata", "--offline", "--format-version", "1"]),
        Duration::from_secs(60),
    )?;
    verify_compiler_metadata(&metadata.stdout, &schema_vendor::verify(&root())?)?;
    let engine = schema_regex_vendor::verify(&root()).map_err(|error| error.to_string())?;
    schema_regex_vendor::verify_metadata(&metadata.stdout, &engine.canonical_root)?;
    let lock =
        fs::read_to_string(directory.join("Cargo.lock")).map_err(|error| error.to_string())?;
    verify_lock(&lock, &reviewed)
}

fn prepare_probe_workspace(
    snapshot: &Inventory,
    mode: SourceMode,
) -> Result<(TempDir, String), String> {
    let generated = match mode {
        SourceMode::Default => source(snapshot)?,
        SourceMode::Explicit => source_for(snapshot, mode)?,
    };
    prepare_generated_workspace(generated)
}

fn prepare_generated_workspace(
    (generated, membership): (String, String),
) -> Result<(TempDir, String), String> {
    let directory = prepare_workspace()?;
    fs::write(directory.path().join("src/main.rs"), generated)
        .map_err(|error| error.to_string())?;
    Ok((directory, membership))
}

fn prepare_workspace() -> Result<TempDir, String> {
    let root_source =
        fs::read_to_string(root().join("Cargo.toml")).map_err(|error| error.to_string())?;
    let root_manifest: Value = toml::from_str(&root_source).map_err(|error| error.to_string())?;
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    fs::create_dir_all(directory.path().join("src")).map_err(|error| error.to_string())?;
    fs::write(
        directory.path().join("Cargo.toml"),
        manifest(&root_manifest)?,
    )
    .map_err(|error| error.to_string())?;
    for (module, source) in [
        ("schema_formats", include_str!("../schema_formats/mod.rs")),
        ("schema_meter", include_str!("../schema_meter/mod.rs")),
        ("schema_replay", include_str!("../schema_replay/mod.rs")),
        (
            "schema_allocation",
            include_str!("../support/schema_allocation.rs"),
        ),
        (
            "schema_default_allocation",
            include_str!("../support/schema_default_allocation.rs"),
        ),
        (
            "schema_explicit_allocation",
            include_str!("../support/schema_explicit_allocation.rs"),
        ),
    ] {
        fs::write(
            directory.path().join("src").join(format!("{module}.rs")),
            source,
        )
        .map_err(|error| error.to_string())?;
    }
    fs::write(
        directory.path().join("build.rs"),
        include_str!("../support/schema_probe_build.rs"),
    )
    .map_err(|error| error.to_string())?;
    Ok(directory)
}

fn linked_catalog_for(mode: SourceMode, production: bool) -> Result<Catalog, String> {
    let target_dir = match mode {
        SourceMode::Default => root().join("target/schema-compiler-probe"),
        SourceMode::Explicit => root().join("target/schema-compiler-probe/explicit"),
    };
    let lease = acquire_lease(&target_dir)?;
    let inputs = if production {
        None
    } else {
        Some(fixture::Fixture::new().map_err(|error| error.to_string())?)
    };
    let snapshot = if let Some(fixture) = &inputs {
        fixture::inventory(&fixture.bidders, &fixture.source)
    } else {
        inventory(
            &root().join("crates/auctioneer-bidder/src/bidders"),
            &root().join(
                "crates/auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-inventory.toml",
            ),
        )
    }
    .map_err(|error| format!("inventory: {error:?}"))?;
    let (directory, membership) = prepare_probe_workspace(&snapshot, mode)?;
    verify_probe_workspace(directory.path())?;
    let mut reports = Vec::new();
    for target in TARGETS {
        let mut command = cargo(directory.path());
        command
            .args([
                "build",
                "--locked",
                "--offline",
                "-j",
                "2",
                "--bin",
                PACKAGE,
                "--target-dir",
            ])
            .arg(&target_dir);
        let relative = if target == "native" {
            command.arg("--release");
            PathBuf::from("release").join(PACKAGE)
        } else {
            command.args(["--profile", "wasm-release", "--target", target]);
            PathBuf::from(target)
                .join("wasm-release")
                .join(format!("{PACKAGE}.wasm"))
        };
        successful(&mut command, BUILD_TIMEOUT).map_err(|error| format!("{target}: {error}"))?;
        reports.push(
            artifact(&target_dir.join(relative), &membership, target)
                .map_err(|error| format!("{target}: {error}"))?,
        );
    }
    let native = target_dir.join("release").join(PACKAGE);
    let output = successful(
        Command::new(&native).arg("inventory"),
        Duration::from_secs(10),
    )?;
    let parsed: NativeInventory =
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    let expected: Vec<_> = snapshot
        .schemas
        .iter()
        .map(|row| row.witness.name.clone())
        .collect();
    if parsed.names != expected
        || parsed.results.len() != expected.len()
        || parsed.membership != membership
    {
        return Err("native dispatch membership mismatch".into());
    }
    if let Some(report) = reports.first_mut() {
        report.names = parsed.names;
        report.results = parsed.results;
    }
    Ok(Catalog {
        reports,
        directory,
        native,
        target_dir,
        _lease: lease,
        _inputs: inputs,
    })
}

fn verify_wasm(bytes: &[u8], target: &str) -> Result<schema_artifact::WasmArtifact, String> {
    use schema_artifact::ArtifactFormat;

    let expected = match target {
        "wasm32-unknown-unknown" | "wasm32-wasip1" => ArtifactFormat::Module,
        "wasm32-wasip2" => ArtifactFormat::Component,
        _ => return Err("unknown WASM target".into()),
    };
    let measured = schema_artifact::measure(bytes)?;
    if measured.format != expected {
        return Err("unexpected WASM artifact format".into());
    }
    let mut exports = BTreeSet::new();
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        if let wasmparser::Payload::ExportSection(section) =
            payload.map_err(|error| error.to_string())?
        {
            for raw in section {
                let entry = raw.map_err(|error| error.to_string())?;
                if entry.kind == wasmparser::ExternalKind::Func {
                    exports.insert(entry.name);
                }
            }
        }
    }
    if target == "wasm32-unknown-unknown" && !exports.contains("auctioneer_schema_validate") {
        return Err("missing exported validator entry".into());
    }
    if target == "wasm32-unknown-unknown" && !exports.contains("auctioneer_schema_membership") {
        return Err("missing exported membership entry".into());
    }
    if target == "wasm32-unknown-unknown" && !exports.contains("auctioneer_schema_initialize") {
        return Err("missing exported initializer entry".into());
    }
    Ok(measured)
}

fn verify_export_refusals(bytes: &[u8]) -> Result<(), String> {
    for (name, reason) in [
        (
            "auctioneer_schema_validate",
            "missing exported validator entry",
        ),
        (
            "auctioneer_schema_membership",
            "missing exported membership entry",
        ),
        (
            "auctioneer_schema_initialize",
            "missing exported initializer entry",
        ),
    ] {
        let mut changed = bytes.to_vec();
        let positions: Vec<_> = bytes
            .windows(name.len())
            .enumerate()
            .filter_map(|(index, window)| (window == name.as_bytes()).then_some(index))
            .collect();
        if positions.is_empty() {
            return Err("missing export mutation control".into());
        }
        for position in positions {
            let last = position
                .checked_add(name.len())
                .and_then(|end| end.checked_sub(1))
                .ok_or("export offset overflow")?;
            *changed
                .get_mut(last)
                .ok_or("export offset outside artifact")? = b'_';
        }
        if verify_wasm(&changed, "wasm32-unknown-unknown") != Err(reason.into()) {
            return Err(format!("independent export refusal missing: {name}"));
        }
    }
    Ok(())
}

impl Catalog {
    pub fn reports(&self) -> &[LinkReport] {
        &self.reports
    }

    pub fn initialize_native(&self) -> Result<Json, String> {
        let output = successful(
            Command::new(&self.native).arg("initialize"),
            Duration::from_secs(10),
        )?;
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())
    }

    pub fn native_allocations(&self, workload: &str) -> Result<Json, String> {
        let output = self.native_allocation_output(workload)?;
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())
    }

    pub fn native_cache_layouts(&self) -> Result<CacheLayouts, String> {
        let output = successful(
            Command::new(&self.native).arg("cache-layouts"),
            Duration::from_secs(10),
        )?;
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())
    }

    pub fn native_allocation_output(&self, workload: &str) -> Result<Output, String> {
        successful(
            Command::new(&self.native)
                .current_dir(self.directory.path())
                .args(["allocations", workload]),
            Duration::from_secs(10),
        )
    }

    pub fn check_native(&self, name: &str, input: &Json) -> Result<bool, String> {
        self.check_native_operation("check", name, input)
    }

    pub fn check_native_formats_disabled(&self, name: &str, input: &Json) -> Result<bool, String> {
        self.check_native_operation("check-formats-disabled", name, input)
    }

    pub fn check_native_draft4(&self, name: &str, input: &Json) -> Result<bool, String> {
        self.check_native_operation("check-draft4", name, input)
    }

    fn check_native_operation(
        &self,
        operation: &str,
        name: &str,
        input: &Json,
    ) -> Result<bool, String> {
        let output = successful(
            Command::new(&self.native)
                .current_dir(self.directory.path())
                .args([operation, name, &input.to_string()]),
            Duration::from_secs(10),
        )?;
        let parsed: NativeVerdict =
            serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
        Ok(parsed.valid)
    }

    pub fn check_native_sequence(&self, name: &str, inputs: &[Json]) -> Result<Vec<bool>, String> {
        let input = serde_json::to_string(inputs).map_err(|error| error.to_string())?;
        if input.len() > 0x0001_0000 {
            return Err("sequence input limit".into());
        }
        let output = successful(
            Command::new(&self.native)
                .current_dir(self.directory.path())
                .args(["sequence", name, &input]),
            Duration::from_secs(10),
        )?;
        let results: Vec<bool> =
            serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
        if results.len() != inputs.len() {
            return Err("sequence result count mismatch".into());
        }
        Ok(results)
    }

    pub fn check_native_raw(&self, name: &str, input: &str) -> Result<RawVerdict, String> {
        let output = successful(
            Command::new(&self.native)
                .current_dir(self.directory.path())
                .args(["check-raw", name, input]),
            Duration::from_secs(10),
        )?;
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())
    }

    fn reference_probe(&self, schema: &Json) -> Result<Output, String> {
        let path = self.directory.path().join("reference.json");
        fs::write(
            &path,
            serde_json::to_vec(schema).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let path_literal = literal(path.to_str().ok_or("non UTF-8 reference probe")?);
        let declaration = quote! {
            #[jsonschema::validator(
                path = #path_literal, draft = Draft4,
                methods = { is_valid = true, validate = false, iter_errors = false }
            )]
            struct Reference;
            fn main() { let _ = Reference::is_valid(&serde_json::Value::Null); }
        }
        .to_string();
        self.check_generated_source(&declaration)
    }

    pub fn check_generated_source(&self, generated: &str) -> Result<Output, String> {
        if generated.len() > MAX_SOURCE_BYTES {
            return Err("generated source limit".into());
        }
        fs::write(self.directory.path().join("src/generated.rs"), generated)
            .map_err(|error| error.to_string())?;
        let manifest_path = self.directory.path().join("Cargo.toml");
        let contents = fs::read_to_string(&manifest_path).map_err(|error| error.to_string())?;
        let mut document: Value = toml::from_str(&contents).map_err(|error| error.to_string())?;
        let mut entry = toml::Table::new();
        entry.insert(
            "name".into(),
            Value::String("schema-generated-probe".into()),
        );
        entry.insert("path".into(), Value::String("src/generated.rs".into()));
        document
            .as_table_mut()
            .ok_or("missing probe manifest")?
            .insert("bin".into(), Value::Array(vec![Value::Table(entry)]));
        fs::write(
            manifest_path,
            toml::to_string(&document).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        run(
            cargo(self.directory.path())
                .args([
                    "check",
                    "--color",
                    "never",
                    "--locked",
                    "--offline",
                    "-j",
                    "2",
                    "--bin",
                    "schema-generated-probe",
                    "--target-dir",
                ])
                .arg(&self.target_dir),
            BUILD_TIMEOUT,
        )
        .map_err(|error| format!("generated probe: {error:?}"))
    }

    pub fn check_explicit_refusals(&self) -> Result<(), String> {
        let positive =
            self.check_generated_source(&explicit_control_source(EXPLICIT_POSITIVE_BODY, "")?)?;
        if !positive.status.success() {
            return Err(format!(
                "explicit API positive control: {}",
                String::from_utf8_lossy(&positive.stderr)
            ));
        }
        for control in EXPLICIT_REFUSALS
            .into_iter()
            .chain(EXPLICIT_OPTION_REFUSALS)
        {
            let output = self.check_generated_source(&explicit_control_source(
                control.body,
                control.owner_items,
            )?)?;
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            if output.status.success()
                || control
                    .diagnostics
                    .iter()
                    .any(|expected| !diagnostic.contains(expected))
            {
                return Err(format!(
                    "missing explicit API refusal {}: {diagnostic}",
                    control.name
                ));
            }
        }
        Ok(())
    }

    pub fn check_reference_refusals(&self) -> Result<(), String> {
        use serde_json::json;
        use std::net::TcpListener;

        let control = self.reference_probe(&json!({"type": "object"}))?;
        if !control.status.success() {
            return Err(format!(
                "reference positive control: {}",
                String::from_utf8_lossy(&control.stderr)
            ));
        }
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
        listener
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;
        let address = listener.local_addr().map_err(|error| error.to_string())?;
        let external = self.directory.path().join("external.json");
        fs::write(
            &external,
            serde_json::to_vec(&json!({"type": "object"})).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let file_uri = url::Url::from_file_path(&external).map_err(|()| "file URI")?;
        for reference in [
            format!("http://{address}/external.json"),
            file_uri.to_string(),
        ] {
            let output = self.reference_probe(&json!({"$ref": reference}))?;
            if output.status.success()
                || !String::from_utf8_lossy(&output.stderr)
                    .contains("Default retriever does not fetch resources")
            {
                return Err(format!(
                    "missing offline-reference refusal: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }
        match listener.accept() {
            Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(()),
            _ => Err("macro attempted network retrieval".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RunError, SourceMode, fixture, run, source, source_for, verify_wasm};
    use crate::schema_vendor;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::Duration;
    use syn::parse::ParseStream;

    fn fake_cache_layouts() -> serde_json::Value {
        serde_json::json!({"schema_version": 2_u8, "word_bytes": 4_usize,
            "cache_bytes": 20_usize, "cache_alignment": 4_usize,
            "rows": [
                {"name": "fake_empty", "inline_bytes": 0_usize, "alignment": 1_usize},
                {"name": "fake_one", "inline_bytes": 20_usize, "alignment": 4_usize},
                {"name": "fake_three", "inline_bytes": 60_usize, "alignment": 4_usize}]})
    }

    const FAKE_CACHE_HELPERS: [(&str, usize); 3] =
        [("fake_empty", 0), ("fake_one", 1), ("fake_three", 3)];

    #[test]
    fn cache_layout_correspondence_checks_independent_word_and_helper_counts() {
        let layouts: super::CacheLayouts = serde_json::from_value(fake_cache_layouts()).unwrap();
        layouts.validate(4, &FAKE_CACHE_HELPERS).unwrap();
        let mut wide_document = fake_cache_layouts();
        wide_document["word_bytes"] = serde_json::json!(8_usize);
        let wide: super::CacheLayouts = serde_json::from_value(wide_document).unwrap();
        wide.validate(8, &FAKE_CACHE_HELPERS).unwrap();
        assert_eq!(
            wide.validate(4, &FAKE_CACHE_HELPERS).unwrap_err(),
            "cache layout word mismatch"
        );
        let mut helpers = FAKE_CACHE_HELPERS;
        helpers[2].1 = 2;
        assert_eq!(
            layouts.validate(4, &helpers).unwrap_err(),
            "cache wrapper layout mismatch"
        );
        helpers[2].1 = 4;
        assert_eq!(
            layouts.validate(4, &helpers).unwrap_err(),
            "cache wrapper layout mismatch"
        );
    }

    #[test]
    fn cache_layout_correspondence_refuses_each_unbound_or_malformed_fact() {
        use serde_json::json;
        let original = fake_cache_layouts();
        let mut cases = Vec::new();
        for (pointer, replacement, reason) in [
            (
                "/schema_version",
                json!(1_u8),
                "cache layout version mismatch",
            ),
            ("/word_bytes", json!(8_usize), "cache layout word mismatch"),
            ("/cache_bytes", json!(0_usize), "invalid cache base layout"),
            ("/cache_bytes", json!(21_usize), "invalid cache base layout"),
            (
                "/cache_alignment",
                json!(0_usize),
                "invalid cache base layout",
            ),
            (
                "/cache_alignment",
                json!(3_usize),
                "invalid cache base layout",
            ),
            (
                "/rows/1/name",
                json!("fake_wrong"),
                "cache wrapper identity mismatch",
            ),
            (
                "/rows/0/inline_bytes",
                json!(1_usize),
                "cache wrapper layout mismatch",
            ),
            (
                "/rows/0/alignment",
                json!(4_usize),
                "cache wrapper layout mismatch",
            ),
            (
                "/rows/1/inline_bytes",
                json!(19_usize),
                "cache wrapper layout mismatch",
            ),
            (
                "/rows/1/inline_bytes",
                json!(21_usize),
                "cache wrapper layout mismatch",
            ),
            (
                "/rows/1/alignment",
                json!(8_usize),
                "cache wrapper layout mismatch",
            ),
            (
                "/rows/2/inline_bytes",
                json!(20_usize),
                "cache wrapper layout mismatch",
            ),
        ] {
            let mut changed = original.clone();
            *changed.pointer_mut(pointer).unwrap() = replacement;
            cases.push((changed, reason));
        }
        let mut missing = original.clone();
        missing["rows"].as_array_mut().unwrap().pop();
        cases.push((missing, "cache wrapper count mismatch"));
        let mut extra = original.clone();
        extra["rows"]
            .as_array_mut()
            .unwrap()
            .push(original["rows"][0].clone());
        cases.push((extra, "cache wrapper count mismatch"));
        let mut reordered = original;
        reordered["rows"].as_array_mut().unwrap().reverse();
        cases.push((reordered, "cache wrapper identity mismatch"));
        for (changed, reason) in cases {
            let layouts: super::CacheLayouts = serde_json::from_value(changed).unwrap();
            assert_eq!(
                layouts.validate(4, &FAKE_CACHE_HELPERS).unwrap_err(),
                reason
            );
        }
    }

    #[test]
    fn cache_layout_correspondence_requires_a_nonempty_unique_finite_expected_set() {
        let layouts: super::CacheLayouts = serde_json::from_value(fake_cache_layouts()).unwrap();
        for expected in [
            vec![],
            vec![("", 0), ("fake_one", 1), ("fake_three", 3)],
            vec![("fake_empty", 0), ("fake_one", 1), ("fake_one", 3)],
        ] {
            assert_eq!(
                layouts.validate(4, &expected).unwrap_err(),
                "invalid cache wrapper membership"
            );
        }
        let mut expected = FAKE_CACHE_HELPERS;
        expected[2].1 = usize::MAX;
        assert_eq!(
            layouts.validate(4, &expected).unwrap_err(),
            "cache wrapper size overflow"
        );
        for word in [0, 2, 16] {
            assert_eq!(
                layouts.validate(word, &FAKE_CACHE_HELPERS).unwrap_err(),
                "cache layout word mismatch"
            );
        }
    }

    #[test]
    fn cache_layout_wire_shape_requires_all_fields_and_rejects_extras() {
        let original = fake_cache_layouts();
        for key in [
            "schema_version",
            "word_bytes",
            "cache_bytes",
            "cache_alignment",
            "rows",
        ] {
            let mut changed = original.clone();
            changed.as_object_mut().unwrap().remove(key).unwrap();
            serde_json::from_value::<super::CacheLayouts>(changed).unwrap_err();
        }
        for pointer in ["", "/rows/0"] {
            let mut changed = original.clone();
            changed
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("fake_unclassified".to_owned(), serde_json::json!(1_u8));
            serde_json::from_value::<super::CacheLayouts>(changed).unwrap_err();
        }
    }

    #[test]
    fn standalone_workspace_metadata_binds_both_reviewed_local_sources() {
        let temporary = tempfile::tempdir().unwrap();
        let root = super::root();
        let document =
            toml::from_str(&fs::read_to_string(root.join("Cargo.toml")).unwrap()).unwrap();
        fs::create_dir_all(temporary.path().join("src")).unwrap();
        fs::write(
            temporary.path().join("Cargo.toml"),
            super::manifest(&document).unwrap(),
        )
        .unwrap();
        fs::write(
            temporary.path().join("src/main.rs"),
            quote::quote! { fn main() {} }.to_string(),
        )
        .unwrap();
        super::verify_probe_workspace(temporary.path()).unwrap();
        let metadata = super::successful(
            super::cargo(temporary.path()).args([
                "metadata",
                "--offline",
                "--locked",
                "--format-version",
                "1",
            ]),
            Duration::from_secs(60),
        )
        .unwrap();
        let engine = super::schema_regex_vendor::verify(&root).unwrap();
        assert_eq!(
            super::resolved_engine_features(&engine).unwrap(),
            super::schema_regex_vendor::FEATURES
        );
        assert_eq!(
            super::schema_regex_vendor::verify_metadata(&metadata.stdout, &engine.canonical_root)
                .unwrap(),
            super::schema_regex_vendor::FEATURES
        );
        super::verify_compiler_metadata(&metadata.stdout, &schema_vendor::verify(&root).unwrap())
            .unwrap();
    }

    #[test]
    fn generated_modes_typecheck_both_layout_queries_without_constructing_owners() {
        for mode in [SourceMode::Default, SourceMode::Explicit] {
            let syntax: syn::File = syn::parse2(super::source_prelude(mode)).unwrap();
            let constants = syntax
                .items
                .iter()
                .filter_map(|node| {
                    let syn::Item::Const(constant) = node else {
                        return None;
                    };
                    (constant.ident == "_").then_some(constant)
                })
                .collect::<Vec<_>>();
            assert_eq!(constants.len(), 1);
            let item = constants[0];
            assert!(
                item.attrs.is_empty(),
                "both guest and native must typecheck the queries"
            );
            let syn::Type::BareFn(signature) = item.ty.as_ref() else {
                panic!("query witness must be a function pointer");
            };
            assert_eq!(signature.inputs.len(), 2);
            for (argument, expected) in signature.inputs.iter().zip([
                &["regex_automata", "meta", "Regex"][..],
                &["regex_automata", "util", "prefilter", "Prefilter"][..],
            ]) {
                let syn::Type::Reference(reference) = &argument.ty else {
                    panic!("witness must borrow the existing owner");
                };
                assert!(reference.mutability.is_none());
                let syn::Type::Path(path) = reference.elem.as_ref() else {
                    panic!("expected owner type path");
                };
                assert_eq!(
                    path.path
                        .segments
                        .iter()
                        .map(|part| part.ident.to_string())
                        .collect::<Vec<_>>(),
                    expected
                );
            }
            let syn::ReturnType::Type(_, return_type) = &signature.output else {
                panic!("missing layout result");
            };
            let syn::Type::Tuple(layouts) = return_type.as_ref() else {
                panic!("expected two layouts");
            };
            assert_eq!(layouts.elems.len(), 2);
            for layout in &layouts.elems {
                let syn::Type::Path(path) = layout else {
                    panic!("expected Layout path");
                };
                assert_eq!(
                    path.path
                        .segments
                        .iter()
                        .map(|part| part.ident.to_string())
                        .collect::<Vec<_>>(),
                    ["core", "alloc", "Layout"]
                );
            }
            let syn::Expr::Closure(closure) = item.expr.as_ref() else {
                panic!("expected compile-only closure");
            };
            assert_eq!(closure.inputs.len(), 2);
            let syn::Expr::Tuple(results) = closure.body.as_ref() else {
                panic!("only direct queries belong in the witness");
            };
            assert_eq!(results.elems.len(), 2);
            for (expression, owner, getter) in [
                (&results.elems[0], "regex", "strategy_payload"),
                (&results.elems[1], "prefilter", "matcher_payload"),
            ] {
                let syn::Expr::MethodCall(layout) = expression else {
                    panic!("expected payload getter");
                };
                assert_eq!(layout.method, getter);
                assert!(layout.args.is_empty());
                let syn::Expr::MethodCall(query) = layout.receiver.as_ref() else {
                    panic!("expected owning query");
                };
                assert_eq!(query.method, "resource_layout");
                assert!(query.args.is_empty());
                assert_path(&query.receiver, &[owner]);
            }
        }
    }

    fn assert_path(expression: &syn::Expr, expected: &[&str]) {
        let syn::Expr::Path(path) = expression else {
            panic!("expected a path");
        };
        assert_eq!(
            path.path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>(),
            expected
        );
    }

    fn assert_type_path(ty: &syn::Type, expected: &str) {
        let syn::Type::Path(path) = ty else {
            panic!("expected a type path");
        };
        assert!(path.qself.is_none());
        assert!(path.path.is_ident(expected));
    }

    #[test]
    fn explicit_type_controls_have_valid_siblings_and_private_nested_owners() {
        let positive = syn::parse_file(
            &super::explicit_control_source(super::EXPLICIT_POSITIVE_BODY, "").unwrap(),
        )
        .unwrap();
        assert_eq!(
            super::EXPLICIT_REFUSALS.map(|control| control.name),
            [
                "wrong-cache",
                "missing-context",
                "default",
                "clone",
                "immutable",
                "private-field",
                "name-collision"
            ]
        );
        let owners = positive
            .items
            .iter()
            .find_map(|item| {
                let syn::Item::Mod(module) = item else {
                    return None;
                };
                (module.ident == "owners").then_some(module)
            })
            .unwrap();
        let names: Vec<_> = owners
            .content
            .as_ref()
            .unwrap()
            .1
            .iter()
            .filter_map(|item| {
                let syn::Item::Struct(marker) = item else {
                    return None;
                };
                assert!(matches!(marker.vis, syn::Visibility::Public(_)));
                Some(marker.ident.to_string())
            })
            .collect();
        assert_eq!(names, ["First", "Second", "Empty", "Legacy"]);
        for control in super::EXPLICIT_REFUSALS {
            assert!(control.diagnostics[0].starts_with("error[E"));
            assert!(control.diagnostics.len() >= 3);
            let syntax = syn::parse_file(
                &super::explicit_control_source(control.body, control.owner_items).unwrap(),
            )
            .unwrap();
            assert!(matches!(&syntax.items[0], syn::Item::Mod(module) if module.ident == "owners"));
            assert!(matches!(&syntax.items[1], syn::Item::Fn(main) if main.sig.ident == "main"));
        }
    }

    #[test]
    fn explicit_control_generation_reports_the_invalid_syntax_owner() {
        for (body, owner_items, prefix) in [
            ("}", "", "explicit control body syntax:"),
            ("", "fn {", "explicit control owner syntax:"),
        ] {
            let error = super::explicit_control_source(body, owner_items).unwrap_err();
            assert!(error.starts_with(prefix), "{error}");
        }
    }

    #[test]
    fn explicit_option_controls_cover_each_incompatible_generated_mode() {
        assert_eq!(
            super::EXPLICIT_OPTION_REFUSALS.map(|control| control.name),
            [
                "incompatible-engine",
                "incompatible-backend",
                "enabled-validation-errors",
                "enabled-error-iterator"
            ]
        );
        for control in super::EXPLICIT_OPTION_REFUSALS {
            assert_eq!(control.diagnostics.len(), 1);
            assert!(control.diagnostics[0].starts_with("error: `cache = explicit` requires "));
            let source = super::explicit_control_source(control.body, control.owner_items).unwrap();
            let syntax = syn::parse_file(&source).unwrap();
            let syn::Item::Mod(owners) = &syntax.items[0] else {
                panic!("option control must retain its nested owner module");
            };
            assert!(
                matches!(owners.content.as_ref().unwrap().1.last(), Some(syn::Item::Struct(marker)) if marker.ident == "Incompatible")
            );
        }
    }

    fn callback_table<'a>(syntax: &'a syn::File, name: &str) -> &'a syn::ExprArray {
        let table = syntax
            .items
            .iter()
            .find_map(|item| {
                let syn::Item::Static(entry) = item else {
                    return None;
                };
                (entry.ident == name).then_some(entry)
            })
            .unwrap();
        let syn::Expr::Reference(borrow) = table.expr.as_ref() else {
            panic!("callback table must borrow its static array");
        };
        let syn::Expr::Array(array) = borrow.expr.as_ref() else {
            panic!("callback table must be an array");
        };
        array
    }

    fn functions_with_prefix<'a>(syntax: &'a syn::File, prefix: &str) -> Vec<&'a syn::ItemFn> {
        syntax
            .items
            .iter()
            .filter_map(|item| {
                let syn::Item::Fn(function) = item else {
                    return None;
                };
                function
                    .sig
                    .ident
                    .to_string()
                    .starts_with(prefix)
                    .then_some(function)
            })
            .collect()
    }

    fn assert_explicit_marker(
        declaration: &syn::ItemStruct,
        marker: &str,
        draft: &super::Draft,
        path: &Path,
    ) {
        assert_eq!(declaration.ident, marker);
        let syn::Meta::List(attribute) = &declaration.attrs[0].meta else {
            panic!("expected validator options");
        };
        let draft_name = match draft {
            super::Draft::Draft4 => "Draft4",
            super::Draft::Draft202012 => "Draft202012",
            super::Draft::Unspecified | super::Draft::Other(_) => panic!("unqualified draft"),
        };
        let draft_ident: syn::Ident = syn::parse_str(draft_name).unwrap();
        let path_literal = syn::LitStr::new(path.to_str().unwrap(), proc_macro2::Span::call_site());
        let options = quote::quote! {
            path = #path_literal, draft = #draft_ident, validate_formats = true, ignore_unknown_formats = false,
            formats = {
                "uuid" => crate::reference_uuid_format,
                "hostname" => crate::reference_hostname_format
            },
            pattern_options = {
                engine = regex, cache = explicit, size_limit = 65_536, dfa_size_limit = 65_536
            },
            methods = { is_valid = true, validate = false, iter_errors = false }
        };
        assert_eq!(attribute.tokens.to_string(), options.to_string());
    }

    fn assert_cache_variant(variant: &syn::Variant, marker: &str) {
        assert_eq!(variant.ident, marker);
        let syn::Fields::Unnamed(fields) = &variant.fields else {
            panic!("cache variant must contain its typed owner");
        };
        assert_eq!(fields.unnamed.len(), 1);
        assert_type_path(&fields.unnamed[0].ty, &format!("{marker}SearchCache"));
    }

    fn assert_cache_factory(factory: &syn::ItemFn, factory_name: &str, marker: &str) {
        assert_eq!(factory.sig.ident, factory_name);
        assert!(factory.sig.inputs.is_empty());
        let syn::ReturnType::Type(_, return_type) = &factory.sig.output else {
            panic!("factory must return Option<CacheOwner>");
        };
        let syn::Type::Path(return_path) = return_type.as_ref() else {
            panic!("expected Option type");
        };
        assert_eq!(return_path.path.segments.len(), 1);
        assert_eq!(return_path.path.segments[0].ident, "Option");
        let syn::PathArguments::AngleBracketed(arguments) = &return_path.path.segments[0].arguments
        else {
            panic!("expected Option type argument");
        };
        assert_eq!(arguments.args.len(), 1);
        let syn::GenericArgument::Type(ty) = &arguments.args[0] else {
            panic!("expected owner type");
        };
        assert_type_path(ty, "CacheOwner");
        let [syn::Stmt::Expr(syn::Expr::MethodCall(map), None)] = factory.block.stmts.as_slice()
        else {
            panic!("factory must wrap the actual constructor result");
        };
        assert_eq!(map.method, "map");
        assert_eq!(map.args.len(), 1);
        assert_path(&map.args[0], &["CacheOwner", marker]);
        let syn::Expr::Call(construct) = map.receiver.as_ref() else {
            panic!("factory must invoke the actual constructor");
        };
        assert!(construct.args.is_empty());
        assert_path(&construct.func, &[marker, "new_search_cache"]);
    }

    fn assert_checker_signature(check: &syn::ItemFn, check_name: &str) {
        assert_eq!(check.sig.ident, check_name);
        assert_eq!(check.sig.inputs.len(), 2);
        let syn::FnArg::Typed(input) = &check.sig.inputs[0] else {
            panic!("checker must borrow its input");
        };
        let syn::Type::Reference(input_borrow) = input.ty.as_ref() else {
            panic!("checker input must be borrowed");
        };
        assert!(input_borrow.mutability.is_none());
        let syn::Type::Path(input_type) = input_borrow.elem.as_ref() else {
            panic!("checker input must be a JSON value");
        };
        assert_eq!(input_type.path.segments.len(), 2);
        assert_eq!(input_type.path.segments[0].ident, "serde_json");
        assert_eq!(input_type.path.segments[1].ident, "Value");
        let syn::FnArg::Typed(owner_input) = &check.sig.inputs[1] else {
            panic!("checker must borrow the caller owner");
        };
        let syn::Type::Reference(owner_borrow) = owner_input.ty.as_ref() else {
            panic!("checker owner must be borrowed");
        };
        assert!(owner_borrow.mutability.is_some());
        assert_type_path(&owner_borrow.elem, "CacheOwner");
        let syn::ReturnType::Type(_, result) = &check.sig.output else {
            panic!("checker must return a boolean verdict");
        };
        assert_type_path(result, "bool");
    }

    fn assert_checker_dispatch(check: &syn::ItemFn, marker: &str) {
        let [syn::Stmt::Expr(syn::Expr::Match(owner_dispatch), None)] =
            check.block.stmts.as_slice()
        else {
            panic!("checker must dispatch on its caller's typed owner");
        };
        assert_path(&owner_dispatch.expr, &["owner"]);
        assert_eq!(owner_dispatch.arms.len(), 2);
        let syn::Pat::TupleStruct(pattern) = &owner_dispatch.arms[0].pat else {
            panic!("checker must match its own variant");
        };
        assert_eq!(pattern.path.segments.len(), 2);
        assert_eq!(pattern.path.segments[0].ident, "CacheOwner");
        assert_eq!(pattern.path.segments[1].ident, marker);
        assert_eq!(pattern.elems.len(), 1);
        assert!(matches!(&pattern.elems[0], syn::Pat::Ident(cache) if cache.ident == "cache"));
        let syn::Expr::Call(validate) = owner_dispatch.arms[0].body.as_ref() else {
            panic!("checker must call its own validator");
        };
        assert_path(&validate.func, &[marker, "is_valid"]);
        assert_eq!(validate.args.len(), 2);
        assert_path(&validate.args[0], &["input"]);
        assert_path(&validate.args[1], &["cache"]);
        assert!(matches!(owner_dispatch.arms[1].pat, syn::Pat::Wild(_)));
        assert!(
            matches!(owner_dispatch.arms[1].body.as_ref(), syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Bool(value), .. }) if !value.value)
        );
    }

    fn assert_validator_row(
        expression: &syn::Expr,
        name: &str,
        factory_name: &str,
        check_name: &str,
    ) {
        let syn::Expr::Tuple(tuple) = expression else {
            panic!("dispatch row must be a tuple");
        };
        assert_eq!(tuple.elems.len(), 3);
        assert!(
            matches!(&tuple.elems[0], syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(schema_name), .. }) if schema_name.value() == name)
        );
        assert_path(&tuple.elems[1], &[factory_name]);
        assert_path(&tuple.elems[2], &[check_name]);
    }

    fn assert_initializer_row(expression: &syn::Expr, name: &str, marker: &str) {
        let syn::Expr::Tuple(tuple) = expression else {
            panic!("initializer row must be a tuple");
        };
        assert_eq!(tuple.elems.len(), 2);
        assert!(
            matches!(&tuple.elems[0], syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(schema_name), .. }) if schema_name.value() == name)
        );
        assert_path(&tuple.elems[1], &[marker, "initialize_regexes"]);
    }

    #[test]
    fn allocation_helpers_are_native_only_and_mode_specific() {
        let inputs = fixture::Fixture::new().unwrap();
        let snapshot = fixture::inventory(&inputs.bidders, &inputs.source).unwrap();
        for (mode, expected) in [
            (SourceMode::Default, "schema_default_allocation"),
            (SourceMode::Explicit, "schema_explicit_allocation"),
        ] {
            let generated = source_for(&snapshot, mode).unwrap().0;
            let syntax = syn::parse_file(&generated).unwrap();
            let modules: Vec<_> = syntax
                .items
                .iter()
                .filter_map(|item| {
                    let syn::Item::Mod(module) = item else {
                        return None;
                    };
                    module
                        .ident
                        .to_string()
                        .contains("allocation")
                        .then_some(module)
                })
                .collect();
            assert_eq!(modules.len(), 2);
            for (module, name) in modules.iter().zip(["schema_allocation", expected]) {
                assert_eq!(module.ident, name);
                assert!(module.content.is_none());
                assert!(module.semi.is_some());
                assert!(
                    module
                        .attrs
                        .iter()
                        .all(|attribute| !attribute.path().is_ident("path"))
                );
                let guards: Vec<_> = module
                    .attrs
                    .iter()
                    .filter(|attribute| attribute.path().is_ident("cfg"))
                    .collect();
                assert_eq!(guards.len(), 1);
                let syn::Meta::List(negation) = guards[0].parse_args::<syn::Meta>().unwrap() else {
                    panic!("allocation helper must exclude all WASM targets");
                };
                assert!(negation.path.is_ident("not"));
                let syn::Meta::NameValue(target) = negation.parse_args::<syn::Meta>().unwrap()
                else {
                    panic!("expected target_arch guard");
                };
                assert!(target.path.is_ident("target_arch"));
                let syn::Expr::Lit(literal) = target.value else {
                    panic!("expected literal architecture")
                };
                let syn::Lit::Str(architecture) = literal.lit else {
                    panic!("expected string architecture")
                };
                assert_eq!(architecture.value(), "wasm32");
            }
        }
    }

    #[test]
    fn fake_selected_workload_loader_reads_only_selected_owner_files() {
        use serde_json::json;
        use std::collections::BTreeMap;

        let inputs = fixture::Fixture::new().unwrap();
        let snapshot = fixture::inventory(&inputs.bidders, &inputs.source).unwrap();
        let selected = snapshot
            .schemas
            .iter()
            .filter(|schema| ["fake_base", "fake_numeric"].contains(&schema.witness.name.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(selected.len(), 2);
        let authored = json!({"schema": "fake_base", "order": 7_u64, "rows": [
            {"id": "fake_structured", "input": {"fake_values": []}, "valid": false,
                "transforms": [{"op": "max_collection", "pointer": "/fake_values", "invalid": true}]},
            {"id": "fake_raw", "input_json": " \n{} \t", "valid": true, "transforms": []}
        ]});
        fs::write(
            snapshot.root.join("fake_base/tests/schema/workload.toml"),
            toml::to_string(&authored).unwrap(),
        )
        .unwrap();
        // An unselected owner's invalid file must never be read.
        fs::write(
            snapshot.root.join("fake_format/tests/schema/workload.toml"),
            "fake_invalid = [",
        )
        .unwrap();
        assert_eq!(
            super::workload_data_for(&snapshot.root, &selected).unwrap(),
            BTreeMap::from([("fake_base".to_owned(), authored)])
        );
    }

    #[test]
    fn fake_selected_workload_loader_allows_missing_files_and_empty_membership() {
        let inputs = fixture::Fixture::new().unwrap();
        let snapshot = fixture::inventory(&inputs.bidders, &inputs.source).unwrap();
        let selected = [snapshot
            .schemas
            .iter()
            .find(|schema| schema.witness.name == "fake_numeric")
            .unwrap()
            .clone()];
        assert!(
            super::workload_data_for(&snapshot.root, &selected)
                .unwrap()
                .is_empty()
        );
        assert!(
            super::workload_data_for(Path::new("/fake_unread_source"), &[])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn fake_selected_workload_loader_refuses_unselected_schema_in_selected_owner() {
        let inputs = fixture::Fixture::new().unwrap();
        let snapshot = fixture::inventory(&inputs.bidders, &inputs.source).unwrap();
        let selected = [snapshot
            .schemas
            .iter()
            .find(|schema| schema.witness.name == "fake_base")
            .unwrap()
            .clone()];
        let path = snapshot.root.join("fake_base/tests/schema/workload.toml");
        fs::write(&path, "schema = 'fake_alias'\nrows = []\n").unwrap();
        assert_eq!(
            super::workload_data_for(&snapshot.root, &selected),
            Err("workload ownership".to_owned())
        );
        fs::write(path, "fake_invalid = [").unwrap();
        super::workload_data_for(&snapshot.root, &selected).unwrap_err();
    }

    #[test]
    fn fake_selected_workload_loader_preserves_existing_snapshot_api() {
        let inputs = fixture::Fixture::new().unwrap();
        let snapshot = fixture::inventory(&inputs.bidders, &inputs.source).unwrap();
        let expected = super::workload_data(&snapshot).unwrap();
        assert_eq!(expected.len(), 2);
        assert!(expected.contains_key("fake_base"));
        assert!(expected.contains_key("fake_format"));
        assert_eq!(
            super::workload_data_for(&snapshot.root, &snapshot.schemas).unwrap(),
            expected
        );
    }

    #[test]
    fn workload_literals_preserve_order_names_and_json_payloads() {
        let inputs = fixture::Fixture::new().unwrap();
        let snapshot = fixture::inventory(&inputs.bidders, &inputs.source).unwrap();
        let mut expected = super::workload_data(&snapshot)
            .unwrap()
            .into_iter()
            .collect::<Vec<_>>();
        expected.sort_by_key(|(_, data)| data["order"].as_u64().unwrap());
        for mode in [SourceMode::Default, SourceMode::Explicit] {
            let syntax = syn::parse_file(&source_for(&snapshot, mode).unwrap().0).unwrap();
            let table = syntax
                .items
                .iter()
                .find_map(|item| {
                    let syn::Item::Const(table) = item else {
                        return None;
                    };
                    (table.ident == "WORKLOAD_DATA").then_some(table)
                })
                .unwrap();
            assert_eq!(table.attrs.len(), 1);
            let guard = &table.attrs[0];
            assert_eq!(
                quote::quote!(#guard).to_string(),
                quote::quote!(#[cfg(not(target_arch = "wasm32"))]).to_string()
            );
            let syn::Expr::Reference(borrow) = table.expr.as_ref() else {
                panic!("workload table must borrow its static array");
            };
            let syn::Expr::Array(rows) = borrow.expr.as_ref() else {
                panic!("workload table must be an array");
            };
            assert_eq!(rows.elems.len(), expected.len());
            for (row, (name, data)) in rows.elems.iter().zip(&expected) {
                let syn::Expr::Tuple(tuple) = row else {
                    panic!("workload row must be a tuple");
                };
                assert_eq!(tuple.elems.len(), 2);
                let json = data.to_string();
                for (element, expected_literal) in tuple.elems.iter().zip([name, &json]) {
                    let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(value),
                        ..
                    }) = element
                    else {
                        panic!("workload name and JSON payload must be string literals");
                    };
                    assert_eq!(value.value(), *expected_literal);
                }
            }
        }
    }

    #[test]
    fn explicit_dispatch_bijects_markers_typed_owners_factories_and_checks() {
        let inputs = fixture::Fixture::new().unwrap();
        let snapshot = fixture::inventory(&inputs.bidders, &inputs.source).unwrap();
        let (default, membership) = source(&snapshot).unwrap();
        assert_eq!(
            source_for(&snapshot, SourceMode::Default).unwrap().0,
            default
        );
        let (generated, explicit_membership) = source_for(&snapshot, SourceMode::Explicit).unwrap();
        assert_eq!(explicit_membership, membership);
        assert!(!generated.contains("schema_default_allocation"));
        assert!(generated.contains("schema_explicit_allocation"));
        let syntax = syn::parse_file(&generated).unwrap();
        let owner = syntax
            .items
            .iter()
            .find_map(|item| {
                let syn::Item::Enum(declaration) = item else {
                    return None;
                };
                (declaration.ident == "CacheOwner").then_some(declaration)
            })
            .unwrap();
        assert_eq!(owner.variants.len(), 7);
        assert!(
            owner.attrs.is_empty(),
            "owner must not derive Clone or Default"
        );
        let markers: Vec<_> = syntax
            .items
            .iter()
            .filter_map(|item| {
                let syn::Item::Struct(marker) = item else {
                    return None;
                };
                Some(marker)
            })
            .collect();
        let factories = functions_with_prefix(&syntax, "new_cache_");
        let checks = functions_with_prefix(&syntax, "check_schema_");
        assert_eq!(markers.len(), 7);
        assert_eq!(factories.len(), 7);
        assert_eq!(checks.len(), 7);
        let dispatch = callback_table(&syntax, "VALIDATORS");
        assert_eq!(dispatch.elems.len(), 7);
        let initializers = callback_table(&syntax, "INITIALIZERS");
        assert_eq!(initializers.elems.len(), 7);
        for (index, row) in snapshot.schemas.iter().enumerate() {
            let marker = format!("Schema{index}");
            let factory_name = format!("new_cache_{index}");
            let check_name = format!("check_schema_{index}");
            let path = snapshot
                .root
                .join(&row.witness.path)
                .canonicalize()
                .unwrap();
            assert_explicit_marker(markers[index], &marker, &row.audit.draft, &path);
            assert_cache_variant(&owner.variants[index], &marker);
            assert_cache_factory(factories[index], &factory_name, &marker);
            assert_checker_signature(checks[index], &check_name);
            assert_checker_dispatch(checks[index], &marker);
            assert_validator_row(
                &dispatch.elems[index],
                &row.witness.name,
                &factory_name,
                &check_name,
            );
            assert_initializer_row(&initializers.elems[index], &row.witness.name, &marker);
        }
    }

    fn explicit_sequence_arm(syntax: &syn::File) -> &syn::Arm {
        let main = syntax
            .items
            .iter()
            .find_map(|item| {
                let syn::Item::Fn(function) = item else {
                    return None;
                };
                (function.sig.ident == "main" && !function.block.stmts.is_empty())
                    .then_some(function)
            })
            .unwrap();
        let dispatch = main
            .block
            .stmts
            .iter()
            .find_map(|statement| {
                let syn::Stmt::Local(local) = statement else {
                    return None;
                };
                let syn::Expr::Match(expression) = local.init.as_ref()?.expr.as_ref() else {
                    return None;
                };
                Some(expression)
            })
            .unwrap();
        dispatch.arms.iter().find(|arm| {
            let Some((_, guard)) = &arm.guard else { return false; };
            let syn::Expr::Binary(and) = guard.as_ref() else { return false; };
            let syn::Expr::Binary(equal) = and.left.as_ref() else { return false; };
            matches!(equal.right.as_ref(), syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(operation), .. }) if operation.value() == "sequence")
        }).unwrap()
    }

    fn assert_sequence_byte_limit(sequence: &syn::Arm) {
        let syn::Expr::Binary(guard) = sequence.guard.as_ref().unwrap().1.as_ref() else {
            panic!("sequence must bound its input");
        };
        assert!(matches!(guard.op, syn::BinOp::And(_)));
        let syn::Expr::Binary(limit) = guard.right.as_ref() else {
            panic!("sequence must check its serialized byte length");
        };
        assert!(matches!(limit.op, syn::BinOp::Le(_)));
        let syn::Expr::MethodCall(length) = limit.left.as_ref() else {
            panic!("sequence must check input length");
        };
        assert_eq!(length.method, "len");
        assert_path(&length.receiver, &["input"]);
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(bytes),
            ..
        }) = limit.right.as_ref()
        else {
            panic!("sequence byte ceiling must be a literal");
        };
        assert_eq!(bytes.base10_parse::<usize>().unwrap(), 0x0001_0000);
    }

    fn assert_sequence_owner_binding(local: &syn::Local) {
        let syn::Expr::Try(construct) = local.init.as_ref().unwrap().expr.as_ref() else {
            panic!("construction failure must refuse the operation");
        };
        let syn::Expr::MethodCall(refusal) = construct.expr.as_ref() else {
            panic!("construction must handle None");
        };
        assert_eq!(refusal.method, "ok_or");
        let syn::Expr::Call(factory) = refusal.receiver.as_ref() else {
            panic!("owner must come from the factory");
        };
        assert_path(&factory.func, &["factory"]);
        assert!(factory.args.is_empty());
    }

    fn assert_sequence_result_binding(local: &syn::Local) {
        let syn::Expr::MethodCall(collect) = local.init.as_ref().unwrap().expr.as_ref() else {
            panic!("sequence must collect results");
        };
        assert_eq!(collect.method, "collect");
        let syn::Expr::MethodCall(map) = collect.receiver.as_ref() else {
            panic!("sequence must map inputs");
        };
        assert_eq!(map.method, "map");
        let syn::Expr::MethodCall(iter) = map.receiver.as_ref() else {
            panic!("sequence must iterate the input array");
        };
        assert_eq!(iter.method, "iter");
        assert_path(&iter.receiver, &["values"]);
        assert_eq!(map.args.len(), 1);
        let syn::Expr::Closure(callback) = &map.args[0] else {
            panic!("sequence must validate each input");
        };
        let syn::Expr::Call(check) = callback.body.as_ref() else {
            panic!("sequence callback must only validate");
        };
        assert_path(&check.func, &["validate"]);
        assert_eq!(check.args.len(), 2);
        let syn::Expr::Reference(borrow) = &check.args[1] else {
            panic!("sequence must reuse the mutable owner");
        };
        assert!(borrow.mutability.is_some());
        assert_path(&borrow.expr, &["owner"]);
    }

    #[test]
    fn explicit_sequence_constructs_one_owner_outside_the_validation_loop() {
        let syntax = syn::parse_file(include_str!("../support/schema_explicit_probe.rs")).unwrap();
        let sequence = explicit_sequence_arm(&syntax);
        let syn::Expr::Block(body) = sequence.body.as_ref() else {
            panic!("sequence must be a block");
        };
        assert_sequence_byte_limit(sequence);
        let bindings: Vec<_> = body
            .block
            .stmts
            .iter()
            .filter_map(|statement| {
                let syn::Stmt::Local(local) = statement else {
                    return None;
                };
                Some(local)
            })
            .collect();
        let owner_index = bindings.iter().position(|local| matches!(&local.pat, syn::Pat::Ident(name) if name.ident == "owner" && name.mutability.is_some())).unwrap();
        assert_sequence_owner_binding(bindings[owner_index]);
        let results_index = bindings.iter().position(|local| matches!(&local.pat, syn::Pat::Type(typed) if matches!(typed.pat.as_ref(), syn::Pat::Ident(name) if name.ident == "results"))).unwrap();
        assert!(owner_index < results_index);
        assert_sequence_result_binding(bindings[results_index]);
    }

    #[test]
    fn oversized_probe_inputs_refuse_before_launch_or_workspace_mutation() {
        let directory = tempfile::tempdir().unwrap();
        let catalog = super::Catalog {
            reports: Vec::new(),
            native: directory.path().join("missing-native"),
            target_dir: directory.path().join("missing-target"),
            directory,
            _lease: tempfile::tempfile().unwrap(),
            _inputs: None,
        };
        assert_eq!(
            catalog.check_native_sequence(
                "fake_base",
                &[serde_json::Value::String("x".repeat(0x0001_0000))]
            ),
            Err("sequence input limit".into())
        );
        assert_eq!(
            catalog
                .check_generated_source(&"x".repeat(super::MAX_SOURCE_BYTES + 1))
                .err(),
            Some("generated source limit".into())
        );
        assert!(
            fs::read_dir(catalog.directory.path())
                .unwrap()
                .next()
                .is_none()
        );
    }

    #[test]
    fn each_source_schema_owns_its_marker_validation_and_initialization_callbacks() {
        let inputs = fixture::Fixture::new().unwrap();
        let snapshot = fixture::inventory(&inputs.bidders, &inputs.source).unwrap();
        let (generated, _) = source(&snapshot).unwrap();
        let syntax = syn::parse_file(&generated).unwrap();
        let initializers = syntax.items.iter().find_map(|item| {
            let syn::Item::Static(entry) = item else {
                return None;
            };
            (entry.ident == "INITIALIZERS").then_some(entry)
        });
        assert!(initializers.is_some(), "missing explicit constructor table");
        let markers: Vec<_> = syntax
            .items
            .iter()
            .filter_map(|item| {
                if let syn::Item::Struct(structure) = item {
                    Some(structure)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(markers.len(), 7);
        let dispatch = syntax
            .items
            .iter()
            .find_map(|item| {
                let syn::Item::Static(entry) = item else {
                    return None;
                };
                (entry.ident == "VALIDATORS").then_some(entry)
            })
            .unwrap();
        for (table, method) in [
            (dispatch, "is_valid"),
            (initializers.unwrap(), "initialize_regexes"),
        ] {
            let syn::Expr::Reference(reference_expr) = table.expr.as_ref() else {
                panic!("dispatch must borrow its static array");
            };
            let syn::Expr::Array(array) = reference_expr.expr.as_ref() else {
                panic!("dispatch must be an array");
            };
            assert_eq!(array.elems.len(), 7);
            for (index, (row, marker)) in snapshot.schemas.iter().zip(&markers).enumerate() {
                let expected_marker = format!("Schema{index}");
                assert_eq!(marker.ident, expected_marker);
                let attribute = marker.attrs.first().unwrap();
                let path = attribute
                    .parse_args_with(|input: ParseStream<'_>| {
                        let key: syn::Ident = input.parse()?;
                        if key != "path" {
                            return Err(input.error("first option must be the source path"));
                        }
                        let _equals: syn::Token![=] = input.parse()?;
                        let value: syn::LitStr = input.parse()?;
                        let _rest: proc_macro2::TokenStream = input.parse()?;
                        Ok(value.value())
                    })
                    .unwrap();
                assert_eq!(
                    PathBuf::from(path),
                    snapshot
                        .root
                        .join(&row.witness.path)
                        .canonicalize()
                        .unwrap()
                );
                let syn::Expr::Tuple(tuple) = &array.elems[index] else {
                    panic!("dispatch row must be a tuple");
                };
                assert_eq!(tuple.elems.len(), 2);
                let syn::Expr::Lit(name) = &tuple.elems[0] else {
                    panic!("dispatch name must be a literal");
                };
                let syn::Lit::Str(name_literal) = &name.lit else {
                    panic!("dispatch name must be a string");
                };
                assert_eq!(name_literal.value(), row.witness.name);
                let syn::Expr::Path(callback) = &tuple.elems[1] else {
                    panic!("dispatch callback must be a method path");
                };
                assert_eq!(callback.path.segments.len(), 2);
                assert_eq!(callback.path.segments[0].ident, expected_marker);
                assert_eq!(callback.path.segments[1].ident, method);
            }
        }
    }

    #[test]
    fn child_deadline_is_distinct_from_a_successful_exit() {
        let output = run(
            Command::new("/bin/echo").arg("bounded"),
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"bounded\n");
        assert!(matches!(
            run(
                Command::new("/bin/sleep").arg("10"),
                Duration::from_millis(50)
            ),
            Err(RunError::Deadline { stderr }) if stderr.is_empty()
        ));
    }

    #[test]
    fn a_child_deadline_retains_completed_phase_diagnostics() {
        let directory = tempfile::tempdir().unwrap();
        let started = directory.path().join("started");
        let mut command = Command::new("/bin/sh");
        command.args([
            "-c",
            "printf 'allocation phase: thread-spawned\\n' >&2; printf started > \"$1\"; exec /bin/sleep 10",
            "probe",
        ]).arg(&started);
        let failure = run(&mut command, Duration::from_secs(1)).err().unwrap();
        assert_eq!(fs::read(&started).unwrap(), b"started");
        let RunError::Deadline { stderr } = failure else {
            panic!("deadline refusal required: {failure:?}");
        };
        assert_eq!(stderr, "allocation phase: thread-spawned\n");
        assert_eq!(
            super::successful(&mut command, Duration::from_secs(1))
                .err()
                .unwrap(),
            "child refused: Deadline { stderr: \"allocation phase: thread-spawned\\n\" }"
        );
    }

    #[test]
    fn deadline_diagnostics_keep_a_bounded_utf8_tail() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("stderr");
        fs::write(&input, [255_u8; 8192]).unwrap();
        let failure = run(
            Command::new("/bin/sh")
                .args([
                    "-c",
                    "cat \"$1\" >&2; printf 'allocation phase: final\\n' >&2; exec /bin/sleep 10",
                    "probe",
                ])
                .arg(input),
            Duration::from_secs(1),
        )
        .err()
        .unwrap();
        let RunError::Deadline { stderr } = failure else {
            panic!("deadline refusal required: {failure:?}");
        };
        assert!(stderr.len() <= 4096);
        assert!(stderr.starts_with('\u{fffd}'));
        assert!(stderr.ends_with("allocation phase: final\n"));
    }

    #[test]
    fn excessive_child_diagnostics_are_refused() {
        assert!(matches!(
            run(&mut Command::new("/usr/bin/yes"), Duration::from_secs(5)),
            Err(RunError::OutputLimit)
        ));
        assert!(matches!(
            run(
                Command::new("/bin/sh").args(["-c", "exec /usr/bin/yes >&2"]),
                Duration::from_secs(5),
            ),
            Err(RunError::OutputLimit)
        ));
    }

    #[test]
    fn an_empty_valid_wasm_module_is_not_a_linked_validator_catalog() {
        let empty = [0_u8, 97, 115, 109, 1, 0, 0, 0];
        assert_eq!(
            verify_wasm(&empty, "wasm32-unknown-unknown"),
            Err("missing exported validator entry".into())
        );
    }

    #[test]
    fn guest_format_must_match_the_declared_compiler_target() {
        let module = [0_u8, 97, 115, 109, 1, 0, 0, 0];
        let component = [0_u8, 97, 115, 109, 13, 0, 1, 0];
        for (bytes, target) in [
            (module, "wasm32-wasip2"),
            (component, "wasm32-wasip1"),
            (component, "wasm32-unknown-unknown"),
        ] {
            assert_eq!(
                verify_wasm(&bytes, target),
                Err("unexpected WASM artifact format".into()),
                "{target}"
            );
        }
        for target in ["native", "unknown-target"] {
            assert_eq!(
                verify_wasm(&module, target),
                Err("unknown WASM target".into())
            );
        }
    }
}
