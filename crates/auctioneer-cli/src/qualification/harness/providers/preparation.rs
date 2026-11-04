use super::super::{HarnessError, Result, io, process};
use super::{OVERRIDE_FEATURE, Provider};
use auctioneer_cli::native_control::SOURCE_ROOT;
use auctioneer_config::codegen::{
    AccountRaw, HostDefaultRaw, render_account_config, render_host_config,
};
use auctioneer_config::types::{AccountConfig, HostConfig};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env::{temp_dir, var_os};
use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::io::Write as _;
#[cfg(test)]
use std::os::unix::fs::symlink;
use std::path::{Component, Path, PathBuf};
use std::str::from_utf8;
#[cfg(test)]
use tests::harness_digest;
use validator::Validate as _;

pub(super) const INPUT_CAP: usize = 64 << 10;
pub(super) const FILE_CAP: usize = 128 << 20;
pub(super) const RECEIPT_CAP: usize = 8 << 20;
const PRODUCTION_FILE_LIMIT: usize = 10_000;
const SOURCE_ROOTS: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "edgezero.toml",
    "fastly.toml",
    "crates",
    ".cargo",
];
const BIDDERS_ROOT: &str = "crates/auctioneer-bidder/src/bidders";
const PRODUCT_DEFAULTS: &str = "crates/auctioneer-config-data/data/product-defaults.toml";
const FIXTURE_REQUEST: &str = "docs/testing/fixtures/provider-handlers/banner-request.json";
const FIXTURE_ROLE: &str = "fake_provider";
const FIXTURE_ROOTS: &[&str] = &[
    "docs/testing/fixtures/provider-handlers",
    "crates/auctioneer-cli/src/qualification/harness",
    SOURCE_ROOT,
];

pub(super) type Inventory = Vec<(String, Option<String>)>;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrepareRequest {
    #[serde(default = "repository_root")]
    pub repo_root: PathBuf,
    pub workspace: PathBuf,
    pub provider: String,
    pub origin: String,
    pub approved_origin: String,
    pub build_features: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileReceipt {
    pub path: PathBuf,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FixtureBidder {
    pub manifest: PathBuf,
    pub public_key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Preparation {
    pub repo_root: PathBuf,
    pub workspace: PathBuf,
    pub provider: String,
    pub auctioneer_commit: String,
    pub source_sha256: String,
    pub source_files: Inventory,
    pub bidders_dir: PathBuf,
    pub target_dir: PathBuf,
    pub target: String,
    pub profile: String,
    pub harness_sha256: String,
    pub fixtures_sha256: String,
    pub reference_data_sha256: String,
    pub baked_config_sha256: String,
    pub fixture_metadata_sha256: String,
    pub build_features: Vec<String>,
    pub production_files: Vec<FileReceipt>,
    pub fixture_bidder: FixtureBidder,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, PathBuf>,
}

pub(super) fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub(super) fn provider(value: &str) -> Result<Provider> {
    Provider::parse(&serde_json::Value::String(value.to_owned()))
}

pub(super) fn check_features(selected: Provider, features: &[String]) -> Result<()> {
    if features.len() != 2
        || features
            .iter()
            .filter(|feature| *feature == selected.name())
            .count()
            != 1
        || features
            .iter()
            .filter(|feature| *feature == OVERRIDE_FEATURE)
            .count()
            != 1
    {
        return Err(HarnessError::new("build_feature_missing"));
    }
    Ok(())
}

pub(super) fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub fn validate_origin(origin: &str, approved_origin: &str) -> Result<()> {
    let refusal = HarnessError::new("invalid_fixture_origin");
    if origin != approved_origin || origin.len() > 253 {
        return Err(refusal);
    }
    let parsed = url::Url::parse(origin).map_err(|_error| refusal)?;
    let host = parsed.host_str().ok_or(refusal)?;
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.path() != "/"
        || parsed.origin().ascii_serialization() != origin
        || !matches!(parsed.host(), Some(url::Host::Domain(_)))
        || !host.contains('.')
        || host.ends_with('.')
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || !label
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                || !label
                    .as_bytes()
                    .last()
                    .is_some_and(u8::is_ascii_alphanumeric)
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
        || host
            .rsplit('.')
            .next()
            .is_some_and(|label| ["localhost", "local", "internal"].contains(&label))
    {
        return Err(refusal);
    }
    Ok(())
}

pub(super) fn canonical_directory(path: &Path, category: &'static str) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path).map_err(|_error| HarnessError::new(category))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(HarnessError::new(category));
    }
    fs::canonicalize(path).map_err(|_error| HarnessError::new(category))
}

pub(super) fn isolated_workspace(repo: &Path, workspace: &Path, empty: bool) -> Result<PathBuf> {
    let category = "workspace_not_isolated";
    let canonical = canonical_directory(workspace, category)?;
    let temporary = fs::canonicalize(temp_dir()).map_err(|_error| HarnessError::new(category))?;
    if canonical == temporary
        || !canonical.starts_with(&temporary)
        || canonical.starts_with(repo)
        || repo.starts_with(&canonical)
    {
        return Err(HarnessError::new(category));
    }
    if empty
        && fs::read_dir(&canonical)
            .map_err(|_error| HarnessError::new(category))?
            .next()
            .is_some()
    {
        return Err(HarnessError::new(category));
    }
    Ok(canonical)
}

pub(super) fn private_directory(path: &Path) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700)
    };
    builder
        .create(path)
        .map_err(|_error| HarnessError::new("output_io"))
}

pub(super) fn write_private(path: &Path, bytes: &[u8], cap: usize) -> Result<()> {
    if bytes.len() > cap {
        return Err(HarnessError::new("output_limit"));
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600)
    };
    let mut output = options
        .open(path)
        .map_err(|_error| HarnessError::new("output_io"))?;
    output
        .write_all(bytes)
        .and_then(|()| output.sync_all())
        .map_err(|_error| HarnessError::new("output_io"))
}

pub(super) fn strict_file(root: &Path, relative: &Path, cap: usize) -> Result<Vec<u8>> {
    check_chain(root, relative)?;
    let file = root.join(relative);
    let before = io::file_digest(&file, cap)?;
    let bytes = io::read_bytes(&file, cap)?;
    if io::sha256(&bytes) != before || io::file_digest(&file, cap)? != before {
        return Err(HarnessError::new("input_changed"));
    }
    Ok(bytes)
}

pub(super) fn check_chain(root: &Path, relative: &Path) -> Result<()> {
    let mut current = root.to_path_buf();
    if canonical_directory(root, "invalid_input")? != root {
        return Err(HarnessError::new("invalid_input"));
    }
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(HarnessError::new("invalid_input"));
        };
        current.push(name);
        let metadata =
            fs::symlink_metadata(&current).map_err(|_error| HarnessError::new("invalid_input"))?;
        if metadata.file_type().is_symlink() {
            return Err(HarnessError::new("invalid_input"));
        }
    }
    Ok(())
}

pub(super) fn parse_toml(bytes: &[u8]) -> Result<toml::Value> {
    if bytes.len() > INPUT_CAP {
        return Err(HarnessError::new("input_limit"));
    }
    let text = from_utf8(bytes).map_err(|_error| HarnessError::new("invalid_input"))?;
    let value =
        toml::from_str::<toml::Value>(text).map_err(|_error| HarnessError::new("invalid_input"))?;
    let mut nodes = 0_usize;
    toml_limits(&value, 0, &mut nodes)?;
    Ok(value)
}

fn toml_limits(value: &toml::Value, depth: usize, nodes: &mut usize) -> Result<()> {
    *nodes = nodes
        .checked_add(1)
        .ok_or(HarnessError::new("input_limit"))?;
    if depth > 32 || *nodes > 10_000 {
        return Err(HarnessError::new("input_limit"));
    }
    let next = depth
        .checked_add(1)
        .ok_or(HarnessError::new("input_limit"))?;
    match value {
        toml::Value::Table(table) => {
            for child in table.values() {
                toml_limits(child, next, nodes)?;
            }
        }
        toml::Value::Array(array) => {
            for child in array {
                toml_limits(child, next, nodes)?;
            }
        }
        toml::Value::String(_)
        | toml::Value::Integer(_)
        | toml::Value::Float(_)
        | toml::Value::Boolean(_)
        | toml::Value::Datetime(_) => {}
    }
    Ok(())
}

pub(super) fn encode_toml(value: &toml::Value) -> Result<Vec<u8>> {
    let encoded = toml::to_string(value).map_err(|_error| HarnessError::new("invalid_input"))?;
    if parse_toml(encoded.as_bytes())? != *value {
        return Err(HarnessError::new("invalid_input"));
    }
    Ok(encoded.into_bytes())
}

struct InventoryWalk {
    entries: Inventory,
    nodes: usize,
    bytes: u64,
    cargo_outputs: bool,
}

impl InventoryWalk {
    fn visit(&mut self, root: &Path, relative: &Path, depth: usize) -> Result<()> {
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or(HarnessError::new("input_limit"))?;
        if self.nodes > 10_000 || depth > 32 {
            return Err(HarnessError::new("input_limit"));
        }
        let file = root.join(relative);
        let metadata = match fs::symlink_metadata(&file) {
            Ok(metadata) => metadata,
            Err(error) if depth == 0 && error.kind() == ErrorKind::NotFound => {
                return Ok(());
            }
            Err(_error) => return Err(HarnessError::new("invalid_input")),
        };
        if metadata.file_type().is_symlink() {
            return Err(HarnessError::new("invalid_input"));
        }
        let name = relative
            .to_str()
            .ok_or(HarnessError::new("invalid_input"))?;
        if metadata.is_dir() {
            let mut names = Vec::new();
            for entry in fs::read_dir(&file).map_err(|_error| HarnessError::new("invalid_input"))? {
                if names.len() >= 10_000 {
                    return Err(HarnessError::new("input_limit"));
                }
                names.push(
                    entry
                        .map_err(|_error| HarnessError::new("invalid_input"))?
                        .file_name(),
                );
            }
            names.sort();
            let owns_target =
                if self.cargo_outputs && names.iter().any(|entry| entry == "Cargo.toml") {
                    let manifest =
                        parse_toml(&strict_file(root, &relative.join("Cargo.toml"), INPUT_CAP)?)?;
                    ["package", "workspace"]
                        .iter()
                        .any(|key| manifest.get(key).is_some_and(toml::Value::is_table))
                } else {
                    false
                };
            if !name.is_empty() {
                self.entries.push((format!("{name}/"), None));
            }
            let next = depth
                .checked_add(1)
                .ok_or(HarnessError::new("input_limit"))?;
            for entry in names {
                if owns_target && entry == "target" {
                    continue;
                }
                self.visit(root, &relative.join(entry), next)?;
            }
        } else {
            self.bytes = self
                .bytes
                .checked_add(metadata.len())
                .ok_or(HarnessError::new("input_limit"))?;
            if self.bytes > 512 << 20_u32 {
                return Err(HarnessError::new("input_limit"));
            }
            check_chain(root, relative)?;
            self.entries
                .push((name.to_owned(), Some(io::file_digest(&file, FILE_CAP)?)));
        }
        Ok(())
    }
}

