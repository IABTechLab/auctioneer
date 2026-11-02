//! Fixed source and controlled-native producers. Receipts are local records, not
//! authentication against concurrent hostile filesystem mutation.

mod conformance;
mod files;
mod native;

use super::{Error, check, source::REVISION};
use auctioneer_cli::native_control::io::{canonical_digest, hash_file, parse_json, sha256};
use auctioneer_cli::{BuildIdentity, Snapshot, snapshot};
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use std::env::{current_exe, var_os};
use std::fs;
use std::path::{Path, PathBuf};

const CAP: usize = 1 << 20;
const VERSION: u8 = 3;
const PRODUCER: &str = "reference_integrity";
const OPERATION: &str = "inspect_reference_inventory";
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const BUILT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/resource-identity.json"));
const UNAVAILABLE: [&str; 8] = [
    "foundation",
    "schema_qualification",
    "registry_lifecycle",
    "proprietary_decoder",
    "resource_qualification",
    "current_shipped_subset",
    "priority_contracts",
    "composed_onboarding",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Failure {
    Input,
    Binding,
    RunMismatch,
    Membership,
    Output,
    Producer,
    OperationFailed,
}

impl Failure {
    pub(super) const fn code(self) -> &'static str {
        match self {
            Self::Input => "bidder_evidence_input",
            Self::Binding => "bidder_evidence_binding",
            Self::RunMismatch => "bidder_evidence_run_mismatch",
            Self::Membership => "bidder_evidence_membership",
            Self::Output => "bidder_evidence_output",
            Self::Producer => "bidder_evidence_producer",
            Self::OperationFailed => "bidder_evidence_operation_failed",
        }
    }
}

impl From<Failure> for Error {
    fn from(value: Failure) -> Self {
        Self::Evidence(value)
    }
}

#[derive(Debug, Args)]
pub(crate) struct Inputs {
    #[arg(long)]
    source: Option<PathBuf>,
    #[arg(
        long,
        default_value = "crates/auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-inventory.toml"
    )]
    inventory: PathBuf,
    #[arg(long)]
    run_id: String,
}

