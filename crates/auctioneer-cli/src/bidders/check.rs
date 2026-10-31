use super::source::GitSource;
use super::{Error, inventory, wave};
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;
use std::fs::{OpenOptions, symlink_metadata};
use std::io::Read as _;
use std::path::Path;
use std::str::from_utf8;

#[cfg(unix)]
use rustix::fs::OFlags;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt as _;

const MAX_INVENTORY_BYTES: u64 = 0x0010_0000;
const FACTORY_CAPTURE_SHA256: &str =
    "d1a0f18d99f53cbd4f62fe74c5d1e5580f8688fce95a29b1b05ee934dbe9ff0c";

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    ReferenceValid,
    ReferenceDrift,
    MigrationIncomplete,
    WaveDesignValid,
    WaveSourceDrift,
}

#[derive(Serialize)]
pub(super) struct Report {
    scope: &'static str,
    status: Status,
    identities: usize,
    migrated: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    wave: Option<WaveSummary>,
}

#[derive(Serialize)]
struct WaveSummary {
    id: String,
    sha256: String,
    review_status: &'static str,
    operations: usize,
    drivers: usize,
    promoted: usize,
}

impl Report {
    pub const fn exit_code(&self) -> i32 {
        match self.status {
            Status::ReferenceValid | Status::WaveDesignValid => 0,
            Status::ReferenceDrift | Status::MigrationIncomplete | Status::WaveSourceDrift => 1,
        }
    }

    const fn new(status: Status, identities: usize) -> Self {
        Self {
            scope: "reference_inventory_only",
            status,
            identities,
            migrated: 0,
            wave: None,
        }
    }
}

#[cfg(test)]
fn read_inventory(path: &Path) -> Result<toml::Value, Error> {
    parse_inventory(&read_input(path)?)
}

pub(super) fn read_input(path: &Path) -> Result<Vec<u8>, Error> {
    let metadata = symlink_metadata(path).map_err(|_error| Error::InventoryInput)?;
    if !metadata.is_file() || metadata.len() > MAX_INVENTORY_BYTES {
        return Err(Error::InventoryInput);
    }
    read_opened_input(path)
}

// Revalidate the opened object: the path may change after the preflight.
fn read_opened_input(path: &Path) -> Result<Vec<u8>, Error> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(
        (OFlags::NOFOLLOW | OFlags::NONBLOCK)
            .bits()
            .try_into()
            .map_err(|_error| Error::InventoryInput)?,
    );
    let file = options.open(path).map_err(|_error| Error::InventoryInput)?;
    if !file
        .metadata()
        .map_err(|_error| Error::InventoryInput)?
        .is_file()
    {
        return Err(Error::InventoryInput);
    }
    let mut bytes = Vec::new();
    file.take(MAX_INVENTORY_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_error| Error::InventoryInput)?;
    if u64::try_from(bytes.len()).map_err(|_error| Error::InventoryInput)? > MAX_INVENTORY_BYTES {
        return Err(Error::InventoryInput);
    }
    Ok(bytes)
}

fn parse_inventory(bytes: &[u8]) -> Result<toml::Value, Error> {
    let text = from_utf8(bytes).map_err(|_error| Error::InventoryInput)?;
    toml::from_str(text).map_err(|_error| Error::InventoryInput)
}

pub(super) fn parse_factory_bindings(bytes: &[u8]) -> Result<toml::Value, Error> {
    if format!("{:x}", Sha256::digest(bytes)) != FACTORY_CAPTURE_SHA256 {
        return Err(Error::InvalidSource);
    }
    parse_inventory(bytes).map_err(|_error| Error::InvalidSource)
}

#[cfg(test)]
fn read_opened_inventory(path: &Path) -> Result<toml::Value, Error> {
    parse_inventory(&read_opened_input(path)?)
}

pub(super) async fn run(
    source: &Path,
    input: &Path,
    require_complete: bool,
    wave_path: Option<&Path>,
    factory_bindings: Option<&Path>,
) -> Result<Report, Error> {
    let bytes = read_input(input)?;
    run_bytes(
        source,
        &bytes,
        require_complete,
        wave_path,
        factory_bindings,
    )
    .await
}

