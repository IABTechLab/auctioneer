use super::super::{
    HarnessError, Result, io,
    process::{ProcessSpec, Supervisor},
};
use super::{
    geometry,
    inputs::{self, Candidate},
};
use crate::qualification::profile::Digest;
use auctioneer_config_data::types::{BidderInfo, BidderMetadataDeclaration, MediaType};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read as _, Write as _};
use std::path::{Path, PathBuf};

const ARTIFACT_CAP: usize = 512 << 20_u32;
const SNAPSHOT_CAP: usize = 1 << 20_u32;
const SNAPSHOT_FILE: &str = ".edgezero/local-config-auctioneer_config.json";
// The native fixture listener is shared with the runner and frozen-binary probes.
const FIXTURE_ENDPOINT: &str = "http://127.0.0.1:8787/openrtb2/auction";

#[derive(Clone, Copy, Serialize)]
struct PurposeTwo {
    #[serde(rename = "enforce_algo")]
    algo: &'static str,
    #[serde(rename = "enforce_purpose")]
    purpose: &'static str,
    #[serde(rename = "enforce_vendors")]
    vendors: &'static str,
}

const PURPOSE_TWO: PurposeTwo = PurposeTwo {
    algo: "full",
    purpose: "enabled",
    vendors: "enabled",
};

#[derive(Deserialize, Serialize)]
struct LocalStore {
    auctioneer_config: String,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct EnvelopeDigest {
    sha256: Digest,
}

#[derive(Clone, Debug)]
pub struct InstalledVariant {
    pub id: u16,
    pub dir: PathBuf,
    pub config_sha256: Digest,
    pub settings_sha256: Digest,
    pub nodes: u32,
    pub banner_sha256: Digest,
    pub store_sha256: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub config_sha256: Digest,
    pub store_sha256: Digest,
}

pub async fn install_variant(
    cli: &Path,
    repo: &Path,
    dir: &Path,
    candidate: &Candidate,
    id: u16,
    supervisor: &Supervisor,
) -> Result<InstalledVariant> {
    let document = config_document(candidate, id)?;
    install_document(cli, repo, dir, candidate, id, &document, supervisor).await
}

// Historical binaries still author their historical schema. This fixture-only
// path never enters current config validate/diff/push or rewrites saved evidence.
pub(super) async fn install_historical_variant(
    cli: &Path,
    repo: &Path,
    dir: &Path,
    candidate: &Candidate,
    id: u16,
    supervisor: &Supervisor,
) -> Result<InstalledVariant> {
    let document = historical_config_document(candidate, id)?;
    install_document(cli, repo, dir, candidate, id, &document, supervisor).await
}

async fn install_document(
    cli: &Path,
    repo: &Path,
    dir: &Path,
    candidate: &Candidate,
    id: u16,
    document: &toml::Value,
    supervisor: &Supervisor,
) -> Result<InstalledVariant> {
    let nodes = variant_nodes(candidate, id)?;
    let banner_sha256 = Digest::of(variant_banner(id)?.as_bytes());
    let settings_sha256 = Digest::canonical(&json!({
        "nodes": nodes, "calls": candidate.maximum_load.calls, "banner_sha256": banner_sha256,
        "debug_allowed": true, "debug_denied": false, "purpose2": PURPOSE_TWO,
    }))
    .map_err(|_error| HarnessError::new("invalid_input"))?;
    supervisor.check_active().await?;
    let repository = real_directory(repo)?;
    let variant_dir = real_directory(dir)?;
    if variant_dir.starts_with(&repository) || repository.starts_with(&variant_dir) {
        return Err(HarnessError::new("workspace_not_isolated"));
    }
    real_directory(&repository.join("crates"))?;
    real_directory(&repository.join("crates/auctioneer-adapter-axum"))?;
    ensure_directory(&variant_dir.join("crates"))?;
    ensure_directory(&variant_dir.join("crates/auctioneer-adapter-axum"))?;
    ensure_directory(&variant_dir.join(".edgezero"))?;
    for relative in ["edgezero.toml", "crates/auctioneer-adapter-axum/axum.toml"] {
        let bytes = strict_bytes(&repository.join(relative), SNAPSHOT_CAP)?;
        replace_private(&variant_dir.join(relative), &bytes)?;
    }
    let encoded = toml::to_string(document).map_err(|_error| HarnessError::new("invalid_input"))?;
    if encoded.len() > SNAPSHOT_CAP {
        return Err(HarnessError::new("output_limit"));
    }
    replace_private(&variant_dir.join("auctioneer.toml"), encoded.as_bytes())?;
    let store = variant_dir.join(SNAPSHOT_FILE);
    validate_replaceable(&store)?;
    supervisor.run(config_push_spec(cli, &variant_dir)).await?;
    supervisor.check_active().await?;
    let snapshot = read_snapshot(&variant_dir)?;
    Ok(InstalledVariant {
        id,
        dir: variant_dir,
        config_sha256: snapshot.config_sha256,
        settings_sha256,
        nodes,
        banner_sha256,
        store_sha256: snapshot.store_sha256,
    })
}

pub fn read_snapshot(dir: &Path) -> Result<Snapshot> {
    let path = snapshot_path(dir)?;
    let bytes = strict_bytes(&path, SNAPSHOT_CAP)?;
    let outer: LocalStore = inputs::parse_json(&bytes, SNAPSHOT_CAP)
        .map_err(|_error| HarnessError::new("snapshot_invalid"))?;
    let envelope: EnvelopeDigest =
        inputs::parse_json(outer.auctioneer_config.as_bytes(), SNAPSHOT_CAP)
            .map_err(|_error| HarnessError::new("snapshot_invalid"))?;
    Ok(Snapshot {
        config_sha256: envelope.sha256,
        store_sha256: Digest::of(&bytes),
    })
}

pub fn corrupt_snapshot(dir: &Path) -> Result<()> {
    let path = snapshot_path(dir)?;
    let bytes = strict_bytes(&path, SNAPSHOT_CAP)?;
    let mut outer: LocalStore = inputs::parse_json(&bytes, SNAPSHOT_CAP)
        .map_err(|_error| HarnessError::new("snapshot_invalid"))?;
    "{".clone_into(&mut outer.auctioneer_config);
    let encoded = io::json_bytes(&outer, SNAPSHOT_CAP)?;
    if file_digest(&path, SNAPSHOT_CAP)? != Digest::of(&bytes) {
        return Err(HarnessError::new("input_changed"));
    }
    replace_private(&path, &encoded)
}

pub fn freeze_artifact(source: &Path, destination: &Path) -> Result<Digest> {
    let digest = file_digest(source, ARTIFACT_CAP)?;
    let mut input = open_regular(source, ARTIFACT_CAP)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600)
    };
    let mut output = options
        .open(destination)
        .map_err(|_error| HarnessError::new("output_io"))?;
    if copy_stream(&mut input, &mut output)? != digest {
        return Err(HarnessError::new("identity_mismatch"));
    }
    output
        .sync_all()
        .map_err(|_error| HarnessError::new("output_io"))?;
    freeze_permissions(&output)?;
    assert_artifact(source, &digest)?;
    assert_artifact(destination, &digest)?;
    Ok(digest)
}