pub(super) fn inventory(root: &Path, roots: &[&str], cargo_outputs: bool) -> Result<Inventory> {
    let canonical = canonical_directory(root, "invalid_input")?;
    let mut walk = InventoryWalk {
        entries: Vec::new(),
        nodes: 0,
        bytes: 0,
        cargo_outputs,
    };
    for relative in roots {
        walk.visit(&canonical, Path::new(relative), 0)?;
    }
    walk.entries.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(walk.entries)
}

pub fn source_inventory(root: &Path) -> Result<Inventory> {
    inventory(root, SOURCE_ROOTS, true)
}

fn reference_input(relative: &str) -> bool {
    let path = Path::new(relative);
    relative.contains("/data/")
        || (path.starts_with(BIDDERS_ROOT)
            && (path.extension().is_some_and(|extension| {
                ["toml", "json", "yaml", "yml"]
                    .iter()
                    .any(|value| extension == *value)
            }) || path.file_name().is_some_and(|file_name| {
                file_name
                    .to_str()
                    .is_some_and(|name| name.starts_with("LICENSE"))
            })))
}

fn bidder_file_cap(path: &Path) -> usize {
    if path.file_name().is_some_and(|name| name == "bidder.toml") {
        INPUT_CAP
    } else {
        FILE_CAP
    }
}

pub(super) fn tool_environment() -> BTreeMap<OsString, OsString> {
    BTreeMap::from([
        (
            OsString::from("PATH"),
            var_os("PATH").unwrap_or_else(|| OsString::from("/usr/bin:/bin")),
        ),
        (OsString::from("LC_ALL"), OsString::from("C")),
        (OsString::from("GIT_CONFIG_NOSYSTEM"), OsString::from("1")),
        (
            OsString::from("GIT_CONFIG_GLOBAL"),
            OsString::from("/dev/null"),
        ),
    ])
}

pub(super) async fn git(
    supervisor: &process::Supervisor,
    root: &Path,
    arguments: &[&str],
    cap: usize,
) -> Result<Vec<u8>> {
    let mut spec = process::ProcessSpec::new("/usr/bin/git");
    spec.args = vec![
        OsString::from("--no-replace-objects"),
        OsString::from("-C"),
        root.as_os_str().to_owned(),
    ];
    spec.args.extend(arguments.iter().map(OsString::from));
    spec.output_cap = cap;
    spec.env = tool_environment();
    let output = supervisor.run(spec).await?;
    if !output.summary.closed {
        return Err(HarnessError::new("subprocess_failed"));
    }
    Ok(output.stdout)
}

async fn revision(supervisor: &process::Supervisor, root: &Path) -> Result<String> {
    let bytes = git(supervisor, root, &["rev-parse", "HEAD"], INPUT_CAP).await?;
    let text = from_utf8(&bytes)
        .map_err(|_error| HarnessError::new("invalid_input"))?
        .trim();
    if !hex(text, 40) {
        return Err(HarnessError::new("invalid_input"));
    }
    Ok(text.to_owned())
}

async fn production_receipts(
    supervisor: &process::Supervisor,
    repo: &Path,
) -> Result<Vec<FileReceipt>> {
    let listing = git(
        supervisor,
        repo,
        &["ls-tree", "-r", "--name-only", "HEAD", "--", BIDDERS_ROOT],
        RECEIPT_CAP,
    )
    .await?;
    let text = from_utf8(&listing).map_err(|_error| HarnessError::new("invalid_input"))?;
    let mut receipts = Vec::new();
    for relative in text.lines() {
        if receipts.len() >= PRODUCTION_FILE_LIMIT || !Path::new(relative).starts_with(BIDDERS_ROOT)
        {
            return Err(HarnessError::new("invalid_input"));
        }
        let cap = bidder_file_cap(Path::new(relative));
        let bytes = strict_file(repo, Path::new(relative), cap)?;
        let committed = git(
            supervisor,
            repo,
            &["show", &format!("HEAD:{relative}")],
            cap,
        )
        .await?;
        if bytes != committed {
            return Err(HarnessError::new("production_metadata_changed"));
        }
        receipts.push(FileReceipt {
            path: repo.join(relative),
            sha256: io::sha256(&bytes),
        });
    }
    if receipts.is_empty() {
        return Err(HarnessError::new("invalid_input"));
    }
    Ok(receipts)
}

fn receipted_bytes(repo: &Path, row: &FileReceipt) -> Result<Vec<u8>> {
    let relative = row
        .path
        .strip_prefix(repo)
        .map_err(|_error| HarnessError::new("invalid_input"))?;
    if !relative.starts_with(BIDDERS_ROOT) {
        return Err(HarnessError::new("invalid_input"));
    }
    let bytes = strict_file(repo, relative, bidder_file_cap(relative))?;
    if io::sha256(&bytes) != row.sha256 {
        return Err(HarnessError::new("production_metadata_changed"));
    }
    Ok(bytes)
}

fn passthrough_witness(repo: &Path, row: &FileReceipt, owner: &str) -> Result<bool> {
    let before = receipted_bytes(repo, row)?;
    let fixture: serde_json::Value = io::read_json(&row.path, FILE_CAP)?;
    if receipted_bytes(repo, row)? != before {
        return Err(HarnessError::new("input_changed"));
    }
    if fixture
        .get("implementation")
        .and_then(serde_json::Value::as_str)
        != Some(owner)
    {
        return Err(HarnessError::new("invalid_input"));
    }
    let Some(case) = fixture
        .get("cases")
        .and_then(serde_json::Value::as_array)
        .and_then(|cases| cases.first())
    else {
        return Ok(false);
    };
    let Some(request) = case.get("request").filter(|value| value.is_object()) else {
        return Ok(false);
    };
    Ok(case
        .get("outbound")
        .and_then(serde_json::Value::as_array)
        .filter(|calls| calls.len() == 1)
        .and_then(|calls| calls.first())
        .and_then(|call| call.get("body"))
        == Some(request))
}

fn fixed_https_definition(definition: &toml::Value) -> bool {
    let Some(table) = definition.as_table() else {
        return false;
    };
    if table.keys().any(|key| {
        !matches!(
            key.as_str(),
            "endpoint" | "maintainer" | "capabilities" | "debugAllow" | "disabled"
        )
    }) || table.get("disabled").and_then(toml::Value::as_bool) == Some(true)
    {
        return false;
    }
    let Some(endpoint) = table.get("endpoint").and_then(toml::Value::as_str) else {
        return false;
    };
    if endpoint.contains(['{', '}']) {
        return false;
    }
    let Ok(url) = url::Url::parse(endpoint) else {
        return false;
    };
    let origin = url.origin().ascii_serialization();
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && validate_origin(&origin, &origin).is_ok()
        && definition
            .get("capabilities")
            .and_then(|value| value.get("site"))
            .and_then(|value| value.get("mediaTypes"))
            .and_then(toml::Value::as_array)
            .is_some_and(|media| media.iter().any(|kind| kind.as_str() == Some("banner")))
}

fn select_fixture_bidder(repo: &Path, receipts: &[FileReceipt]) -> Result<FixtureBidder> {
    let mut selected = None;
    for row in receipts.iter().filter(|row| {
        row.path
            .file_name()
            .is_some_and(|name| name == "bidder.toml")
    }) {
        let manifest = parse_toml(&receipted_bytes(repo, row)?)?;
        if manifest.get("factory").and_then(toml::Value::as_bool) != Some(true)
            || manifest.get("settings").is_some()
            || manifest.get("required_feature").is_some()
        {
            continue;
        }
        let Some(definitions) = manifest.get("definitions").and_then(toml::Value::as_table) else {
            continue;
        };
        if !definitions.values().any(fixed_https_definition) {
            continue;
        }
        let owner = manifest
            .get("key")
            .and_then(toml::Value::as_str)
            .ok_or(HarnessError::new("invalid_input"))?;
        let witness_path = row
            .path
            .parent()
            .ok_or(HarnessError::new("invalid_input"))?
            .join("fixtures/factory.json");
        // Metadata alone cannot establish that request bodies pass through unchanged.
        let Some(witness) = receipts.iter().find(|entry| entry.path == witness_path) else {
            continue;
        };
        if !passthrough_witness(repo, witness, owner)? {
            continue;
        }
        for (public_key, definition) in definitions {
            if !fixed_https_definition(definition) {
                continue;
            }
            if selected.is_some() {
                return Err(HarnessError::new("invalid_input"));
            }
            selected = Some(FixtureBidder {
                manifest: row
                    .path
                    .strip_prefix(repo.join(BIDDERS_ROOT))
                    .map_err(|_error| HarnessError::new("invalid_input"))?
                    .to_path_buf(),
                public_key: public_key.clone(),
            });
        }
    }
    selected.ok_or(HarnessError::new("invalid_input"))
}

fn fixture_request(selection: &FixtureBidder) -> Result<Vec<u8>> {
    let mut request: serde_json::Value =
        io::read_json(&repository_root().join(FIXTURE_REQUEST), INPUT_CAP)?;
    let impressions = request
        .get_mut("imp")
        .and_then(serde_json::Value::as_array_mut)
        .filter(|impressions| impressions.len() == 1)
        .ok_or(HarnessError::new("invalid_input"))?;
    let bidders = impressions
        .first_mut()
        .and_then(|imp| imp.pointer_mut("/ext/bidder"))
        .and_then(serde_json::Value::as_object_mut)
        .filter(|bidders| bidders.len() == 1)
        .ok_or(HarnessError::new("invalid_input"))?;
    let params = bidders
        .remove(FIXTURE_ROLE)
        .filter(serde_json::Value::is_object)
        .ok_or(HarnessError::new("invalid_input"))?;
    bidders.insert(selection.public_key.clone(), params);
    io::json_bytes(&request, INPUT_CAP)
}

fn build_args(selected: Provider) -> Vec<String> {
    [
        "build",
        "--offline",
        "--locked",
        "--no-default-features",
        "--message-format=json",
        "--target",
        selected.target(),
        "--profile",
        "release",
        "-p",
        &format!("auctioneer-adapter-{}", selected.name()),
        "-p",
        "auctioneer-config-data",
        "--features",
        OVERRIDE_FEATURE,
        "--features",
        &format!("auctioneer-adapter-{0}/{0}", selected.name()),
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn build_environment(workspace: &Path) -> BTreeMap<String, PathBuf> {
    BTreeMap::from([
        (
            "CARGO_TARGET_DIR".to_owned(),
            workspace.join("cargo-target"),
        ),
        (
            "AUCTIONEER__BIDDERS__DIR".to_owned(),
            workspace.join("bidders"),
        ),
    ])
}

fn fixture_digest() -> Result<String> {
    fixture_digest_at(&repository_root())
}

fn fixture_digest_at(root: &Path) -> Result<String> {
    io::canonical_digest(&(1_u32, inventory(root, FIXTURE_ROOTS, false)?))
}

#[expect(
    clippy::cfg_not_test,
    reason = "unit fixtures use an explicit executable through the same bounded hash helper"
)]
#[cfg(not(test))]
fn harness_digest() -> Result<String> {
    use std::env::current_exe;

    let root = canonical_directory(&repository_root(), "invalid_input")?;
    let executable = current_exe().map_err(|_error| HarnessError::new("invalid_input"))?;
    harness_digest_at(&root, &executable)
}