impl Inputs {
    fn source(&self) -> Result<PathBuf, Error> {
        self.source
            .clone()
            .or_else(|| var_os("AUCTIONEER__BIDDER__SOURCE_DIR").map(PathBuf::from))
            .ok_or(Failure::Input.into())
    }
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    Begin {
        #[command(flatten)]
        inputs: Inputs,
        #[arg(long)]
        output: PathBuf,
    },
    Produce {
        #[command(flatten)]
        inputs: Inputs,
        #[arg(long)]
        run_manifest: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        producer: String,
    },
    Run {
        #[command(flatten)]
        inputs: Inputs,
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Producer {
    id: String,
    operation: String,
    operation_count: u8,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Requirements {
    producers: Vec<Producer>,
    unavailable: Vec<String>,
}

fn requirements() -> Requirements {
    Requirements {
        producers: vec![
            Producer {
                id: PRODUCER.to_owned(),
                operation: OPERATION.to_owned(),
                operation_count: 1,
            },
            conformance::requirement(),
            native::requirement(),
        ],
        unavailable: UNAVAILABLE.into_iter().map(str::to_owned).collect(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    build_identity_sha256: String,
    executable_sha256: String,
    workspace_sha256: String,
    lock_sha256: String,
    features: Vec<String>,
    source_revision: String,
    inventory_sha256: String,
    evidence_target_directory: PathBuf,
    evidence_tool_environment_sha256: String,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u8,
    scope: String,
    run_id: String,
    requirements: Requirements,
    inputs: Binding,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    Completed,
    Failed,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema_version: u8,
    run_id: String,
    manifest_sha256: String,
    producer: String,
    operation: String,
    operation_count: u8,
    outcome: Outcome,
    error: Option<String>,
}

struct Context {
    inventory: Vec<u8>,
    binding: Binding,
}

fn valid_id(id: &str) -> Result<(), Error> {
    if id.is_empty()
        || id.len() > 256
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        || id == "."
        || id == ".."
    {
        return Err(Failure::Input.into());
    }
    Ok(())
}

fn current(inventory: &Path) -> Result<Context, Error> {
    let built: BuildIdentity = parse_json(BUILT, CAP).map_err(|_error| Failure::Binding)?;
    let root = fs::canonicalize(ROOT).map_err(|_error| Failure::Binding)?;
    let workspace = validate_current_source(&root, &built)?;
    let inventory_bytes = files::read(inventory)?;
    let lock = files::read(&root.join("Cargo.lock"))?;
    let lock_sha256 = sha256(&lock);
    if built
        .snapshot
        .files
        .get("Cargo.lock")
        .and_then(Option::as_ref)
        != Some(&lock_sha256)
    {
        return Err(Failure::Binding.into());
    }
    let mut features = vec!["application".to_owned()];
    features.extend(
        auctioneer_adapter_axum::build_features()
            .into_iter()
            .map(str::to_owned),
    );
    if cfg!(feature = "test-support") {
        features.push("test-support".to_owned());
    }
    let (evidence_target_directory, evidence_tool_environment_sha256) =
        conformance::build_binding(&root)?;
    let binding = Binding {
        build_identity_sha256: sha256(BUILT),
        // Cargo may hard-link the running binary into its output directory.
        // Supplied evidence keeps the stricter file_digest link policy.
        executable_sha256: hash_file(
            &current_exe().map_err(|_error| Failure::Binding)?,
            512 << 20,
        )
        .map_err(|_error| Failure::Binding)?,
        workspace_sha256: workspace.sha256,
        lock_sha256,
        features,
        source_revision: REVISION.to_owned(),
        inventory_sha256: sha256(&inventory_bytes),
        evidence_target_directory,
        evidence_tool_environment_sha256,
    };
    Ok(Context {
        inventory: inventory_bytes,
        binding,
    })
}

fn validate_current_source(root: &Path, built: &BuildIdentity) -> Result<Snapshot, Error> {
    let workspace = snapshot(root).map_err(|_error| Failure::Binding)?;
    if workspace != built.snapshot {
        return Err(Failure::Binding.into());
    }
    Ok(workspace)
}

fn expected(inventory: &Path, id: &str) -> Result<(Manifest, Context), Error> {
    valid_id(id)?;
    let context = current(inventory)?;
    let manifest = Manifest {
        schema_version: VERSION,
        scope: "controlled_native_acceptance".to_owned(),
        run_id: id.to_owned(),
        requirements: requirements(),
        inputs: context.binding.clone(),
    };
    Ok((manifest, context))
}

fn load(inventory: &Path, root: &Path, id: &str) -> Result<(Manifest, Context), Error> {
    let bytes = files::read(&root.join("manifest.json"))?;
    let manifest: Manifest = parse_json(&bytes, CAP).map_err(|_error| Failure::Input)?;
    if manifest.run_id != id {
        return Err(Failure::RunMismatch.into());
    }
    let (required, context) = expected(inventory, id)?;
    if manifest.requirements != required.requirements {
        return Err(Failure::Membership.into());
    }
    if manifest != required {
        return Err(Failure::Binding.into());
    }
    Ok((manifest, context))
}

fn begin(inventory: &Path, root: &Path, id: &str) -> Result<(), Error> {
    let (manifest, _context) = expected(inventory, id)?;
    files::fresh_dir(root)?;
    files::publish(&root.join("manifest.json"), &manifest)
}

async fn produce(source: &Path, inventory: &Path, root: &Path, id: &str) -> Result<(), Error> {
    let (manifest, context) = load(inventory, root, id)?;
    run_members(root, &["manifest.json"])?;
    let slot = root.join(PRODUCER);
    files::fresh_dir(&slot)?;
    // The exclusive slot is claimed before work; any interruption remains incomplete.
    let result = check::run_bytes(source, &context.inventory, false, None, None).await;
    let success = result.is_ok_and(|report| report.exit_code() == 0_i32)
        && current(inventory).is_ok_and(|after| after.binding == context.binding);
    run_members(root, &["manifest.json", PRODUCER])?;
    files::members(&slot, &[])?;
    let (still_manifest, _context) = load(inventory, root, id)?;
    if manifest != still_manifest {
        return Err(Failure::Binding.into());
    }
    let receipt = Receipt {
        schema_version: VERSION,
        run_id: id.to_owned(),
        manifest_sha256: canonical_digest(&manifest).map_err(|_error| Failure::Input)?,
        producer: PRODUCER.to_owned(),
        operation: OPERATION.to_owned(),
        operation_count: 1,
        outcome: if success {
            Outcome::Completed
        } else {
            Outcome::Failed
        },
        error: (!success).then(|| "reference_integrity_failed".to_owned()),
    };
    files::publish(&slot.join("receipt.json"), &receipt)?;
    if !success {
        return Err(Failure::OperationFailed.into());
    }
    Ok(())
}

async fn validate(source: &Path, inventory: &Path, root: &Path, id: &str) -> Result<(), Error> {
    let (manifest, context) = load(inventory, root, id)?;
    run_members(root, &["manifest.json", PRODUCER])?;
    let slot = root.join(PRODUCER);
    files::members(&slot, &["receipt.json"])?;
    let receipt: Receipt = parse_json(&files::read(&slot.join("receipt.json"))?, CAP)
        .map_err(|_error| Failure::Input)?;
    if receipt.run_id != id {
        return Err(Failure::RunMismatch.into());
    }
    if receipt.schema_version != VERSION
        || receipt.producer != PRODUCER
        || receipt.operation != OPERATION
        || receipt.operation_count != 1
    {
        return Err(Failure::Membership.into());
    }
    if receipt.manifest_sha256 != canonical_digest(&manifest).map_err(|_error| Failure::Input)? {
        return Err(Failure::Binding.into());
    }
    if receipt.outcome != Outcome::Completed || receipt.error.is_some() {
        return Err(Failure::OperationFailed.into());
    }
    // Do not let a fabricated success receipt hide current reference drift.
    let report = check::run_bytes(source, &context.inventory, false, None, None).await?;
    if report.exit_code() != 0_i32 {
        return Err(Failure::OperationFailed.into());
    }
    if current(inventory)?.binding != context.binding {
        return Err(Failure::Binding.into());
    }
    Ok(())
}

fn run_members(root: &Path, required: &[&str]) -> Result<(), Error> {
    // Distributed producers can claim fixed slots in either order. Each
    // consumer independently requires its own slot and exact operation set.
    files::subset(
        root,
        &[
            "manifest.json",
            PRODUCER,
            conformance::PRODUCER,
            native::PRODUCER,
        ],
        required,
    )
}

#[derive(Serialize)]
struct Preparation {
    status: &'static str,
    error: Option<&'static str>,
}

#[derive(Serialize)]
struct Report {
    schema_version: u8,
    scope: &'static str,
    status: &'static str,
    migrated: u8,
    preparation: Preparation,
    conformance: conformance::Report,
    native: native::Report,
    full_catalog_unavailable: [&'static str; 8],
    full_catalog_complete: bool,
    formal_qualified: bool,
    wasm_ready: bool,
}

pub(super) async fn consume(
    source: &Path,
    inventory: &Path,
    root: &Path,
    id: &str,
) -> Result<i32, Error> {
    let error = validate(source, inventory, root, id).await.err();
    let conformance = conformance::consume(source, inventory, root, id).await;
    let native = native::consume(source, inventory, root, id).await;
    let complete = error.is_none() && conformance.complete() && native.complete();
    let report = Report {
        schema_version: VERSION,
        scope: "controlled_native_acceptance",
        status: if complete {
            "native_acceptance_complete"
        } else {
            "native_acceptance_incomplete"
        },
        migrated: if complete { 14 } else { 0 },
        preparation: Preparation {
            status: if error.is_none() {
                "complete"
            } else {
                "incomplete"
            },
            error: error.map(Error::code),
        },
        conformance,
        native,
        full_catalog_unavailable: UNAVAILABLE,
        full_catalog_complete: false,
        formal_qualified: false,
        wasm_ready: false,
    };
    let mut bytes = serde_json::to_vec(&report).map_err(|_error| Error::Output)?;
    bytes.push(b'\n');
    super::write_output(&bytes)?;
    Ok(i32::from(!complete))
}

pub(super) async fn run(command: Command) -> Result<i32, Error> {
    match command {
        Command::Begin { inputs, output } => {
            let _source = inputs.source()?;
            begin(&inputs.inventory, &output, &inputs.run_id)?;
            Ok(0_i32)
        }
        Command::Produce {
            inputs,
            run_manifest,
            output,
            producer,
        } => {
            if producer != PRODUCER
                && producer != conformance::PRODUCER
                && producer != native::PRODUCER
            {
                return Err(Failure::Producer.into());
            }
            let root = run_manifest.parent().ok_or(Failure::Input)?;
            if run_manifest.file_name().and_then(|name| name.to_str()) != Some("manifest.json")
                || output != root.join(&producer)
            {
                return Err(Failure::Output.into());
            }
            let source = inputs.source()?;
            if producer == PRODUCER {
                produce(&source, &inputs.inventory, root, &inputs.run_id).await?;
            } else if producer == conformance::PRODUCER {
                conformance::produce(&source, &inputs.inventory, root, &inputs.run_id).await?;
            } else {
                native::produce(&source, &inputs.inventory, root, &inputs.run_id).await?;
            }
            Ok(0_i32)
        }
        Command::Run { inputs, output } => {
            let source = inputs.source()?;
            begin(&inputs.inventory, &output, &inputs.run_id)?;
            produce(&source, &inputs.inventory, &output, &inputs.run_id).await?;
            conformance::produce(&source, &inputs.inventory, &output, &inputs.run_id).await?;
            native::produce(&source, &inputs.inventory, &output, &inputs.run_id).await?;
            consume(&source, &inputs.inventory, &output, &inputs.run_id).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use std::process::Command as ProcessCommand;
    use tempfile::{TempDir, tempdir_in};

    fn source_fixture() -> (TempDir, BuildIdentity) {
        let directory = tempdir_in(temp_dir().canonicalize().unwrap()).unwrap();
        let root = directory.path();
        fs::write(root.join("owner.rs"), "fn owner() {}\n").unwrap();
        fs::write(root.join("Cargo.lock"), "fixture lock\n").unwrap();
        for args in [
            &["init", "--quiet"][..],
            &["add", "owner.rs", "Cargo.lock"],
            &[
                "-c",
                "user.name=Evidence Test",
                "-c",
                "user.email=evidence@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "fixture",
            ],
        ] {
            let output = ProcessCommand::new("git")
                .current_dir(root)
                .args(args)
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
        }
        let built = BuildIdentity {
            snapshot: snapshot(root).unwrap(),
            compiler_verbose: "fixture compiler".to_owned(),
            compiled_target: "fixture target".to_owned(),
            code_base_commit: "0".repeat(40),
            source_tree_matches_commit: false,
        };
        (directory, built)
    }

    #[test]
    fn sourceguard_compares_current_fixture_bytes_with_the_supplied_build_snapshot() {
        let (directory, built) = source_fixture();
        let root = directory.path();
        assert_eq!(
            validate_current_source(root, &built).unwrap(),
            built.snapshot
        );
        for (name, original) in [
            ("owner.rs", "fn owner() {}\n"),
            ("Cargo.lock", "fixture lock\n"),
        ] {
            fs::write(root.join(name), "changed bytes\n").unwrap();
            assert_eq!(
                validate_current_source(root, &built).unwrap_err(),
                Error::Evidence(Failure::Binding)
            );
            fs::write(root.join(name), original).unwrap();
            assert_eq!(
                validate_current_source(root, &built).unwrap(),
                built.snapshot
            );
        }
        let untracked = root.join("new.rs");
        fs::write(&untracked, "fn added() {}\n").unwrap();
        assert_eq!(
            validate_current_source(root, &built).unwrap_err(),
            Error::Evidence(Failure::Binding)
        );
        fs::remove_file(untracked).unwrap();
        assert_eq!(
            validate_current_source(root, &built).unwrap(),
            built.snapshot
        );
    }
}