pub fn assert_artifact(path: &Path, digest: &Digest) -> Result<()> {
    if &file_digest(path, ARTIFACT_CAP)? != digest {
        return Err(HarnessError::new("identity_mismatch"));
    }
    Ok(())
}

fn config_document(candidate: &Candidate, id: u16) -> Result<toml::Value> {
    let nodes = variant_nodes(candidate, id)?;
    let declarations = auctioneer_config_data::bidder_metadata_declarations();
    let bidder = fixture_bidder(declarations, auctioneer_config_data::bidders())?;
    let implementation = declarations
        .iter()
        .find(|row| row.bidder == bidder)
        .ok_or(HarnessError::new("fixture_unavailable"))?
        .implementation;
    let aliases = geometry::aliases(candidate.maximum_load.calls)?
        .into_iter()
        .map(|name| (name, bidder))
        .collect::<BTreeMap<_, _>>();
    let document = json!({
        "schema_version": 1_u32,
        "host": {
            "resource_limits": {
                "default_tmax_ms": 500_u16, "max_tmax_ms": 1000_u16,
                "max_parsed_nodes": nodes,
                "max_outbound_requests": candidate.maximum_load.calls,
                "max_bidders_per_auction": candidate.maximum_load.calls,
                "max_transport_bytes": candidate.maximum_load.encoded_bytes,
                "max_decoded_bytes": candidate.maximum_load.decoded_bytes,
            },
            "status_response": variant_banner(id)?,
            "aliases": aliases,
            "gdpr": {
                "enabled": true, "default_value": "0",
                "tcf2": {"enabled": true, "purposes": {"2": PURPOSE_TWO}},
            },
        },
        "accounts": {"q2_allowed": {"debug_allow": true}, "q2_refused": {"debug_allow": false}},
        "bidders": {(bidder): {
            "enabled": true, "implementation": implementation,
            "settings": {"endpoint": FIXTURE_ENDPOINT},
            "timeout_ms": 500_u16, "sample_rate": 1.0_f64,
        }},
    });
    toml::Value::try_from(document).map_err(|_error| HarnessError::new("invalid_input"))
}

fn historical_config_document(candidate: &Candidate, id: u16) -> Result<toml::Value> {
    let mut current = config_document(candidate, id)?;
    let entries = current
        .get_mut("bidders")
        .and_then(toml::Value::as_table_mut)
        .ok_or(HarnessError::new("invalid_input"))?;
    for (_, entry) in entries.iter_mut() {
        let fields = entry
            .as_table_mut()
            .ok_or(HarnessError::new("invalid_input"))?;
        fields.remove("enabled");
        fields.remove("implementation");
        fields.remove("settings");
    }
    let invalid = HarnessError::new("invalid_input");
    let host = current.get("host").ok_or(invalid)?;
    let document = json!({
        "baked": {"host": {}, "account": {}},
        "runtime": {
            "auction": host.get("resource_limits").ok_or(invalid)?,
            "status": {"banner": host.get("status_response").ok_or(invalid)?},
            "aliases": host.get("aliases").ok_or(invalid)?,
            "gdpr": host.get("gdpr").ok_or(invalid)?,
            "accounts": current.get("accounts").ok_or(invalid)?,
            "bidders": current.get("bidders").ok_or(invalid)?,
        },
    });
    toml::Value::try_from(document).map_err(|_error| HarnessError::new("invalid_input"))
}