fn harness_digest_at(root: &Path, executable: &Path) -> Result<String> {
    let sources = inventory(
        root,
        &[
            "crates/auctioneer-cli/src/qualification/harness",
            SOURCE_ROOT,
        ],
        false,
    )?;
    let binary = io::hash_file(executable, FILE_CAP)?;
    io::canonical_digest(&(1_u32, sources, binary))
}

pub async fn prepare_build(
    input: &Path,
    supervisor: &process::Supervisor,
) -> Result<serde_json::Value> {
    let request: PrepareRequest = io::read_json(input, INPUT_CAP)?;
    let preparation = prepare(request, supervisor).await?;
    Ok(
        serde_json::json!({"schema_version":1_u8,"kind":"offline_handler_preparation",
        "source_sha256":preparation.source_sha256,"fixture_metadata_sha256":preparation.fixture_metadata_sha256,
        "build_features":preparation.build_features,"target":preparation.target,
        "harness_sha256":preparation.harness_sha256,"fixtures_sha256":preparation.fixtures_sha256,
        "reference_data_sha256":preparation.reference_data_sha256,"baked_config_sha256":preparation.baked_config_sha256,
        "deployed_handler_acceptance":"pending","resource_qualification":"not_evaluated","publisher_pilot_ready":false}),
    )
}

pub(super) async fn prepare(
    request: PrepareRequest,
    supervisor: &process::Supervisor,
) -> Result<Preparation> {
    supervisor.check_active().await?;
    let selected = provider(&request.provider)?;
    check_features(selected, &request.build_features)?;
    validate_origin(&request.origin, &request.approved_origin)?;
    let repo = canonical_directory(&request.repo_root, "invalid_input")?;
    let workspace = isolated_workspace(&repo, &request.workspace, true)?;
    let auctioneer_commit = revision(supervisor, &repo).await?;
    let production_files = production_receipts(supervisor, &repo).await?;
    let fixture_bidder = select_fixture_bidder(&repo, &production_files)?;
    let source_files = source_inventory(&repo)?;
    let baked_config_sha256 = product_defaults_digest(&repo)?;
    let bidders_dir = workspace.join("bidders");
    let target_dir = workspace.join("cargo-target");
    private_directory(&bidders_dir)?;
    private_directory(&target_dir)?;
    for row in &production_files {
        let relative = row
            .path
            .strip_prefix(&repo)
            .map_err(|_error| HarnessError::new("invalid_input"))?;
        let cap = bidder_file_cap(relative);
        let mut bytes = strict_file(&repo, relative, cap)?;
        if io::sha256(&bytes) != row.sha256 {
            return Err(HarnessError::new("production_metadata_changed"));
        }
        let member = relative
            .strip_prefix(BIDDERS_ROOT)
            .map_err(|_error| HarnessError::new("invalid_input"))?;
        if member == fixture_bidder.manifest {
            let mut manifest = parse_toml(&bytes)?;
            let table = manifest
                .get_mut("definitions")
                .and_then(|value| value.get_mut(fixture_bidder.public_key.as_str()))
                .and_then(toml::Value::as_table_mut)
                .ok_or(HarnessError::new("invalid_input"))?;
            table.insert(
                "endpoint".to_owned(),
                toml::Value::String(format!("{}/openrtb2/auction", request.origin)),
            );
            bytes = encode_toml(&manifest)?;
        }
        supervisor.check_active().await?;
        let destination = bidders_dir.join(member);
        fs::create_dir_all(
            destination
                .parent()
                .ok_or(HarnessError::new("invalid_input"))?,
        )
        .map_err(|_error| HarnessError::new("output_io"))?;
        write_private(&destination, &bytes, cap)?;
    }
    write_private(
        &workspace.join("banner-request.json"),
        &fixture_request(&fixture_bidder)?,
        INPUT_CAP,
    )?;
    let result = Preparation {
        repo_root: repo,
        workspace: workspace.clone(),
        provider: request.provider,
        auctioneer_commit,
        source_sha256: io::canonical_digest(&source_files)?,
        reference_data_sha256: io::canonical_digest(
            &source_files
                .iter()
                .filter(|row| reference_input(&row.0))
                .collect::<Vec<_>>(),
        )?,
        source_files,
        bidders_dir: bidders_dir.clone(),
        target_dir,
        target: selected.target().to_owned(),
        profile: "release".to_owned(),
        harness_sha256: harness_digest()?,
        fixtures_sha256: fixture_digest()?,
        baked_config_sha256,
        fixture_metadata_sha256: io::canonical_digest(&inventory(&bidders_dir, &[""], false)?)?,
        build_features: request.build_features,
        production_files,
        fixture_bidder,
        command: "cargo".to_owned(),
        args: build_args(selected),
        env: build_environment(&workspace),
    };
    verify_frozen_inputs(&result, supervisor).await?;
    supervisor.check_active().await?;
    io::write_json(&workspace.join("build-plan.json"), &result, RECEIPT_CAP)?;
    Ok(result)
}

fn product_defaults_digest(root: &Path) -> Result<String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ProductDefaults<Host, Account> {
        host: Host,
        account: Account,
    }

    let invalid = HarnessError::new("invalid_input");
    let document = parse_toml(&strict_file(root, Path::new(PRODUCT_DEFAULTS), INPUT_CAP)?)?;
    let raw: ProductDefaults<HostDefaultRaw, AccountRaw> =
        document.clone().try_into().map_err(|_error| invalid)?;
    raw.host
        .resource_limits
        .validate()
        .map_err(|_error| invalid)?;
    if raw.host.user_sync.is_some()
        || !raw.host.aliases.is_empty()
        || raw.host.gdpr.host_vendor_id != 0
        || !raw.host.gdpr.non_standard_publishers.is_empty()
        || raw.host.host_cookie.domain.is_some()
        || raw.host.host_cookie.optout_cookie.is_some()
        || !raw.host.status_response.is_empty()
    {
        return Err(invalid);
    }
    render_host_config(&raw.host)
        .map(drop)
        .map_err(|_error| invalid)?;
    render_account_config(&raw.account)
        .map(drop)
        .map_err(|_error| invalid)?;
    // JSON's typed map-key decoder handles the same numeric TCF keys as codegen.
    let normalized: ProductDefaults<HostConfig, AccountConfig> =
        serde_json::from_value(serde_json::to_value(document).map_err(|_error| invalid)?)
            .map_err(|_error| invalid)?;
    // Codegen emits Some(0), not None, even though the effective type is optional.
    if normalized.host.gdpr.host_vendor_id != Some(0) {
        return Err(invalid);
    }
    let bytes = serde_json::to_vec(&(normalized.host, normalized.account))
        .map_err(|_error| HarnessError::new("invalid_input"))?;
    Ok(io::sha256(&bytes))
}

pub(super) fn validate_preparation(preparation: &Preparation) -> Result<()> {
    let selected = provider(&preparation.provider)?;
    check_features(selected, &preparation.build_features)?;
    let repo = canonical_directory(&preparation.repo_root, "source_changed")?;
    if repo != preparation.repo_root
        || isolated_workspace(&repo, &preparation.workspace, false)? != preparation.workspace
        || preparation.bidders_dir != preparation.workspace.join("bidders")
        || preparation.target_dir != preparation.workspace.join("cargo-target")
        || preparation.target != selected.target()
        || preparation.profile != "release"
        || preparation.command != "cargo"
        || preparation.args != build_args(selected)
        || preparation.env != build_environment(&preparation.workspace)
        || !hex(&preparation.auctioneer_commit, 40)
        || preparation.source_files.len() > 10_000
        || preparation.production_files.is_empty()
        || preparation.production_files.len() > PRODUCTION_FILE_LIMIT
    {
        return Err(HarnessError::new("invalid_input"));
    }
    for value in [
        &preparation.source_sha256,
        &preparation.harness_sha256,
        &preparation.fixtures_sha256,
        &preparation.reference_data_sha256,
        &preparation.baked_config_sha256,
        &preparation.fixture_metadata_sha256,
    ] {
        if !hex(value, 64) {
            return Err(HarnessError::new("invalid_input"));
        }
    }
    if io::canonical_digest(&preparation.source_files)? != preparation.source_sha256 {
        return Err(HarnessError::new("source_changed"));
    }
    // Bind every frozen catalog file exactly once, including retained raw assets.
    let mut expected: BTreeMap<_, _> = preparation
        .source_files
        .iter()
        .filter(|(relative, _digest)| Path::new(relative).starts_with(BIDDERS_ROOT))
        .filter_map(|(relative, digest)| {
            digest
                .as_deref()
                .map(|sha256| (repo.join(relative), sha256))
        })
        .collect();
    if expected.len() != preparation.production_files.len() {
        return Err(HarnessError::new("production_metadata_changed"));
    }
    for row in &preparation.production_files {
        if expected.remove(&row.path) != Some(row.sha256.as_str()) {
            return Err(HarnessError::new("production_metadata_changed"));
        }
    }
    let fixture_bidder = select_fixture_bidder(&repo, &preparation.production_files)
        .map_err(|_error| HarnessError::new("production_metadata_changed"))?;
    if preparation.fixture_bidder != fixture_bidder {
        return Err(HarnessError::new("invalid_input"));
    }
    Ok(())
}

pub fn verify_production(preparation: &Preparation) -> Result<()> {
    for row in &preparation.production_files {
        let relative = row
            .path
            .strip_prefix(&preparation.repo_root)
            .map_err(|_error| HarnessError::new("production_metadata_changed"))?;
        if !relative.starts_with(BIDDERS_ROOT)
            || !hex(&row.sha256, 64)
            || io::sha256(
                &strict_file(&preparation.repo_root, relative, bidder_file_cap(relative))
                    .map_err(|_error| HarnessError::new("production_metadata_changed"))?,
            ) != row.sha256
        {
            return Err(HarnessError::new("production_metadata_changed"));
        }
    }
    Ok(())
}