pub(super) async fn run_bytes(
    source: &Path,
    inventory_bytes: &[u8],
    require_complete: bool,
    wave_path: Option<&Path>,
    factory_bindings: Option<&Path>,
) -> Result<Report, Error> {
    let supplied = parse_inventory(inventory_bytes)?;
    let expected = inventory::inspect(source).await?;
    let identities = expected.identity_count();
    let expected_value = toml::Value::try_from(expected).map_err(|_error| Error::Output)?;
    if supplied != expected_value {
        return Ok(Report::new(Status::ReferenceDrift, 0));
    }
    if let Some(path) = wave_path {
        let bytes = read_input(path).map_err(|_error| Error::WaveInput)?;
        let design = wave::parse(&bytes)?;
        let bindings_path = factory_bindings.unwrap_or(Path::new(
            "crates/auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-bindings.toml",
        ));
        let binding_bytes = read_input(bindings_path).map_err(|_error| Error::WaveInput)?;
        // A pending-review reference input, not a dependency on test source or
        // permission to install these factories in the serving application.
        let bindings = parse_factory_bindings(&binding_bytes).map_err(|_error| Error::WaveInput)?;
        let mut source_tree = GitSource::open(source).await?;
        let entries = source_tree.entries("adapters").await?;
        let matches = wave_matches_inventory(&design, &expected_value, &bindings)
            && design.bidders.iter().all(|bidder| {
                let Some(directory) = bindings
                    .get("bindings")
                    .and_then(|rows| rows.get(&bidder.base))
                    .and_then(toml::Value::as_str)
                else {
                    return false;
                };
                let prefix = format!("{directory}/");
                let actual: BTreeSet<_> = entries
                    .iter()
                    .filter(|entry| {
                        entry.path.strip_prefix(&prefix).is_some_and(|suffix| {
                            !suffix.contains('/')
                                && Path::new(suffix).extension().is_some_and(|ext| ext == "go")
                        })
                    })
                    .map(|entry| (entry.path.as_str(), entry.oid.as_str()))
                    .collect();
                let declared: BTreeSet<_> = bidder
                    .source_files
                    .iter()
                    .map(|file| (file.path.as_str(), file.blob.as_str()))
                    .collect();
                !actual.is_empty() && actual == declared
            });
        let status = if !matches {
            Status::WaveSourceDrift
        } else if require_complete {
            Status::MigrationIncomplete
        } else {
            Status::WaveDesignValid
        };
        let operations = design.operation_count();
        let drivers = design
            .bidders
            .iter()
            .flat_map(|bidder| &bidder.operations)
            .map(|operation| operation.driver.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        let mut report = Report::new(status, design.bidders.len());
        report.scope = "wave_design_only";
        report.wave = Some(WaveSummary {
            id: design.id,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            review_status: "pending",
            operations,
            drivers,
            promoted: 0,
        });
        return Ok(report);
    }
    // Inventory integrity is not producer evidence or protocol review.
    let status = if require_complete {
        Status::MigrationIncomplete
    } else {
        Status::ReferenceValid
    };
    Ok(Report::new(status, identities))
}

fn wave_matches_inventory(
    design: &wave::Wave,
    inventory: &toml::Value,
    bindings: &toml::Value,
) -> bool {
    let Some(identities) = inventory.get("identity").and_then(toml::Value::as_array) else {
        return false;
    };
    let Some(schemas) = inventory.get("schema").and_then(toml::Value::as_array) else {
        return false;
    };
    let Some(implementations) = inventory
        .get("implementations")
        .and_then(toml::Value::as_array)
    else {
        return false;
    };
    design.bidders.iter().all(|bidder| {
        let Some(identity) = identities
            .iter()
            .find(|row| row["name"].as_str() == Some(&bidder.name))
        else {
            return false;
        };
        let base = identity
            .get("alias_of")
            .and_then(toml::Value::as_str)
            .unwrap_or(&bidder.name);
        let Some(base_identity) = identities
            .iter()
            .find(|row| row["name"].as_str() == Some(base))
        else {
            return false;
        };
        let endpoint = identity
            .get("endpoint")
            .or_else(|| base_identity.get("endpoint"))
            .and_then(toml::Value::as_str)
            .unwrap_or("");
        let routing = if endpoint.starts_with("http://") {
            "https_default_pending"
        } else if endpoint.contains("REGION")
            && base_identity["metadata_fields"]
                .as_array()
                .is_some_and(|fields| fields.iter().any(|field| field.as_str() == Some("xapi")))
        {
            "regional_credentials_required"
        } else {
            "https_review_pending"
        };
        let Some(directory) = bindings["bindings"].get(base).and_then(toml::Value::as_str) else {
            return false;
        };
        identity["blob"].as_str() == Some(&bidder.metadata_blob)
            && base == bidder.base
            && bidder.routing == routing
            && identity
                .get("sync")
                .and_then(|sync| sync.get("origin"))
                .and_then(toml::Value::as_str)
                == Some(&bidder.sync)
            && schemas.iter().any(|row| {
                row["name"].as_str() == Some(base)
                    && row["blob"].as_str() == Some(&bidder.schema_blob)
            })
            && implementations.iter().any(|row| {
                row["internal_key"].as_str() == Some(&bidder.implementation)
                    && row["source_directory"].as_str() == Some(directory)
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn inventory_input_limit_accepts_the_exact_boundary_and_refuses_one_extra_byte() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("inventory.toml");
        let mut bytes = format!("#{}", "x".repeat(0x000f_ffff));
        fs::write(&path, &bytes).unwrap();
        assert!(
            read_inventory(&path)
                .unwrap()
                .as_table()
                .unwrap()
                .is_empty()
        );
        bytes.push('x');
        fs::write(&path, &bytes).unwrap();
        assert_eq!(read_inventory(&path), Err(Error::InventoryInput));
        assert_eq!(read_opened_inventory(&path), Err(Error::InventoryInput));
        assert_eq!(read_inventory(directory.path()), Err(Error::InventoryInput));
        assert_eq!(
            read_inventory(&directory.path().join("missing")),
            Err(Error::InventoryInput)
        );
    }

    #[cfg(unix)]
    #[test]
    fn inventory_input_never_follows_a_symlink() {
        use std::os::unix::fs::symlink;
        let directory = tempdir().unwrap();
        let target = directory.path().join("target.toml");
        let link = directory.path().join("link.toml");
        fs::write(&target, "revision = 'fixture'\n").unwrap();
        symlink(target, &link).unwrap();
        assert_eq!(read_inventory(&link), Err(Error::InventoryInput));
        assert_eq!(read_opened_inventory(&link), Err(Error::InventoryInput));
    }

    #[cfg(unix)]
    #[test]
    fn inventory_replaced_by_a_fifo_is_refused_without_waiting_for_a_writer() {
        use std::env::{current_exe, var_os};
        use std::io::{Write as _, stdout};
        use std::process::{Command, Stdio};
        use std::thread::sleep;
        use std::time::{Duration, Instant};

        const CHILD_PATH: &str = "AUCTIONEER__TEST__INVENTORY_FIFO_CHILD";
        if let Some(path) = var_os(CHILD_PATH) {
            assert_eq!(
                read_opened_inventory(Path::new(&path)),
                Err(Error::InventoryInput)
            );
            stdout()
                .lock()
                .write_all(b"\ninventory_fifo_refused\n")
                .unwrap();
            return;
        }
        let directory = tempdir().unwrap();
        let path = directory.path().join("inventory.toml");
        fs::write(&path, "").unwrap();
        assert!(fs::symlink_metadata(&path).unwrap().is_file());
        fs::remove_file(&path).unwrap();
        assert!(
            Command::new("/usr/bin/mkfifo")
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        let mut child = Command::new(current_exe().unwrap())
            .args([
                "--exact",
                "bidders::check::tests::inventory_replaced_by_a_fifo_is_refused_without_waiting_for_a_writer",
                "--nocapture",
            ])
            .env(CHILD_PATH, &path)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "FIFO refusal child failed: {status}");
                let mut output = String::new();
                child
                    .stdout
                    .take()
                    .unwrap()
                    .read_to_string(&mut output)
                    .unwrap();
                // A stale --exact filter exits 0 without running anything.
                assert!(
                    output.lines().any(|line| line == "inventory_fifo_refused"),
                    "{output}"
                );
                break;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("opening a replaced inventory blocked on a FIFO");
            }
            sleep(Duration::from_millis(10));
        }
    }
}