fn fixture_bidder(
    declarations: &[BidderMetadataDeclaration],
    infos: &[BidderInfo],
) -> Result<&'static str> {
    // The native child opts into local transport. Discover its configurable,
    // default-disabled publication from generated bindings and metadata.
    let mut candidates = declarations.iter().filter(|declaration| {
        declaration.required_feature.is_none()
            && infos
                .iter()
                .find(|info| info.name == declaration.bidder)
                .is_some_and(|info| {
                    !info.disabled
                        && info.default_enabled == Some(false)
                        && info.debug_allow
                        && info.gvl_vendor_id.is_none()
                        && info.endpoint.is_none()
                        && info.endpoint_macros.is_empty()
                        && info.endpoint_compression.is_none()
                        && info.capabilities.site.contains(&MediaType::Banner)
                })
    });
    let selected = candidates
        .next()
        .ok_or(HarnessError::new("fixture_unavailable"))?;
    if candidates.next().is_some() {
        return Err(HarnessError::new("fixture_unavailable"));
    }
    Ok(selected.bidder)
}

fn variant_nodes(candidate: &Candidate, id: u16) -> Result<u32> {
    let maximum = candidate.maximum_load.nodes;
    if !(1..=0x0001_0000).contains(&maximum)
        || !(1..=0x0001_0000).contains(&candidate.base_load.nodes)
    {
        return Err(HarnessError::new("invalid_input"));
    }
    match id {
        1 | 3 => Ok(maximum),
        2 => Ok(candidate.base_load.nodes.max(
            maximum
                .checked_div(2)
                .ok_or(HarnessError::new("invalid_input"))?,
        )),
        _ => Err(HarnessError::new("invalid_input")),
    }
}

const fn variant_banner(id: u16) -> Result<&'static str> {
    match id {
        1 | 3 => Ok("q2_a"),
        2 => Ok("q2_b"),
        _ => Err(HarnessError::new("invalid_input")),
    }
}

fn config_push_spec(cli: &Path, dir: &Path) -> ProcessSpec {
    let mut spec = ProcessSpec::new(cli);
    spec.cwd = Some(dir.to_path_buf());
    spec.args = vec![
        OsString::from("config"),
        OsString::from("push"),
        OsString::from("--adapter"),
        OsString::from("axum"),
        OsString::from("--manifest"),
        dir.join("edgezero.toml").into_os_string(),
        OsString::from("--app-config"),
        dir.join("auctioneer.toml").into_os_string(),
        OsString::from("--local"),
        OsString::from("--yes"),
        OsString::from("--no-env"),
        OsString::from("--no-diff"),
    ];
    spec.output_cap = 32 << 10_u32;
    spec
}

fn snapshot_path(dir: &Path) -> Result<PathBuf> {
    let path = real_directory(dir)?.join(SNAPSHOT_FILE);
    real_directory(path.parent().ok_or(HarnessError::new("invalid_input"))?)?;
    Ok(path)
}

fn real_directory(path: &Path) -> Result<PathBuf> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_error| HarnessError::new("invalid_input"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(HarnessError::new("invalid_input"));
    }
    fs::canonicalize(path).map_err(|_error| HarnessError::new("input_io"))
}

fn ensure_directory(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => real_directory(path).map(|_canonical| ()),
        Err(error) if error.kind() == ErrorKind::NotFound => {
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
        Err(_) => Err(HarnessError::new("input_io")),
    }
}

fn regular_metadata(metadata: &fs::Metadata) -> Result<()> {
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(HarnessError::new("invalid_input"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if metadata.nlink() != 1 {
            return Err(HarnessError::new("invalid_input"));
        }
    }
    Ok(())
}

fn validate_replaceable(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => regular_metadata(&metadata),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(_) => Err(HarnessError::new("input_io")),
    }
}

fn open_regular(path: &Path, cap: usize) -> Result<File> {
    #[cfg(unix)]
    let file = {
        use rustix::fs::{Mode, OFlags, open};
        let flags = OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC | OFlags::NOFOLLOW;
        let descriptor = open(path, flags, Mode::empty())
            .map_err(|_error| HarnessError::new("invalid_input"))?;
        File::from(descriptor)
    };
    #[cfg(not(unix))]
    let file = {
        regular_metadata(
            &fs::symlink_metadata(path).map_err(|_error| HarnessError::new("input_io"))?,
        )?;
        File::open(path).map_err(|_error| HarnessError::new("input_io"))?
    };
    let metadata = file
        .metadata()
        .map_err(|_error| HarnessError::new("input_io"))?;
    regular_metadata(&metadata)?;
    if metadata.len() > u64::try_from(cap).map_err(|_error| HarnessError::new("input_limit"))? {
        return Err(HarnessError::new("input_limit"));
    }
    Ok(file)
}

fn file_digest(path: &Path, cap: usize) -> Result<Digest> {
    Digest::try_from(io::file_digest(path, cap)?)
        .map_err(|_error| HarnessError::new("invalid_input"))
}

fn strict_bytes(path: &Path, cap: usize) -> Result<Vec<u8>> {
    let file = open_regular(path, cap)?;
    let limit = u64::try_from(cap).map_err(|_error| HarnessError::new("input_limit"))?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_error| HarnessError::new("input_io"))?;
    if bytes.len() > cap {
        return Err(HarnessError::new("input_limit"));
    }
    if file_digest(path, cap)? != Digest::of(&bytes) {
        return Err(HarnessError::new("input_changed"));
    }
    Ok(bytes)
}