pub async fn verify_frozen_inputs(
    preparation: &Preparation,
    supervisor: &process::Supervisor,
) -> Result<()> {
    supervisor.check_active().await?;
    validate_preparation(preparation)?;
    verify_production(preparation)?;
    let current_revision = revision(supervisor, &preparation.repo_root).await?;
    if current_revision != preparation.auctioneer_commit
        || io::canonical_digest(
            &source_inventory(&preparation.repo_root)
                .map_err(|_error| HarnessError::new("source_changed"))?,
        )? != preparation.source_sha256
    {
        return Err(HarnessError::new("source_changed"));
    }
    if product_defaults_digest(&preparation.repo_root)
        .map_err(|_error| HarnessError::new("source_changed"))?
        != preparation.baked_config_sha256
    {
        return Err(HarnessError::new("source_changed"));
    }
    let frozen = (|| -> Result<()> {
        check_chain(&preparation.workspace, Path::new("bidders"))?;
        check_chain(&preparation.workspace, Path::new("cargo-target"))?;
        if io::canonical_digest(&inventory(&preparation.bidders_dir, &[""], false)?)?
            != preparation.fixture_metadata_sha256
            || fixture_digest()? != preparation.fixtures_sha256
            || harness_digest()? != preparation.harness_sha256
            || strict_file(
                &preparation.workspace,
                Path::new("banner-request.json"),
                INPUT_CAP,
            )? != fixture_request(&preparation.fixture_bidder)?
        {
            return Err(HarnessError::new("fixture_changed"));
        }
        Ok(())
    })();
    frozen.map_err(|_error| HarnessError::new("fixture_changed"))?;
    supervisor.check_active().await
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::bidders::resources::build_report;

    const FIXTURE_MANIFEST: &str = "fake_passthrough/bidder.toml";

    pub(super) fn harness_digest() -> Result<String> {
        let root = canonical_directory(&repository_root(), "invalid_input")?;
        let fixture = harness_executable();
        harness_digest_at(&root, fixture.path())
    }

    pub(super) fn harness_executable() -> tempfile::NamedTempFile {
        // Provider unit fixtures hash an explicit binary, not libtest's debug
        // executable, whose unrelated test code may exceed the production cap.
        let fixture = tempfile::NamedTempFile::new().expect("synthetic harness executable");
        fs::write(fixture.path(), b"fake_provider_harness").expect("write synthetic harness");
        fixture
    }

    fn product_data_fixture(root: &Path) -> PathBuf {
        let path = root.join(PRODUCT_DEFAULTS);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::copy(repository_root().join(PRODUCT_DEFAULTS), &path).unwrap();
        path
    }

    fn product_fixture_root() -> tempfile::TempDir {
        let temporary = canonical_directory(&temp_dir(), "invalid_input").unwrap();
        tempfile::tempdir_in(temporary).unwrap()
    }

    #[test]
    fn selected_product_defaults_decode_through_the_existing_schema_stages() {
        let root = product_fixture_root();
        product_data_fixture(root.path());
        assert!(
            check_chain(root.path(), Path::new(PRODUCT_DEFAULTS)).is_ok(),
            "product_source_stage"
        );
        let bytes = strict_file(root.path(), Path::new(PRODUCT_DEFAULTS), INPUT_CAP)
            .unwrap_or_else(|_error| panic!("product_source_stage"));
        let document = parse_toml(&bytes).unwrap_or_else(|_error| panic!("product_toml_stage"));
        let host: HostDefaultRaw = document["host"]
            .clone()
            .try_into()
            .unwrap_or_else(|_error| panic!("product_raw_host_stage"));
        let account: AccountRaw = document["account"]
            .clone()
            .try_into()
            .unwrap_or_else(|_error| panic!("product_raw_account_stage"));
        assert!(
            host.resource_limits.validate().is_ok(),
            "product_resource_stage"
        );
        assert!(
            render_host_config(&host).is_ok(),
            "product_host_renderer_stage"
        );
        assert!(
            render_account_config(&account).is_ok(),
            "product_account_renderer_stage"
        );
        let json =
            serde_json::to_value(document).unwrap_or_else(|_error| panic!("product_json_stage"));
        assert!(
            serde_json::from_value::<HostConfig>(json["host"].clone()).is_ok(),
            "product_typed_host_stage"
        );
        assert!(
            serde_json::from_value::<AccountConfig>(json["account"].clone()).is_ok(),
            "product_typed_account_stage"
        );
        assert!(
            product_defaults_digest(root.path()).is_ok(),
            "product_tuple_digest_stage"
        );
    }

    #[test]
    fn qualification_identity_uses_neutral_product_defaults_not_authored_policy() {
        let host = auctioneer_config_data::product_host_default();
        assert!(host.user_sync.is_none());
        assert!(host.aliases.is_empty());
        assert_eq!(host.gdpr.host_vendor_id, Some(0));
        assert!(host.gdpr.non_standard_publishers.is_empty());
        let root = product_fixture_root();
        product_data_fixture(root.path());
        let baseline = product_defaults_digest(root.path()).unwrap();
        fs::write(root.path().join("auctioneer.toml"), b"not TOML [").unwrap();
        assert_eq!(product_defaults_digest(root.path()).unwrap(), baseline);
        fs::remove_file(root.path().join("auctioneer.toml")).unwrap();
        assert_eq!(product_defaults_digest(root.path()).unwrap(), baseline);
        let report = build_report().unwrap();
        for row in &report.adapters {
            assert_eq!(row.baked_config_sha256, baseline);
        }
    }

    #[test]
    fn selected_product_defaults_match_the_compiled_tuple_and_track_each_selected_layer() {
        let compiled_host = auctioneer_config_data::product_host_default();
        let compiled_account = auctioneer_config_data::product_account_default();
        let compiled = io::sha256(&serde_json::to_vec(&(compiled_host, compiled_account)).unwrap());
        for layer in ["host", "account"] {
            let root = product_fixture_root();
            let path = product_data_fixture(root.path());
            let mut document = parse_toml(&fs::read(&path).unwrap()).unwrap();
            assert_eq!(product_defaults_digest(root.path()).unwrap(), compiled);
            let mut expected_host = compiled_host.clone();
            let mut expected_account = compiled_account.clone();
            if layer == "host" {
                document["host"]["resource_limits"]["default_tmax_ms"] = toml::Value::Integer(1234);
                expected_host.resource_limits.default_tmax_ms = 1234;
            } else {
                document["account"]["targeting_prefix"] = toml::Value::String("ad".to_owned());
                expected_account.targeting_prefix = "ad".to_owned();
            }
            fs::write(path, toml::to_string(&document).unwrap()).unwrap();
            let selected = product_defaults_digest(root.path()).unwrap();
            assert_ne!(selected, compiled);
            assert_eq!(
                selected,
                io::sha256(&serde_json::to_vec(&(expected_host, expected_account)).unwrap())
            );
        }
    }

    #[test]
    fn selected_product_defaults_use_typed_numeric_tcf_keys_and_renderer_semantics() {
        use auctioneer_config::types::{EnforceAlgo, PurposeEnforcement, TriState};
        let root = product_fixture_root();
        let path = product_data_fixture(root.path());
        let mut document =
            serde_json::to_value(parse_toml(&fs::read(&path).unwrap()).unwrap()).unwrap();
        let entry = serde_json::json!({
            "enforce_algo": "full", "enforce_purpose": "enabled",
            "enforce_vendors": "disabled", "vendor_exceptions": []
        });
        document["host"]["gdpr"]["tcf2"]["purposes"]["2"] = entry.clone();
        document["account"]["gdpr"]["purposes"]["3"] = entry;
        fs::write(&path, toml::to_string(&document).unwrap()).unwrap();
        let mut host = auctioneer_config_data::product_host_default().clone();
        let mut account = auctioneer_config_data::product_account_default().clone();
        let expected = PurposeEnforcement {
            enforce_algo: EnforceAlgo::Full,
            enforce_purpose: TriState::Enabled,
            enforce_vendors: TriState::Disabled,
            vendor_exceptions: Vec::new(),
        };
        host.gdpr.tcf2.purposes.insert(2, expected.clone());
        account.gdpr.purposes.insert(3, expected);
        assert_eq!(
            product_defaults_digest(root.path()).unwrap(),
            io::sha256(&serde_json::to_vec(&(host, account)).unwrap())
        );
        document["host"]["gdpr"]["tcf2"]["purposes"]["11"] =
            document["host"]["gdpr"]["tcf2"]["purposes"]["2"].clone();
        fs::write(&path, toml::to_string(&document).unwrap()).unwrap();
        assert_eq!(
            product_defaults_digest(root.path()).unwrap_err().code,
            "invalid_input"
        );
    }

    #[test]
    fn selected_product_defaults_fail_closed_on_invalid_shape_values_and_source_bounds() {
        let root = product_fixture_root();
        let path = product_data_fixture(root.path());
        let original = fs::read(&path).unwrap();
        let baseline = serde_json::to_value(parse_toml(&original).unwrap()).unwrap();
        product_defaults_digest(root.path()).unwrap();
        for (pointer, value) in [
            (
                "/host/resource_limits/max_tmax_ms",
                serde_json::json!(0_i64),
            ),
            (
                "/account/privacy/ipv4/anon_keep_bits",
                serde_json::json!(33_u8),
            ),
            ("/account/gdpr/enabled", serde_json::json!("invalid")),
            ("/host/status_response", serde_json::json!("fake_operator")),
        ] {
            let mut document = baseline.clone();
            *document.pointer_mut(pointer).unwrap() = value;
            fs::write(&path, toml::to_string(&document).unwrap()).unwrap();
            assert_eq!(
                product_defaults_digest(root.path()).unwrap_err().code,
                "invalid_input",
                "{pointer}"
            );
        }
        for pointer in ["", "/host", "/account"] {
            let mut document = baseline.clone();
            document
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("unexpected".to_owned(), serde_json::json!(true));
            fs::write(&path, toml::to_string(&document).unwrap()).unwrap();
            assert_eq!(
                product_defaults_digest(root.path()).unwrap_err().code,
                "invalid_input",
                "{pointer}"
            );
        }
        let mut incomplete = baseline;
        assert!(
            incomplete["host"]["gdpr"]
                .as_object_mut()
                .unwrap()
                .remove("host_vendor_id")
                .is_some()
        );
        fs::write(&path, toml::to_string(&incomplete).unwrap()).unwrap();
        assert_eq!(
            product_defaults_digest(root.path()).unwrap_err().code,
            "invalid_input"
        );
        let mut oversized = original;
        oversized.resize(INPUT_CAP.checked_add(1).unwrap(), b' ');
        fs::write(&path, oversized).unwrap();
        assert_eq!(
            product_defaults_digest(root.path()).unwrap_err().code,
            "input_limit"
        );
        fs::remove_file(&path).unwrap();
        product_defaults_digest(root.path()).unwrap_err();
    }

    fn write_digest_source(root: &Path, relative: &str, bytes: &[u8]) -> PathBuf {
        let file = root.join(relative);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, bytes).unwrap();
        file
    }

    fn digest_tree() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        write_digest_source(
            root.path(),
            "crates/auctioneer-cli/src/qualification/harness/mod.rs",
            b"// existing harness\n",
        );
        write_digest_source(
            root.path(),
            "docs/testing/fixtures/provider-handlers/request.txt",
            b"fixture request\n",
        );
        write_digest_source(root.path(), "frozen-cli", b"abc");
        root
    }

    #[test]
    fn harness_digest_binds_each_native_control_source() {
        for relative in [
            "crates/auctioneer-cli/src/native_control/mod.rs",
            "crates/auctioneer-cli/src/native_control/io.rs",
            "crates/auctioneer-cli/src/native_control/process.rs",
        ] {
            let root = digest_tree();
            let executable = root.path().join("frozen-cli");
            let source = write_digest_source(root.path(), relative, b"// native control alpha\n");
            let baseline = harness_digest_at(root.path(), &executable).unwrap();
            fs::write(&source, b"// native control beta\n").unwrap();
            assert_ne!(
                harness_digest_at(root.path(), &executable).unwrap(),
                baseline,
                "provider harness digest omitted {relative}"
            );
            fs::write(&source, b"// native control alpha\n").unwrap();
            assert_eq!(
                harness_digest_at(root.path(), &executable).unwrap(),
                baseline
            );
        }
    }

    #[test]
    fn harness_digest_binds_executable_bytes_and_keeps_the_production_file_cap() {
        let root = digest_tree();
        let executable = root.path().join("frozen-cli");
        let baseline = harness_digest_at(root.path(), &executable).unwrap();
        fs::write(&executable, b"changed harness bytes").unwrap();
        assert_ne!(
            harness_digest_at(root.path(), &executable).unwrap(),
            baseline
        );
        fs::write(&executable, b"abc").unwrap();
        assert_eq!(
            harness_digest_at(root.path(), &executable).unwrap(),
            baseline
        );
        fs::OpenOptions::new()
            .write(true)
            .open(&executable)
            .unwrap()
            .set_len(u64::try_from(FILE_CAP).unwrap().checked_add(1).unwrap())
            .unwrap();
        assert_eq!(
            harness_digest_at(root.path(), &executable).unwrap_err(),
            HarnessError::new("input_limit")
        );
    }

    #[test]
    fn fixture_digest_binds_each_native_control_source() {
        for relative in [
            "crates/auctioneer-cli/src/native_control/mod.rs",
            "crates/auctioneer-cli/src/native_control/io.rs",
            "crates/auctioneer-cli/src/native_control/process.rs",
        ] {
            let root = digest_tree();
            let source = write_digest_source(root.path(), relative, b"// native control alpha\n");
            let baseline = fixture_digest_at(root.path()).unwrap();
            fs::write(&source, b"// native control beta\n").unwrap();
            assert_ne!(
                fixture_digest_at(root.path()).unwrap(),
                baseline,
                "provider fixture digest omitted {relative}"
            );
            fs::write(&source, b"// native control alpha\n").unwrap();
            assert_eq!(fixture_digest_at(root.path()).unwrap(), baseline);
        }
    }

    #[test]
    fn digests_exclude_neighboring_unrelated_rust_sources() {
        let root = digest_tree();
        let executable = root.path().join("frozen-cli");
        write_digest_source(
            root.path(),
            "crates/auctioneer-cli/src/native_control/mod.rs",
            b"// native control\n",
        );
        let harness = harness_digest_at(root.path(), &executable).unwrap();
        let fixtures = fixture_digest_at(root.path()).unwrap();
        for relative in [
            "crates/auctioneer-cli/src/native_control_extra/mod.rs",
            "crates/auctioneer-cli/src/native_controls/process.rs",
            "crates/auctioneer-cli/src/qualification/harness_extra/mod.rs",
            "crates/auctioneer-cli/src/resource_identity.rs",
        ] {
            let source = write_digest_source(root.path(), relative, b"// unrelated alpha\n");
            for bytes in [b"// unrelated alpha\n".as_slice(), b"// unrelated beta\n"] {
                fs::write(&source, bytes).unwrap();
                assert_eq!(
                    harness_digest_at(root.path(), &executable).unwrap(),
                    harness,
                    "provider harness digest admitted {relative}"
                );
                assert_eq!(
                    fixture_digest_at(root.path()).unwrap(),
                    fixtures,
                    "provider fixture digest admitted {relative}"
                );
            }
        }
    }

    #[test]
    fn digest_helpers_preserve_existing_payload_formats_and_root_independence() {
        let first = digest_tree();
        let second = digest_tree();
        let sources = vec![
            ("crates/auctioneer-cli/src/qualification/harness/", None),
            (
                "crates/auctioneer-cli/src/qualification/harness/mod.rs",
                Some(io::sha256(b"// existing harness\n")),
            ),
        ];
        let mut fixtures = sources.clone();
        fixtures.extend([
            ("docs/testing/fixtures/provider-handlers/", None),
            (
                "docs/testing/fixtures/provider-handlers/request.txt",
                Some(io::sha256(b"fixture request\n")),
            ),
        ]);
        let harness = io::canonical_digest(&(1_u32, sources, io::sha256(b"abc"))).unwrap();
        let expected_fixtures = io::canonical_digest(&(1_u32, fixtures)).unwrap();
        for root in [&first, &second] {
            assert_eq!(
                harness_digest_at(root.path(), &root.path().join("frozen-cli")).unwrap(),
                harness
            );
            assert_eq!(fixture_digest_at(root.path()).unwrap(), expected_fixtures);
        }
    }

    fn fixture_manifest(key: &str) -> Vec<u8> {
        format!(
            "key = '{key}'\nfactory = true\n[provenance]\norigin = 'local'\n[definitions.{key}]\nendpoint = 'https://bidder.example/auction'\n[definitions.{key}.capabilities.site]\nmediaTypes = ['banner']\n"
        )
        .into_bytes()
    }

    fn factory_fixture(key: &str) -> Vec<u8> {
        let request = serde_json::json!({"id":"owner-request", "imp":[{
            "id":"i", "banner":{"w":300_u32,"h":250_u32},
            "ext":{"bidder":{"fake_placement":"fixture"}}}]});
        serde_json::to_vec(&serde_json::json!({
            "schema_version":1_u32, "implementation":key,
            "cases":[{"request":request,"outbound":[{"body":request}]}]
        }))
        .unwrap()
    }

    async fn commit_fixture(root: &Path, supervisor: &process::Supervisor) {
        for arguments in [
            vec!["init", "--quiet"],
            vec!["add", "--all"],
            vec![
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "-m",
                "Capture isolated test inputs",
            ],
        ] {
            git(supervisor, root, &arguments, INPUT_CAP).await.unwrap();
        }
    }

    const RAW_CATALOG_MEMBERS: &[(&str, &[u8])] = &[
        ("parameters.json", b"{\"type\":\"object\"}\n"),
        ("rawmetadata.yaml", b"name: fake_fixture\n"),
        ("LICENSE.reference", b"Fixture source license\n"),
        ("sourceonly.toml", b"source = 'fixture'\n"),
    ];

    fn raw_catalog_fixture(
        raw_members: usize,
        prefix: &str,
    ) -> (tempfile::TempDir, BTreeMap<String, Vec<u8>>) {
        let root = tempfile::tempdir().unwrap();
        let mut members = BTreeMap::from([
            (
                FIXTURE_MANIFEST.to_owned(),
                fixture_manifest("fake_passthrough"),
            ),
            (
                "fake_passthrough/fixtures/factory.json".to_owned(),
                factory_fixture("fake_passthrough"),
            ),
            (
                "shared/LICENSE".to_owned(),
                b"Shared source license\n".to_vec(),
            ),
        ]);
        for (index, (name, bytes)) in RAW_CATALOG_MEMBERS
            .iter()
            .cycle()
            .take(raw_members)
            .enumerate()
        {
            members.insert(
                format!("{prefix}fake_catalog_{index:04}/{name}"),
                bytes.to_vec(),
            );
        }
        for (member, bytes) in &members {
            let destination = root.path().join(BIDDERS_ROOT).join(member);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::write(destination, bytes).unwrap();
        }
        product_data_fixture(root.path());
        let schema = root
            .path()
            .join("crates/auctioneer-config/src/bidder_manifest.rs");
        fs::create_dir_all(schema.parent().unwrap()).unwrap();
        fs::write(schema, b"// Shared manifest fixture\n").unwrap();
        (root, members)
    }

    fn assert_raw_catalog_receipts(
        repo: &Path,
        members: &BTreeMap<String, Vec<u8>>,
        receipts: &[FileReceipt],
    ) {
        assert_eq!(
            receipts.len(),
            members.len(),
            "catalog must not be truncated"
        );
        let actual: BTreeMap<_, _> = receipts
            .iter()
            .map(|row| (row.path.clone(), row.sha256.clone()))
            .collect();
        let expected: BTreeMap<_, _> = members
            .iter()
            .map(|(member, bytes)| (repo.join(BIDDERS_ROOT).join(member), io::sha256(bytes)))
            .collect();
        assert_eq!(actual, expected, "every raw catalog member needs a receipt");
    }

    fn receipt_snapshot(repo: &Path) -> Vec<FileReceipt> {
        inventory(repo, &[BIDDERS_ROOT], false)
            .unwrap()
            .into_iter()
            .filter_map(|(relative, digest)| {
                digest.map(|sha256| FileReceipt {
                    path: repo.join(relative),
                    sha256,
                })
            })
            .collect()
    }

    #[test]
    fn current_source_has_one_evidenced_passthrough_without_assuming_its_identity() {
        let repo = canonical_directory(&repository_root(), "invalid_input").unwrap();
        let receipts = receipt_snapshot(&repo);
        let selected = select_fixture_bidder(&repo, &receipts).unwrap();
        let manifest = parse_toml(
            &strict_file(&repo.join(BIDDERS_ROOT), &selected.manifest, INPUT_CAP).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["factory"].as_bool(), Some(true));
        assert!(fixed_https_definition(
            &manifest["definitions"][&selected.public_key]
        ));
        assert_eq!(receipts.len(), receipt_snapshot(&repo).len());
    }

    #[test]
    fn selection_carries_the_source_path_and_public_key_without_identity_assumptions() {
        let (root, _members) = raw_catalog_fixture(0, "");
        let repo = root.path().canonicalize().unwrap();
        let path = repo.join(BIDDERS_ROOT).join(FIXTURE_MANIFEST);
        let mut manifest = parse_toml(&fs::read(&path).unwrap()).unwrap();
        let definitions = manifest["definitions"].as_table_mut().unwrap();
        let definition = definitions.remove("fake_passthrough").unwrap();
        definitions.insert("fake_public".to_owned(), definition);
        fs::write(path, encode_toml(&manifest).unwrap()).unwrap();
        let receipts = receipt_snapshot(&repo);
        let selected = select_fixture_bidder(&repo, &receipts).unwrap();
        assert_eq!(selected.manifest, Path::new(FIXTURE_MANIFEST));
        assert_eq!(selected.public_key, "fake_public");
        let rendered: serde_json::Value =
            serde_json::from_slice(&fixture_request(&selected).unwrap()).unwrap();
        let mut template: serde_json::Value =
            io::read_json(&repository_root().join(FIXTURE_REQUEST), INPUT_CAP).unwrap();
        let slots = template["imp"][0]["ext"]["bidder"].as_object_mut().unwrap();
        let params = slots.remove(FIXTURE_ROLE).unwrap();
        slots.insert(selected.public_key, params);
        assert_eq!(
            rendered, template,
            "only the synthetic bidder slot is rebound"
        );
    }

    #[test]
    fn selection_refuses_each_missing_or_unsafe_manifest_property() {
        let (root, _members) = raw_catalog_fixture(0, "");
        let repo = root.path().canonicalize().unwrap();
        let path = repo.join(BIDDERS_ROOT).join(FIXTURE_MANIFEST);
        let original = parse_toml(&fs::read(&path).unwrap()).unwrap();
        for (field, value) in [
            ("factory", toml::Value::Boolean(false)),
            ("settings", toml::Value::Table(toml::Table::new())),
            (
                "required_feature",
                toml::Value::String("fake_optional".into()),
            ),
            ("definitions", toml::Value::Table(toml::Table::new())),
        ] {
            let mut changed = original.clone();
            changed.as_table_mut().unwrap().insert(field.into(), value);
            fs::write(&path, encode_toml(&changed).unwrap()).unwrap();
            assert_eq!(
                select_fixture_bidder(&repo, &receipt_snapshot(&repo))
                    .unwrap_err()
                    .code,
                "invalid_input",
                "{field}"
            );
        }
        for endpoint in [
            "http://bidder.example/auction",
            "https://{{.Host}}/auction",
            "https://bidder.example/{{.AccountID}}",
            "https://user:secret@bidder.example/auction",
            "https://127.0.0.1/auction",
            "https://bidder.example/auction#fragment",
        ] {
            let mut changed = original.clone();
            changed["definitions"]["fake_passthrough"]["endpoint"] =
                toml::Value::String(endpoint.into());
            fs::write(&path, encode_toml(&changed).unwrap()).unwrap();
            assert_eq!(
                select_fixture_bidder(&repo, &receipt_snapshot(&repo)).unwrap_err(),
                HarnessError::new("invalid_input"),
                "{endpoint}"
            );
        }
        for (field, value) in [
            ("hosts", toml::Value::Array(Vec::new())),
            ("settings", toml::Value::Table(toml::Table::new())),
            ("extra_info", toml::Value::Table(toml::Table::new())),
            ("gvlVendorID", toml::Value::Integer(1)),
            ("disabled", toml::Value::Boolean(true)),
            ("capabilities", toml::Value::Table(toml::Table::new())),
        ] {
            let mut changed = original.clone();
            changed["definitions"]["fake_passthrough"]
                .as_table_mut()
                .unwrap()
                .insert(field.into(), value);
            fs::write(&path, encode_toml(&changed).unwrap()).unwrap();
            assert_eq!(
                select_fixture_bidder(&repo, &receipt_snapshot(&repo)).unwrap_err(),
                HarnessError::new("invalid_input"),
                "{field}"
            );
        }
    }

    #[test]
    fn selection_refuses_missing_changed_or_unreceipted_owner_evidence() {
        let (root, _members) = raw_catalog_fixture(0, "");
        let repo = root.path().canonicalize().unwrap();
        let witness = repo
            .join(BIDDERS_ROOT)
            .join("fake_passthrough/fixtures/factory.json");
        let receipts = receipt_snapshot(&repo);
        fs::write(&witness, b"changed").unwrap();
        assert_eq!(
            select_fixture_bidder(&repo, &receipts).unwrap_err().code,
            "production_metadata_changed"
        );
        let original: serde_json::Value =
            serde_json::from_slice(&factory_fixture("fake_passthrough")).unwrap();
        for mutation in ["owner", "body", "empty", "split"] {
            let mut changed = original.clone();
            match mutation {
                "owner" => changed["implementation"] = serde_json::json!("fake_other"),
                "body" => {
                    changed["cases"][0]["outbound"][0]["body"]["id"] = serde_json::json!("changed");
                }
                "empty" => changed["cases"] = serde_json::json!([]),
                "split" => {
                    let call = changed["cases"][0]["outbound"][0].clone();
                    changed["cases"][0]["outbound"]
                        .as_array_mut()
                        .unwrap()
                        .push(call);
                }
                _ => panic!("unknown evidence mutation: {mutation}"),
            }
            fs::write(&witness, serde_json::to_vec(&changed).unwrap()).unwrap();
            assert_eq!(
                select_fixture_bidder(&repo, &receipt_snapshot(&repo)).unwrap_err(),
                HarnessError::new("invalid_input"),
                "{mutation}"
            );
        }
        fs::write(&witness, factory_fixture("fake_passthrough")).unwrap();
        let without_witness: Vec<_> = receipt_snapshot(&repo)
            .into_iter()
            .filter(|row| row.path != witness)
            .collect();
        assert_eq!(
            select_fixture_bidder(&repo, &without_witness).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        fs::remove_file(witness).unwrap();
        assert_eq!(
            select_fixture_bidder(&repo, &receipt_snapshot(&repo)).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        assert_eq!(
            select_fixture_bidder(&repo, &[]).unwrap_err(),
            HarnessError::new("invalid_input")
        );
    }

    #[test]
    fn selection_refuses_multiple_eligible_owners_and_public_definitions() {
        let (root, _members) = raw_catalog_fixture(0, "");
        let repo = root.path().canonicalize().unwrap();
        let path = repo.join(BIDDERS_ROOT).join(FIXTURE_MANIFEST);
        let original = parse_toml(&fs::read(&path).unwrap()).unwrap();
        let mut changed = original.clone();
        let definition = changed["definitions"]["fake_passthrough"].clone();
        changed["definitions"]
            .as_table_mut()
            .unwrap()
            .insert("fake_alias".into(), definition);
        fs::write(&path, encode_toml(&changed).unwrap()).unwrap();
        assert_eq!(
            select_fixture_bidder(&repo, &receipt_snapshot(&repo)).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        fs::write(path, encode_toml(&original).unwrap()).unwrap();
        let extra = repo.join(BIDDERS_ROOT).join("fake_other");
        fs::create_dir_all(extra.join("fixtures")).unwrap();
        fs::write(extra.join("bidder.toml"), fixture_manifest("fake_other")).unwrap();
        fs::write(
            extra.join("fixtures/factory.json"),
            factory_fixture("fake_other"),
        )
        .unwrap();
        let mut receipts = receipt_snapshot(&repo);
        assert_eq!(
            select_fixture_bidder(&repo, &receipts).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        receipts.reverse();
        assert_eq!(
            select_fixture_bidder(&repo, &receipts).unwrap_err(),
            HarnessError::new("invalid_input")
        );
    }

    #[test]
    fn selection_enforces_the_exact_manifest_byte_cap() {
        let (root, _members) = raw_catalog_fixture(0, "");
        let repo = root.path().canonicalize().unwrap();
        let path = repo.join(BIDDERS_ROOT).join(FIXTURE_MANIFEST);
        let mut bytes = fs::read(&path).unwrap();
        bytes.resize(INPUT_CAP, b' ');
        fs::write(&path, &bytes).unwrap();
        select_fixture_bidder(&repo, &receipt_snapshot(&repo)).unwrap();
        bytes.push(b' ');
        fs::write(path, bytes).unwrap();
        assert_eq!(
            select_fixture_bidder(&repo, &receipt_snapshot(&repo))
                .unwrap_err()
                .code,
            "input_limit"
        );
    }

    #[tokio::test]
    async fn production_receipts_capture_more_than_128_raw_catalog_members_without_truncation() {
        let (root, members) = raw_catalog_fixture(129, "");
        let repo = root.path().canonicalize().unwrap();
        let supervisor = process::Supervisor::new();
        commit_fixture(&repo, &supervisor).await;
        let result = production_receipts(&supervisor, &repo).await;
        supervisor.cleanup().await.unwrap();
        let receipts =
            result.expect("native receipts must accept more than 128 raw catalog members");
        assert_raw_catalog_receipts(&repo, &members, &receipts);
    }

    #[tokio::test]
    async fn production_receipts_capture_listing_beyond_input_cap_without_truncation() {
        let component = "catalog".repeat(24);
        let prefix = format!("{component}/{component}/{component}/");
        let (root, members) = raw_catalog_fixture(125, &prefix);
        let repo = root.path().canonicalize().unwrap();
        let supervisor = process::Supervisor::new();
        commit_fixture(&repo, &supervisor).await;
        let listing = git(
            &supervisor,
            &repo,
            &["ls-tree", "-r", "--name-only", "HEAD", "--", BIDDERS_ROOT],
            RECEIPT_CAP,
        )
        .await
        .unwrap();
        assert!(listing.len() > INPUT_CAP);
        assert!(listing.len() < RECEIPT_CAP);
        assert_eq!(members.len(), 128, "isolate the listing byte limit");
        let result = production_receipts(&supervisor, &repo).await;
        supervisor.cleanup().await.unwrap();
        let receipts = result.expect("catalog listings must use the native receipt byte budget");
        assert_raw_catalog_receipts(&repo, &members, &receipts);
    }

    #[tokio::test]
    async fn preparation_freezes_full_raw_catalog_and_refuses_truncation_and_mutations() {
        let (root, members) = raw_catalog_fixture(129, "");
        let repo = root.path().canonicalize().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let supervisor = process::Supervisor::new();
        commit_fixture(&repo, &supervisor).await;
        let plan = prepare(
            PrepareRequest {
                repo_root: repo.clone(),
                workspace: workspace.path().to_owned(),
                provider: "spin".to_owned(),
                origin: "https://fixture.example".to_owned(),
                approved_origin: "https://fixture.example".to_owned(),
                build_features: vec!["spin".to_owned(), OVERRIDE_FEATURE.to_owned()],
            },
            &supervisor,
        )
        .await
        .expect("preparation must freeze the entire raw catalog beyond 128 members");
        assert_raw_catalog_receipts(&repo, &members, &plan.production_files);
        validate_preparation(&plan).unwrap();
        let copied = inventory(&plan.bidders_dir, &[""], false).unwrap();
        assert_eq!(
            copied
                .iter()
                .filter(|(_path, digest)| digest.is_some())
                .count(),
            members.len()
        );
        for (member, bytes) in &members {
            if member != FIXTURE_MANIFEST {
                assert_eq!(fs::read(plan.bidders_dir.join(member)).unwrap(), *bytes);
            }
        }
        let schema = "crates/auctioneer-config/src/bidder_manifest.rs";
        assert!(plan.source_files.contains(&(
            schema.to_owned(),
            Some(io::sha256(&fs::read(repo.join(schema)).unwrap()))
        )));
        assert!(
            !reference_input(schema),
            "shared Rust schema is source, not reference data"
        );
        let loaded: Preparation =
            io::read_json(&workspace.path().join("build-plan.json"), RECEIPT_CAP).unwrap();
        assert_raw_catalog_receipts(&repo, &members, &loaded.production_files);
        verify_frozen_inputs(&loaded, &supervisor).await.unwrap();

        let mut truncated = plan.clone();
        truncated.production_files.truncate(128);
        assert!(
            verify_frozen_inputs(&truncated, &supervisor).await.is_err(),
            "a legacy-sized receipt subset must not validate against the full source tree"
        );
        let mut duplicate = plan.clone();
        duplicate.production_files[0] = duplicate.production_files[1].clone();
        assert!(
            verify_frozen_inputs(&duplicate, &supervisor).await.is_err(),
            "duplicate receipts must not hide a missing catalog member"
        );
        let mut oversized = plan.clone();
        oversized.production_files = vec![plan.production_files[0].clone(); 10_001];
        assert_eq!(
            validate_preparation(&oversized).unwrap_err().code,
            "invalid_input"
        );

        assert_raw_catalog_input_mutations(&plan, &supervisor).await;
        supervisor.cleanup().await.unwrap();
    }

    async fn assert_raw_catalog_input_mutations(
        plan: &Preparation,
        supervisor: &process::Supervisor,
    ) {
        let repo = &plan.repo_root;
        let schema = "crates/auctioneer-config/src/bidder_manifest.rs";
        let mut inputs: Vec<_> = RAW_CATALOG_MEMBERS
            .iter()
            .enumerate()
            .map(|(index, (name, _bytes))| {
                (
                    format!("{BIDDERS_ROOT}/fake_catalog_{index:04}/{name}"),
                    "production_metadata_changed",
                )
            })
            .collect();
        inputs.push((
            format!("{BIDDERS_ROOT}/shared/LICENSE"),
            "production_metadata_changed",
        ));
        inputs.push((schema.to_owned(), "source_changed"));
        inputs.push((PRODUCT_DEFAULTS.to_owned(), "source_changed"));
        for (relative, category) in inputs {
            let path = repo.join(&relative);
            let before = fs::read(&path).unwrap();
            fs::write(&path, b"changed\n").unwrap();
            assert_eq!(
                verify_frozen_inputs(plan, supervisor)
                    .await
                    .unwrap_err()
                    .code,
                category,
                "changed {relative}"
            );
            fs::write(&path, &before).unwrap();
            fs::remove_file(&path).unwrap();
            assert_eq!(
                verify_frozen_inputs(plan, supervisor)
                    .await
                    .unwrap_err()
                    .code,
                category,
                "deleted {relative}"
            );
            fs::write(&path, &before).unwrap();

            let added = format!("{relative}.added");
            fs::write(repo.join(&added), b"added\n").unwrap();
            assert_eq!(
                verify_frozen_inputs(plan, supervisor)
                    .await
                    .unwrap_err()
                    .code,
                "source_changed",
                "untracked {added}"
            );
            git(supervisor, repo, &["add", "--", &added], INPUT_CAP)
                .await
                .unwrap();
            assert_eq!(
                verify_frozen_inputs(plan, supervisor)
                    .await
                    .unwrap_err()
                    .code,
                "source_changed",
                "added {added}"
            );
            git(
                supervisor,
                repo,
                &["rm", "--cached", "--", &added],
                INPUT_CAP,
            )
            .await
            .unwrap();
            fs::remove_file(repo.join(added)).unwrap();
        }
        verify_frozen_inputs(plan, supervisor).await.unwrap();
    }

    #[tokio::test]
    async fn production_receipts_bind_bidder_manifests_source_and_retained_assets() {
        let root = tempfile::tempdir().unwrap();
        let repo = root.path().canonicalize().unwrap();
        let package = "crates/auctioneer-bidder/src/bidders/fake_receipts";
        let directory = repo.join(package);
        fs::create_dir_all(directory.join("fixtures")).unwrap();
        for (relative, bytes) in [
            ("bidder.toml", b"key = 'fake_receipts'\n".as_slice()),
            ("mod.rs", b"// fixture source\n".as_slice()),
            ("fixtures/capture.json", b"[]\n".as_slice()),
        ] {
            fs::write(directory.join(relative), bytes).unwrap();
        }
        let supervisor = process::Supervisor::new();
        commit_fixture(&repo, &supervisor).await;
        let receipts = production_receipts(&supervisor, &repo).await.unwrap();
        assert_eq!(receipts.len(), 3);
        for relative in ["bidder.toml", "mod.rs", "fixtures/capture.json"] {
            let path = directory.join(relative);
            let row = receipts.iter().find(|row| row.path == path).unwrap();
            assert_eq!(row.sha256, io::sha256(&fs::read(&path).unwrap()));
        }
        for relative in ["bidder.toml", "mod.rs", "fixtures/capture.json"] {
            let path = directory.join(relative);
            let before = fs::read(&path).unwrap();
            fs::write(&path, b"changed\n").unwrap();
            assert_eq!(
                production_receipts(&supervisor, &repo)
                    .await
                    .unwrap_err()
                    .code,
                "production_metadata_changed",
                "{relative}"
            );
            fs::write(path, before).unwrap();
        }
        for relative in ["large.rs", "fixtures/large.json"] {
            let bytes = vec![b' '; INPUT_CAP.checked_add(1).unwrap()];
            fs::write(directory.join(relative), bytes).unwrap();
        }
        commit_fixture(&repo, &supervisor).await;
        let expanded_receipts = production_receipts(&supervisor, &repo).await.unwrap();
        assert_eq!(expanded_receipts.len(), 5);
        for relative in ["large.rs", "fixtures/large.json"] {
            let path = directory.join(relative);
            let row = expanded_receipts
                .iter()
                .find(|row| row.path == path)
                .unwrap();
            assert_eq!(row.sha256, io::sha256(&fs::read(path).unwrap()));
        }
        supervisor.cleanup().await.unwrap();
    }

    #[test]
    fn bidder_data_receipts_include_metadata_and_captures_but_not_rust_sources() {
        for member in [
            "fake_metadata/bidder.toml",
            "fake_captured/parameters.json",
            "fake_captured/fixtures/protocol/case.json",
            "fake_captured/fixtures/parameters.json",
            "fake_captured/fixtures/LICENSE.reference",
        ] {
            assert!(
                reference_input(&format!("{BIDDERS_ROOT}/{member}")),
                "{member}"
            );
        }
        for member in [
            "mod.rs",
            "fake_metadata/mod.rs",
            "fake_captured/conformance.rs",
            "README.md",
        ] {
            assert!(
                !reference_input(&format!("{BIDDERS_ROOT}/{member}")),
                "{member}"
            );
        }
        assert!(reference_input(
            "crates/auctioneer-config-data/data/product-defaults.toml"
        ));
        assert!(!reference_input("crates/auctioneer-core/src/lib.rs"));
    }

    #[test]
    fn approved_origins_refuse_every_unsafe_family() {
        validate_origin("https://fixture.example", "https://fixture.example").unwrap();
        validate_origin(
            "https://fixture.example:8443",
            "https://fixture.example:8443",
        )
        .unwrap();
        let unsafe_origins = [
            "http://fixture.example",
            "https://a:b@fixture.example",
            "https://fixture.example/other",
            "https://fixture.example?token=x",
            "https://fixture.example/#x",
            "https://localhost",
            "https://127.0.0.1",
            "https://10.0.0.1",
            "https://[::1]",
            "https://fixture.example.",
            "https://fixture..example",
            "https://-fixture.example",
            "https://fixture-.example",
            "https://fixture.local",
            "https://fixture.internal",
            "https://fixture.localhost",
            "https://Fixture.example",
            "https://fixture.example/",
            "https://fixture.example:443",
            "https://2130706433",
            "https://fixture.example\\@evil.example",
            "https://fixture.example#",
        ];
        let mut checked = 0_usize;
        for origin in unsafe_origins {
            assert_eq!(
                validate_origin(origin, origin).unwrap_err().code,
                "invalid_fixture_origin"
            );
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(checked, 22);
        assert_eq!(
            validate_origin("https://evil.example", "https://fixture.example")
                .unwrap_err()
                .code,
            "invalid_fixture_origin"
        );
        assert_eq!(
            validate_origin(
                &format!("https://{}.example", "a".repeat(64)),
                &format!("https://{}.example", "a".repeat(64))
            )
            .unwrap_err()
            .code,
            "invalid_fixture_origin"
        );
    }

    pub fn minimal_source() -> tempfile::TempDir {
        let (root, _members) = raw_catalog_fixture(0, "");
        let source = canonical_directory(&repository_root(), "invalid_input").unwrap();
        for relative in [
            "Cargo.toml",
            "Cargo.lock",
            "edgezero.toml",
            "crates/auctioneer-config-data/Cargo.toml",
            "crates/auctioneer-core/Cargo.toml",
            "crates/auctioneer-core/src/lib.rs",
            "crates/auctioneer-adapter-spin/spin.toml",
            "fastly.toml",
        ] {
            let destination = root.path().join(relative);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(source.join(relative), destination).unwrap();
        }
        root
    }

    pub async fn prepared(
        selected: &str,
        supervisor: &process::Supervisor,
    ) -> (tempfile::TempDir, tempfile::TempDir, Preparation) {
        prepared_with_source(selected, supervisor, |_root| {}).await
    }

    pub async fn prepared_with_source<F: FnOnce(&Path)>(
        selected: &str,
        supervisor: &process::Supervisor,
        before: F,
    ) -> (tempfile::TempDir, tempfile::TempDir, Preparation) {
        let root = minimal_source();
        commit_fixture(root.path(), supervisor).await;
        before(root.path());
        let workspace = tempfile::tempdir().unwrap();
        let request = PrepareRequest {
            repo_root: root.path().to_owned(),
            workspace: workspace.path().to_owned(),
            provider: selected.to_owned(),
            origin: "https://fixture.example".to_owned(),
            approved_origin: "https://fixture.example".to_owned(),
            build_features: vec![selected.to_owned(), OVERRIDE_FEATURE.to_owned()],
        };
        let plan = prepare(request, supervisor).await.unwrap();
        (root, workspace, plan)
    }

    #[tokio::test]
    async fn preparation_binds_selected_product_data_without_capturing_runtime_authoring() {
        let supervisor = process::Supervisor::new();
        let (_root, _workspace, plan) = prepared("spin", &supervisor).await;
        assert!(
            plan.source_files
                .iter()
                .any(|(path, digest)| path == PRODUCT_DEFAULTS && digest.is_some())
        );
        assert!(
            !plan
                .source_files
                .iter()
                .any(|(path, _digest)| path == "auctioneer.toml")
        );
        let operator = plan.repo_root.join("auctioneer.toml");
        assert!(!operator.exists());
        fs::write(&operator, b"not TOML [").unwrap();
        verify_frozen_inputs(&plan, &supervisor).await.unwrap();
        fs::remove_file(&operator).unwrap();
        verify_frozen_inputs(&plan, &supervisor).await.unwrap();
        let mut forged = plan.clone();
        forged.baked_config_sha256 = io::sha256(b"not the selected product data");
        assert_ne!(forged.baked_config_sha256, plan.baked_config_sha256);
        assert_eq!(
            verify_frozen_inputs(&forged, &supervisor)
                .await
                .unwrap_err()
                .code,
            "source_changed"
        );
        let product = plan.repo_root.join(PRODUCT_DEFAULTS);
        let before = fs::read(&product).unwrap();
        fs::write(&product, b"changed selected product data").unwrap();
        assert_eq!(
            verify_frozen_inputs(&plan, &supervisor)
                .await
                .unwrap_err()
                .code,
            "source_changed"
        );
        fs::write(&product, before).unwrap();
        verify_frozen_inputs(&plan, &supervisor).await.unwrap();
        supervisor.cleanup().await.unwrap();
    }

    #[test]
    fn filesystem_inventory_includes_ignored_untracked_build_inputs_and_precise_target_sources() {
        let root = minimal_source();
        fs::create_dir_all(root.path().join(".cargo")).unwrap();
        fs::write(root.path().join(".cargo/config.toml"), b"[build]\njobs=1\n").unwrap();
        fs::write(
            root.path().join("crates/auctioneer-core/build.rs"),
            b"fn main() {}\n",
        )
        .unwrap();
        fs::write(
            root.path().join(".gitignore"),
            b"crates/auctioneer-core/src/hidden.rs\n",
        )
        .unwrap();
        let initial = source_inventory(root.path()).unwrap();
        for relative in [
            "fastly.toml",
            "crates/auctioneer-core/src/hidden.rs",
            "crates/auctioneer-core/src/untracked.rs",
            "crates/auctioneer-core/src/target.rs",
            "crates/auctioneer-core/src/target/mod.rs",
            "crates/auctioneer-core/build-input.bin",
        ] {
            let file = root.path().join(relative);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(&file, b"A").unwrap();
            let first = source_inventory(root.path()).unwrap();
            assert!(
                first
                    .iter()
                    .any(|row| row.0 == relative && row.1 == Some(io::sha256(b"A")))
            );
            fs::write(&file, b"B").unwrap();
            assert_ne!(first, source_inventory(root.path()).unwrap());
            fs::remove_file(&file).unwrap();
            assert_ne!(first, source_inventory(root.path()).unwrap());
        }
        assert!(initial.iter().any(|row| row.0 == ".cargo/config.toml"));
        assert!(
            initial
                .iter()
                .any(|row| row.0 == "crates/auctioneer-core/build.rs")
        );
    }

    #[test]
    fn cargo_owned_targets_ignore_create_change_remove_even_broken_links() {
        let root = minimal_source();
        let baseline = source_inventory(root.path()).unwrap();
        for generated in [
            "target",
            "crates/auctioneer-core/target",
            "crates/auctioneer-config-data/target",
        ] {
            let directory = root.path().join(generated);
            fs::create_dir_all(directory.join("debug/deps")).unwrap();
            fs::write(directory.join("debug/deps/cache.o"), b"A").unwrap();
            #[cfg(unix)]
            symlink("missing", directory.join("broken")).unwrap();
            assert_eq!(source_inventory(root.path()).unwrap(), baseline);
            fs::write(directory.join("debug/deps/cache.o"), b"B").unwrap();
            assert_eq!(source_inventory(root.path()).unwrap(), baseline);
            fs::remove_dir_all(directory).unwrap();
            assert_eq!(source_inventory(root.path()).unwrap(), baseline);
        }
    }

    #[cfg(unix)]
    #[test]
    fn inventories_refuse_source_symlinks_hardlinks_and_parent_escapes() {
        let root = minimal_source();
        let source = root.path().join("crates/auctioneer-core/src/lib.rs");
        let link = root.path().join("crates/auctioneer-core/src/link.rs");
        symlink(&source, &link).unwrap();
        source_inventory(root.path()).unwrap_err();
        fs::remove_file(&link).unwrap();
        fs::hard_link(&source, &link).unwrap();
        source_inventory(root.path()).unwrap_err();
        strict_file(root.path(), Path::new("../Cargo.toml"), INPUT_CAP).unwrap_err();
    }

    #[test]
    fn toml_and_inventory_caps_are_executed_not_only_declared() {
        parse_toml(&vec![b'a'; INPUT_CAP.checked_add(1).unwrap()]).unwrap_err();
        parse_toml(b"SECRET_CANARY").unwrap_err();
        let deep = format!("[{}]\nx=1\n", vec!["a"; 34].join("."));
        assert_eq!(parse_toml(deep.as_bytes()).unwrap_err().code, "input_limit");
        let root = tempfile::tempdir().unwrap();
        let mut nested = root.path().join("crates");
        for _index in 0_u8..34 {
            nested.push("x");
        }
        fs::create_dir_all(nested).unwrap();
        assert_eq!(
            source_inventory(root.path()).unwrap_err().code,
            "input_limit"
        );
    }

    #[tokio::test]
    async fn preparation_is_private_isolated_secure_and_production_metadata_never_changes() {
        let supervisor = process::Supervisor::new();
        let (_root, _workspace, plan) = prepared("spin", &supervisor).await;
        let before = strict_file(
            &plan.repo_root,
            &Path::new(BIDDERS_ROOT).join(&plan.fixture_bidder.manifest),
            INPUT_CAP,
        )
        .unwrap();
        let mut expected = parse_toml(&before).unwrap();
        expected
            .get_mut("definitions")
            .unwrap()
            .get_mut(plan.fixture_bidder.public_key.as_str())
            .unwrap()
            .as_table_mut()
            .unwrap()
            .insert(
                "endpoint".to_owned(),
                toml::Value::String("https://fixture.example/openrtb2/auction".to_owned()),
            );
        assert_eq!(
            parse_toml(
                &strict_file(&plan.bidders_dir, &plan.fixture_bidder.manifest, INPUT_CAP).unwrap()
            )
            .unwrap(),
            expected
        );
        verify_production(&plan).unwrap();
        assert_eq!(
            fs::read(
                plan.repo_root
                    .join(BIDDERS_ROOT)
                    .join(&plan.fixture_bidder.manifest)
            )
            .unwrap(),
            before
        );
        assert_eq!(plan.env.len(), 2);
        assert_eq!(plan.env["AUCTIONEER__BIDDERS__DIR"], plan.bidders_dir);
        let prepared_inputs = inventory(&plan.bidders_dir, &[""], false).unwrap();
        let original_inputs = inventory(&plan.repo_root.join(BIDDERS_ROOT), &[""], false).unwrap();
        assert_eq!(prepared_inputs.len(), original_inputs.len());
        for (member, digest) in original_inputs {
            if Path::new(&member) == plan.fixture_bidder.manifest {
                assert_ne!(
                    prepared_inputs
                        .iter()
                        .find(|row| row.0 == member)
                        .unwrap()
                        .1,
                    digest
                );
            } else {
                assert!(prepared_inputs.contains(&(member, digest)));
            }
        }
        assert!(plan.args.contains(&"--no-default-features".to_owned()));
        assert!(plan.args.contains(&"--offline".to_owned()));
        assert_eq!(plan.harness_sha256, harness_digest().unwrap());
        assert_eq!(plan.fixtures_sha256, fixture_digest().unwrap());
        assert_eq!(plan.target, "wasm32-wasip2");
        let mut changed = plan.clone();
        changed.production_files[0].sha256 = "b".repeat(64);
        assert_eq!(
            verify_production(&changed).unwrap_err().code,
            "production_metadata_changed"
        );
        assert_eq!(
            isolated_workspace(&plan.repo_root, &plan.repo_root, true)
                .unwrap_err()
                .code,
            "workspace_not_isolated"
        );
        assert_eq!(
            check_features(provider("spin").unwrap(), &["spin".to_owned()])
                .unwrap_err()
                .code,
            "build_feature_missing"
        );
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn selected_binding_and_prepared_request_cannot_be_substituted_or_removed() {
        let supervisor = process::Supervisor::new();
        let (_source, _work, plan) = prepared("spin", &supervisor).await;
        for (manifest, public_key) in [
            (
                PathBuf::from("fake_other/bidder.toml"),
                plan.fixture_bidder.public_key.clone(),
            ),
            (
                PathBuf::from("../bidder.toml"),
                plan.fixture_bidder.public_key.clone(),
            ),
            (plan.fixture_bidder.manifest.clone(), "fake_other".into()),
        ] {
            let mut changed = plan.clone();
            changed.fixture_bidder = FixtureBidder {
                manifest,
                public_key,
            };
            assert_eq!(
                verify_frozen_inputs(&changed, &supervisor)
                    .await
                    .unwrap_err()
                    .code,
                "invalid_input"
            );
        }
        let request = plan.workspace.join("banner-request.json");
        let original = fs::read(&request).unwrap();
        assert_eq!(original, fixture_request(&plan.fixture_bidder).unwrap());
        fs::write(&request, b"changed").unwrap();
        assert_eq!(
            verify_frozen_inputs(&plan, &supervisor)
                .await
                .unwrap_err()
                .code,
            "fixture_changed"
        );
        fs::remove_file(&request).unwrap();
        assert_eq!(
            verify_frozen_inputs(&plan, &supervisor)
                .await
                .unwrap_err()
                .code,
            "fixture_changed"
        );
        fs::write(request, original).unwrap();
        verify_frozen_inputs(&plan, &supervisor).await.unwrap();
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn preparation_file_command_emits_only_safe_metadata_and_refuses_unknown_input() {
        let supervisor = process::Supervisor::new();
        let (_source, _work, base) = prepared("spin", &supervisor).await;
        let destination = tempfile::tempdir().unwrap();
        let inputs = tempfile::tempdir().unwrap();
        let input = inputs.path().join("SECRET_INPUT.json");
        fs::write(&input, b"SECRET_CONTENT").unwrap();
        assert_eq!(
            prepare_build(&input, &supervisor).await.unwrap_err(),
            HarnessError::new("invalid_input")
        );
        let request = serde_json::json!({"repo_root":base.repo_root,"workspace":destination.path(),"provider":"spin",
            "origin":"https://fixture.example","approved_origin":"https://fixture.example","build_features":["spin","auctioneer-config-data/test-harness-override"]});
        fs::write(&input, serde_json::to_vec(&request).unwrap()).unwrap();
        let result = prepare_build(&input, &supervisor).await.unwrap();
        assert_eq!(result["kind"], "offline_handler_preparation");
        assert_eq!(result["deployed_handler_acceptance"], "pending");
        assert_eq!(result["resource_qualification"], "not_evaluated");
        assert_eq!(result["publisher_pilot_ready"], false);
        let keys = result
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            [
                "baked_config_sha256",
                "build_features",
                "deployed_handler_acceptance",
                "fixture_metadata_sha256",
                "fixtures_sha256",
                "harness_sha256",
                "kind",
                "publisher_pilot_ready",
                "reference_data_sha256",
                "resource_qualification",
                "schema_version",
                "source_sha256",
                "target"
            ]
        );
        let loaded: Preparation =
            io::read_json(&destination.path().join("build-plan.json"), RECEIPT_CAP).unwrap();
        verify_frozen_inputs(&loaded, &supervisor).await.unwrap();
        supervisor.cleanup().await.unwrap();
    }
}