fn replace_private(path: &Path, bytes: &[u8]) -> Result<()> {
    if bytes.len() > SNAPSHOT_CAP {
        return Err(HarnessError::new("output_limit"));
    }
    let parent = path.parent().ok_or(HarnessError::new("invalid_input"))?;
    real_directory(parent)?;
    validate_replaceable(path)?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|_error| HarnessError::new("output_io"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_error| HarnessError::new("output_io"))?;
    };
    temporary
        .write_all(bytes)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|_error| HarnessError::new("output_io"))?;
    validate_replaceable(path)?;
    temporary
        .persist(path)
        .map_err(|_error| HarnessError::new("output_io"))?;
    Ok(())
}

fn copy_stream(input: &mut File, output: &mut File) -> Result<Digest> {
    let mut digest = Sha256::new();
    let mut count = 0_usize;
    let mut buffer = [0_u8; 8192];
    loop {
        let length = input
            .read(&mut buffer)
            .map_err(|_error| HarnessError::new("input_io"))?;
        if length == 0 {
            break;
        }
        count = count
            .checked_add(length)
            .filter(|total| *total <= ARTIFACT_CAP)
            .ok_or(HarnessError::new("input_limit"))?;
        let chunk = buffer.get(..length).ok_or(HarnessError::new("input_io"))?;
        digest.update(chunk);
        output
            .write_all(chunk)
            .map_err(|_error| HarnessError::new("output_io"))?;
    }
    Digest::try_from(hex::encode(digest.finalize()))
        .map_err(|_error| HarnessError::new("invalid_input"))
}

fn freeze_permissions(file: &File) -> Result<()> {
    #[cfg(unix)]
    let permissions = {
        use std::os::unix::fs::PermissionsExt as _;
        fs::Permissions::from_mode(0o500)
    };
    #[cfg(not(unix))]
    let permissions = {
        let mut permissions = file
            .metadata()
            .map_err(|_error| HarnessError::new("output_io"))?
            .permissions();
        permissions.set_readonly(true);
        permissions
    };
    file.set_permissions(permissions)
        .map_err(|_error| HarnessError::new("output_io"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Args, Command, application};
    use auctioneer_config_data::types::{Capabilities, Compression};
    use auctioneer_core::config::SdkAuctioneerAppConfig;
    use clap::Parser as _;
    use edgezero_core::blob_envelope::BlobEnvelope;
    use serde_json::{Value, json};
    use std::ffi::OsString;
    use std::fs;
    use std::iter::once;
    use std::slice::from_ref;
    use validator::Validate as _;

    fn candidate() -> Candidate {
        serde_json::from_slice(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/testing/fixtures/resource-qualification/native-axum.json"
        )))
        .unwrap()
    }

    fn snapshot_file(root: &Path, outer: &Value) -> PathBuf {
        fs::create_dir_all(root.join(".edgezero")).unwrap();
        let path = root.join(SNAPSHOT_FILE);
        fs::write(&path, serde_json::to_vec(outer).unwrap()).unwrap();
        path
    }

    fn valid_store() -> Value {
        let envelope = BlobEnvelope::new(
            json!({"schema_version": 1_u32, "host": {"status_response": "q2_a"}}),
            "2026-10-15T00:00:00Z".to_owned(),
        );
        json!({"auctioneer_config": serde_json::to_string(&envelope).unwrap(), "other_store": "PRESERVE_OTHER_STORE"})
    }

    fn configurable_info(name: &str) -> BidderInfo {
        BidderInfo {
            name: name.to_owned(),
            default_enabled: Some(false),
            debug_allow: true,
            capabilities: Capabilities {
                site: vec![MediaType::Banner],
                ..Capabilities::default()
            },
            ..BidderInfo::default()
        }
    }

    #[test]
    fn fixture_selection_requires_unique_configurable_metadata_without_a_bidder_identity() {
        let local = BidderMetadataDeclaration {
            bidder: "fake_local",
            implementation: "fake_transport",
            required_feature: None,
        };
        let ordinary = BidderMetadataDeclaration {
            bidder: "fake_secure",
            ..local
        };
        assert_eq!(
            fixture_bidder(&[ordinary, local], &[configurable_info("fake_local")]).unwrap(),
            "fake_local"
        );
        assert_eq!(
            fixture_bidder(&[local], &[]).unwrap_err(),
            HarnessError::new("fixture_unavailable")
        );
        assert_eq!(
            fixture_bidder(&[ordinary], &[configurable_info("fake_local")]).unwrap_err(),
            HarnessError::new("fixture_unavailable")
        );
        let sibling = BidderMetadataDeclaration {
            bidder: "fake_sibling",
            ..local
        };
        assert_eq!(
            fixture_bidder(
                &[local, sibling],
                &[
                    configurable_info("fake_local"),
                    configurable_info("fake_sibling")
                ]
            )
            .unwrap_err(),
            HarnessError::new("fixture_unavailable")
        );
    }

    #[test]
    fn active_fixture_metadata_must_preserve_the_real_native_transport_obligations() {
        let local = BidderMetadataDeclaration {
            bidder: "fake_local",
            implementation: "fake_transport",
            required_feature: None,
        };
        let info = configurable_info(local.bidder);
        assert_eq!(
            fixture_bidder(&[local], from_ref(&info)).unwrap(),
            "fake_local"
        );
        let disqualify: [fn(&mut BidderInfo); 8] = [
            |row| row.disabled = true,
            |row| row.default_enabled = None,
            |row| row.debug_allow = false,
            |row| row.gvl_vendor_id = Some(1),
            |row| row.endpoint = Some("http://127.0.0.1:8788/openrtb2/auction".to_owned()),
            |row| row.endpoint_macros.push("AccountID".to_owned()),
            |row| row.endpoint_compression = Some(Compression::Gzip),
            |row| row.capabilities.site.clear(),
        ];
        for change in disqualify {
            let mut invalid = info.clone();
            change(&mut invalid);
            assert_eq!(
                fixture_bidder(&[local], &[invalid]).unwrap_err(),
                HarnessError::new("fixture_unavailable")
            );
        }
    }

    #[test]
    fn structured_toml_preserves_the_native_fixture_fields_in_the_direct_runtime_model() {
        let document = config_document(&candidate(), 1).unwrap();
        let encoded = toml::to_string(&document).unwrap();
        let source: SdkAuctioneerAppConfig = toml::from_str(&encoded).unwrap();
        let runtime = serde_json::to_value(source).unwrap();
        assert!(runtime.get("baked").is_none());
        assert!(runtime.get("runtime").is_none());
        assert_eq!(runtime["schema_version"], 1_u32);
        assert_eq!(
            runtime["host"]["resource_limits"]["default_tmax_ms"],
            500_u16
        );
        assert_eq!(runtime["host"]["resource_limits"]["max_tmax_ms"], 1000_u16);
        assert_eq!(
            runtime["host"]["resource_limits"]["max_outbound_requests"],
            16_u32
        );
        assert_eq!(
            runtime["host"]["resource_limits"]["max_bidders_per_auction"],
            16_u32
        );
        assert_eq!(
            runtime["host"]["resource_limits"]["max_transport_bytes"],
            0x0000_4000_u64
        );
        assert_eq!(
            runtime["host"]["resource_limits"]["max_decoded_bytes"],
            0x0000_8000_u64
        );
        assert_eq!(runtime["accounts"]["q2_allowed"]["debug_allow"], true);
        assert_eq!(runtime["accounts"]["q2_refused"]["debug_allow"], false);
        assert_eq!(runtime["host"]["gdpr"]["enabled"], true);
        assert_eq!(runtime["host"]["gdpr"]["default_value"], "0");
        assert_eq!(runtime["host"]["gdpr"]["tcf2"]["enabled"], true);
        assert_eq!(
            runtime["host"]["gdpr"]["tcf2"]["purposes"]["2"],
            json!({
                "enforce_algo": "full", "enforce_purpose": "enabled", "enforce_vendors": "enabled",
            })
        );
        let bidder = fixture_bidder(
            auctioneer_config_data::bidder_metadata_declarations(),
            auctioneer_config_data::bidders(),
        )
        .unwrap();
        assert_eq!(runtime["bidders"].as_object().unwrap().len(), 1);
        assert_eq!(runtime["bidders"][bidder]["timeout_ms"], 500_u16);
        assert_eq!(runtime["bidders"][bidder]["sample_rate"], 1.0_f64);
        let aliases = runtime["host"]["aliases"].as_object().unwrap();
        assert_eq!(aliases.len(), 16);
        for index in 0..16_u8 {
            assert_eq!(aliases[&format!("fake_{index:02}")], bidder);
        }
    }

    #[test]
    fn historical_cli_input_retains_its_original_shape_and_values() {
        let current = config_document(&candidate(), 1).unwrap();
        let historical = historical_config_document(&candidate(), 1).unwrap();
        assert_eq!(historical.as_table().unwrap().len(), 2);
        assert!(historical.get("schema_version").is_none());
        assert!(historical["baked"]["host"].as_table().unwrap().is_empty());
        assert!(
            historical["baked"]["account"]
                .as_table()
                .unwrap()
                .is_empty()
        );
        assert_eq!(historical["runtime"].as_table().unwrap().len(), 6);
        assert_eq!(
            historical["runtime"]["auction"],
            current["host"]["resource_limits"]
        );
        assert_eq!(
            historical["runtime"]["status"]["banner"].as_str(),
            Some("q2_a")
        );
        assert_eq!(historical["runtime"]["aliases"], current["host"]["aliases"]);
        assert_eq!(historical["runtime"]["gdpr"], current["host"]["gdpr"]);
        assert_eq!(historical["runtime"]["accounts"], current["accounts"]);
        let historical_bidders = historical["runtime"]["bidders"].as_table().unwrap();
        assert_eq!(historical_bidders.len(), 1);
        for entry in historical_bidders.values() {
            assert_eq!(
                *entry,
                toml::Value::try_from(json!({"timeout_ms": 500_u16, "sample_rate": 1.0_f64}))
                    .unwrap()
            );
        }
    }

    #[test]
    fn structured_toml_uses_the_real_semantic_validator_without_enabling_fixture_metadata() {
        let mut document = config_document(&candidate(), 1).unwrap();
        // Secure-default binaries omit local transport; validate against active HTTPS metadata.
        let bidder_name = auctioneer_config_data::bidders()
            .iter()
            .find(|bidder| {
                bidder.uses_https && !bidder.disabled && bidder.endpoint_macros.is_empty()
            })
            .expect("enabled fixed-endpoint HTTPS bidder")
            .name
            .as_str();
        for (_name, bidder) in document["host"]["aliases"]
            .as_table_mut()
            .unwrap()
            .iter_mut()
        {
            *bidder = toml::Value::String(bidder_name.to_owned());
        }
        document["bidders"] = toml::Value::try_from(
            json!({(bidder_name):{"timeout_ms":500_u16,"sample_rate":1.0_f64}}),
        )
        .unwrap();
        let encoded = toml::to_string(&document).unwrap();
        let source: SdkAuctioneerAppConfig = toml::from_str(&encoded).unwrap();
        source.validate().unwrap();
        document["host"]["resource_limits"]["max_parsed_nodes"] = toml::Value::Integer(0);
        let invalid: SdkAuctioneerAppConfig =
            toml::from_str(&toml::to_string(&document).unwrap()).unwrap();
        let structural = serde_json::to_value(invalid.0.validate().unwrap_err()).unwrap();
        assert_eq!(
            structural["host"]["resource_limits"]["max_parsed_nodes"][0]["code"],
            "range"
        );
        let error = serde_json::to_value(invalid.validate().unwrap_err()).unwrap();
        assert_eq!(
            error,
            json!({"runtime": [{
                "code": "invalid_effective_config", "message": "operator policy structure invalid", "params": {}
            }]})
        );
    }

    #[test]
    fn variants_only_change_the_reviewed_nodes_and_banner_then_restore() {
        let candidate = candidate();
        let first = config_document(&candidate, 1).unwrap();
        let second = config_document(&candidate, 2).unwrap();
        let restored = config_document(&candidate, 3).unwrap();
        assert_eq!(first, restored);
        assert_eq!(
            first["host"]["resource_limits"]["max_parsed_nodes"].as_integer(),
            Some(2048)
        );
        assert_eq!(
            second["host"]["resource_limits"]["max_parsed_nodes"].as_integer(),
            Some(1024)
        );
        assert_eq!(first["host"]["status_response"].as_str(), Some("q2_a"));
        assert_eq!(second["host"]["status_response"].as_str(), Some("q2_b"));
        let mut expected = first;
        expected["host"]["resource_limits"]["max_parsed_nodes"] = toml::Value::Integer(1024);
        expected["host"]["status_response"] = toml::Value::String("q2_b".to_owned());
        assert_eq!(second, expected);
        let mut base_dominates = candidate;
        base_dominates.base_load.nodes = 1536;
        assert_eq!(
            config_document(&base_dominates, 2).unwrap()["host"]["resource_limits"]["max_parsed_nodes"]
                .as_integer(),
            Some(1536)
        );
        for id in [0_u16, 4] {
            assert_eq!(
                config_document(&base_dominates, id).unwrap_err(),
                HarnessError::new("invalid_input")
            );
        }
        base_dominates.maximum_load.calls = 65;
        assert_eq!(
            config_document(&base_dominates, 1).unwrap_err(),
            HarnessError::new("invalid_input")
        );
    }

    #[test]
    fn push_spec_has_exact_frozen_cli_arguments_empty_environment_and_bounded_output() {
        let root = tempfile::tempdir().unwrap();
        let cli = root.path().join("frozen-cli");
        let dir = root.path().join("variant");
        let spec = config_push_spec(&cli, &dir);
        let expected = vec![
            OsString::from("config"),
            OsString::from("push"),
            OsString::from("--adapter"),
            OsString::from("axum"),
            OsString::from("--manifest"),
            dir.join("edgezero.toml").into_os_string(),
            OsString::from("--app-config"),
            dir.join("auctioneer.toml").into_os_string(),
            OsString::from("--local"),
            OsString::from("--yes"),
            OsString::from("--no-env"),
            OsString::from("--no-diff"),
        ];
        assert_eq!(spec.program, cli);
        assert_eq!(spec.args, expected);
        assert_eq!(spec.cwd, Some(dir));
        assert!(spec.env.is_empty());
        assert!(!spec.stdin);
        assert_eq!(spec.output_cap, 32_usize << 10_u32);
        assert_eq!(spec.timeout_ms, 10_000);
    }

    #[test]
    fn actual_config_push_produces_a_readable_qualification_snapshot() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("crates/auctioneer-adapter-axum")).unwrap();
        fs::write(
            root.path().join("edgezero.toml"),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../edgezero.toml")),
        )
        .unwrap();
        fs::write(
            root.path().join("crates/auctioneer-adapter-axum/axum.toml"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../auctioneer-adapter-axum/axum.toml"
            )),
        )
        .unwrap();
        fs::write(root.path().join("auctioneer.toml"), "schema_version = 1\n").unwrap();
        assert!(!root.path().join(".edgezero").exists());
        let spec = config_push_spec(Path::new("auctioneer-cli"), root.path());
        let args =
            Args::try_parse_from(once(OsString::from("auctioneer-cli")).chain(spec.args)).unwrap();
        let Command::Application(command) = args.command else {
            panic!("the qualification push must use the application CLI");
        };
        assert_eq!(application::run(command), 0_i32);
        // Read the actual SDK writer's file independently of SNAPSHOT_FILE.
        let bytes = fs::read(
            root.path()
                .join(".edgezero/local-config-auctioneer_config.json"),
        )
        .unwrap();
        let outer: Value = serde_json::from_slice(&bytes).unwrap();
        let envelope: BlobEnvelope =
            serde_json::from_str(outer["auctioneer_config"].as_str().unwrap()).unwrap();
        envelope.verify().unwrap();
        let snapshot = read_snapshot(root.path()).expect("a real push must feed qualification");
        assert_eq!(
            snapshot.config_sha256,
            Digest::try_from(envelope.sha256).unwrap()
        );
        assert_eq!(snapshot.store_sha256, Digest::of(&bytes));
        corrupt_snapshot(root.path()).unwrap();
        assert_eq!(
            read_snapshot(root.path()).unwrap_err(),
            HarnessError::new("snapshot_invalid")
        );
    }

    #[tokio::test]
    async fn install_rejects_invalid_variants_before_files_or_child_processes() {
        let root = tempfile::tempdir().unwrap();
        let supervisor = Supervisor::new();
        for id in [0_u16, 4] {
            assert_eq!(
                install_variant(
                    Path::new("SECRET_MISSING_CLI"),
                    root.path(),
                    root.path(),
                    &candidate(),
                    id,
                    &supervisor
                )
                .await
                .unwrap_err(),
                HarnessError::new("invalid_input")
            );
        }
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
        supervisor.cleanup().await.unwrap();
    }

    #[test]
    fn snapshot_binds_the_typed_inner_digest_and_exact_outer_file_bytes() {
        let root = tempfile::tempdir().unwrap();
        let outer = valid_store();
        let file = snapshot_file(root.path(), &outer);
        let inner: BlobEnvelope =
            serde_json::from_str(outer["auctioneer_config"].as_str().unwrap()).unwrap();
        let before = read_snapshot(root.path()).unwrap();
        assert_eq!(
            before.config_sha256,
            Digest::try_from(inner.sha256).unwrap()
        );
        assert_eq!(before.store_sha256, Digest::of(&fs::read(&file).unwrap()));
        let mut raw = fs::read(&file).unwrap();
        raw.push(b'\n');
        fs::write(&file, raw).unwrap();
        let after = read_snapshot(root.path()).unwrap();
        assert_eq!(before.config_sha256, after.config_sha256);
        assert_ne!(before.store_sha256, after.store_sha256);
    }

    #[test]
    fn snapshot_rejects_missing_malformed_nonstring_and_duplicate_envelope_digests() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(
            read_snapshot(root.path()).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        for outer in [
            json!({}),
            json!({"auctioneer_config": {"sha256": "a".repeat(64)}}),
            json!({"auctioneer_config": "{"}),
        ] {
            snapshot_file(root.path(), &outer);
            assert_eq!(
                read_snapshot(root.path()).unwrap_err(),
                HarnessError::new("snapshot_invalid")
            );
        }
        for inner in [
            json!({"sha256": "A".repeat(64)}),
            json!({"sha256": "a".repeat(63)}),
            json!({"sha256": "SECRET_CANARY"}),
            json!({"sha256": 1_u8}),
            json!({"data": {}}),
        ] {
            snapshot_file(
                root.path(),
                &json!({"auctioneer_config": serde_json::to_string(&inner).unwrap()}),
            );
            let error = read_snapshot(root.path()).unwrap_err();
            assert_eq!(error, HarnessError::new("snapshot_invalid"));
            assert!(!format!("{error:?}").contains("SECRET_CANARY"));
        }
        let duplicate = format!(
            "{{\"sha256\":\"{}\",\"sha256\":\"{}\"}}",
            "a".repeat(64),
            "b".repeat(64)
        );
        snapshot_file(root.path(), &json!({"auctioneer_config": duplicate}));
        assert_eq!(
            read_snapshot(root.path()).unwrap_err(),
            HarnessError::new("snapshot_invalid")
        );
        let outer = serde_json::to_string(&valid_store()).unwrap();
        fs::write(
            root.path().join(SNAPSHOT_FILE),
            outer.replacen(
                "\"auctioneer_config\":",
                "\"auctioneer_config\":\"{}\",\"auctioneer_config\":",
                1,
            ),
        )
        .unwrap();
        assert_eq!(
            read_snapshot(root.path()).unwrap_err(),
            HarnessError::new("snapshot_invalid")
        );
    }

    #[test]
    fn snapshot_byte_node_and_depth_limits_include_ignored_inner_members() {
        let root = tempfile::tempdir().unwrap();
        let file = snapshot_file(root.path(), &valid_store());
        let mut bytes = fs::read(&file).unwrap();
        bytes.resize(SNAPSHOT_CAP, b' ');
        fs::write(&file, &bytes).unwrap();
        read_snapshot(root.path()).unwrap();
        bytes.push(b' ');
        fs::write(&file, bytes).unwrap();
        assert_eq!(
            read_snapshot(root.path()).unwrap_err(),
            HarnessError::new("input_limit")
        );
        let deep = format!(
            "{{\"sha256\":\"{}\",\"discarded\":{}0{}}}",
            "a".repeat(64),
            "[".repeat(129),
            "]".repeat(129)
        );
        snapshot_file(root.path(), &json!({"auctioneer_config": deep}));
        assert_eq!(
            read_snapshot(root.path()).unwrap_err(),
            HarnessError::new("snapshot_invalid")
        );
        let excessive = json!({"sha256": "a".repeat(64), "discarded": vec![0_u8; 1 << 16_u32]});
        snapshot_file(
            root.path(),
            &json!({"auctioneer_config": serde_json::to_string(&excessive).unwrap()}),
        );
        assert_eq!(
            read_snapshot(root.path()).unwrap_err(),
            HarnessError::new("snapshot_invalid")
        );
    }

    #[test]
    fn corruption_preserves_the_outer_store_and_other_values_but_breaks_the_inner_json() {
        let root = tempfile::tempdir().unwrap();
        let file = snapshot_file(root.path(), &valid_store());
        read_snapshot(root.path()).unwrap();
        corrupt_snapshot(root.path()).unwrap();
        let outer: Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
        assert_eq!(outer["auctioneer_config"], "{");
        assert_eq!(outer["other_store"], "PRESERVE_OTHER_STORE");
        assert_eq!(outer.as_object().unwrap().len(), 2);
        assert_eq!(
            read_snapshot(root.path()).unwrap_err(),
            HarnessError::new("snapshot_invalid")
        );
    }

    #[test]
    fn corruption_refuses_invalid_outer_json_without_rewriting_it() {
        let root = tempfile::tempdir().unwrap();
        let file = snapshot_file(root.path(), &json!({"auctioneer_config": 1_u8}));
        let original = fs::read(&file).unwrap();
        assert_eq!(
            corrupt_snapshot(root.path()).unwrap_err(),
            HarnessError::new("snapshot_invalid")
        );
        assert_eq!(fs::read(&file).unwrap(), original);
        fs::write(&file, b"SECRET_BAD_OUTER").unwrap();
        assert_eq!(
            corrupt_snapshot(root.path()).unwrap_err(),
            HarnessError::new("snapshot_invalid")
        );
        assert_eq!(fs::read(&file).unwrap(), b"SECRET_BAD_OUTER");
    }

    #[test]
    fn frozen_artifacts_are_private_independent_and_detect_source_and_copy_tampering() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("frozen");
        fs::write(&source, b"artifact A").unwrap();
        let digest = freeze_artifact(&source, &destination).unwrap();
        assert_eq!(digest, Digest::of(b"artifact A"));
        assert_eq!(fs::read(&destination).unwrap(), b"artifact A");
        assert_artifact(&source, &digest).unwrap();
        assert_artifact(&destination, &digest).unwrap();
        fs::write(&source, b"artifact B").unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"artifact A");
        assert_eq!(
            assert_artifact(&source, &digest).unwrap_err(),
            HarnessError::new("identity_mismatch")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                fs::metadata(&destination).unwrap().permissions().mode() & 0o777,
                0o500
            );
            fs::set_permissions(&destination, fs::Permissions::from_mode(0o600)).unwrap();
        };
        fs::write(&destination, b"artifact C").unwrap();
        assert_eq!(
            assert_artifact(&destination, &digest).unwrap_err(),
            HarnessError::new("identity_mismatch")
        );
    }

    #[test]
    fn freeze_refuses_existing_missing_nonregular_and_over_native_cap_inputs() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("destination");
        fs::write(&source, b"artifact").unwrap();
        fs::write(&destination, b"DO_NOT_REPLACE").unwrap();
        assert_eq!(
            freeze_artifact(&source, &destination).unwrap_err(),
            HarnessError::new("output_io")
        );
        assert_eq!(fs::read(&destination).unwrap(), b"DO_NOT_REPLACE");
        let missing = root.path().join("SECRET_MISSING");
        let input_error = HarnessError::new(if cfg!(unix) {
            "invalid_input"
        } else {
            "input_io"
        });
        assert_eq!(
            assert_artifact(&missing, &Digest::of(b"artifact")).unwrap_err(),
            input_error
        );
        assert_eq!(
            freeze_artifact(&missing, &root.path().join("missing-copy")).unwrap_err(),
            input_error
        );
        assert_eq!(
            freeze_artifact(root.path(), &root.path().join("directory-copy")).unwrap_err(),
            input_error
        );
        assert_eq!(ARTIFACT_CAP, 512_usize << 20_u32);
        let oversized = root.path().join("oversized");
        let file = fs::File::create(&oversized).unwrap();
        file.set_len(u64::try_from(ARTIFACT_CAP).unwrap() + 1)
            .unwrap();
        drop(file);
        let copy = root.path().join("oversized-copy");
        assert_eq!(
            freeze_artifact(&oversized, &copy).unwrap_err(),
            HarnessError::new("input_limit")
        );
        assert!(!copy.exists());
    }

    #[cfg(unix)]
    #[test]
    fn strict_artifacts_and_snapshots_refuse_symlinks_and_hardlinks() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        fs::write(&source, b"artifact").unwrap();
        let soft = root.path().join("soft");
        symlink(&source, &soft).unwrap();
        assert_eq!(
            freeze_artifact(&soft, &root.path().join("soft-copy")).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        assert_eq!(
            assert_artifact(&soft, &Digest::of(b"artifact")).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        assert_eq!(
            freeze_artifact(&source, &soft).unwrap_err(),
            HarnessError::new("output_io")
        );
        assert_eq!(fs::read(&source).unwrap(), b"artifact");
        let hard = root.path().join("hard");
        fs::hard_link(&source, &hard).unwrap();
        assert_eq!(
            freeze_artifact(&source, &root.path().join("hard-copy")).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        assert_eq!(
            assert_artifact(&hard, &Digest::of(b"artifact")).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        let file = snapshot_file(root.path(), &valid_store());
        let saved = root.path().join("saved-store");
        fs::rename(&file, &saved).unwrap();
        symlink(&saved, &file).unwrap();
        assert_eq!(
            read_snapshot(root.path()).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        assert_eq!(
            corrupt_snapshot(root.path()).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        fs::remove_file(&file).unwrap();
        fs::hard_link(&saved, &file).unwrap();
        assert_eq!(
            read_snapshot(root.path()).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        assert_eq!(
            corrupt_snapshot(root.path()).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        assert_eq!(
            fs::read(&saved).unwrap(),
            serde_json::to_vec(&valid_store()).unwrap()
        );
    }
}
