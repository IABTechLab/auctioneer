//! Offline reference inspection. Inventory is not runtime promotion or migration evidence.

use auctioneer_config::bidder_manifest;
mod check;
#[cfg(feature = "application")]
mod empty_object;
#[cfg(feature = "application")]
mod evidence;
mod import;
mod import_packages;
mod import_projection;
mod inventory;
mod metadata;
#[cfg(feature = "application")]
pub(crate) mod resources;
#[cfg(feature = "application")]
mod resources_platform;
mod source;
mod sync;
mod wave;
mod yaml;

use clap::Subcommand;
use std::io::{Write as _, stderr, stdout};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use tokio::runtime::Builder;

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Execute fixed preparation evidence; this cannot certify migration.
    #[cfg(feature = "application")]
    Evidence {
        #[command(subcommand)]
        command: evidence::Command,
    },
    /// Report source-bound resource accounting without changing boot policy.
    #[cfg(feature = "application")]
    Resources {
        #[arg(long, required = true)]
        check: bool,
        /// Inspect one deployment adapter; its compiler target is derived automatically.
        #[arg(
            long,
            value_enum,
            required_unless_present = "all_adapters",
            conflicts_with_all = ["all_adapters", "capture_audit", "audit", "review_commit", "catalog_baseline"]
        )]
        adapter: Option<resources::AdapterId>,
        /// Inspect the complete adapter matrix, required for canonical audits and reviews.
        #[arg(long)]
        all_adapters: bool,
        #[arg(long, requires = "check", conflicts_with = "review_commit")]
        require_fit: bool,
        /// Compare declared catalog growth with the frozen reviewed accounting baseline.
        #[arg(long, requires_all = ["check", "all_adapters"], conflicts_with_all = ["capture_audit", "audit", "review_commit"])]
        catalog_baseline: Option<PathBuf>,
        /// Emit a canonical source audit as TOML; no physical proof or change approval.
        #[arg(long, requires = "all_adapters", conflicts_with_all = ["audit", "plan", "review_commit", "require_fit"])]
        capture_audit: bool,
        /// Render a current-source audit as Markdown, or validate it with --plan.
        #[arg(long, requires = "all_adapters", conflicts_with_all = ["review_commit", "require_fit"])]
        audit: Option<PathBuf>,
        #[arg(long, requires = "audit")]
        plan: Option<PathBuf>,
        /// Check the immutable package in this full review-commit SHA; does not authorize it.
        #[arg(long, requires_all = ["review_diff", "all_adapters"])]
        review_commit: Option<String>,
        /// Check committed review-to-HEAD changes plus staged changes, excluding unstaged edits.
        #[arg(long, group = "review_diff", requires = "review_commit")]
        staged: bool,
        /// Check the complete committed diff from the review to this commit.
        #[arg(long, group = "review_diff", requires = "review_commit")]
        head: Option<String>,
    },
    /// Stage a complete, non-routable reference snapshot, or compare without writing.
    Import {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        staging: PathBuf,
        #[arg(long)]
        check: bool,
    },
    /// Record immutable source membership and endpoint provenance as TOML.
    Inventory {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Check immutable reference facts; inventory alone never proves migration.
    Check {
        #[arg(long)]
        source: PathBuf,
        #[arg(
            long,
            default_value = "crates/auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-inventory.toml"
        )]
        inventory: PathBuf,
        #[arg(long)]
        require_complete: bool,
        /// Validate a source-bound wave design; this is not execution evidence.
        #[arg(long)]
        wave: Option<PathBuf>,
        /// Source factory witness used only by a wave-design check.
        #[arg(long, requires = "wave")]
        factory_bindings: Option<PathBuf>,
        #[cfg(feature = "application")]
        #[arg(long, requires = "run_id", conflicts_with = "wave")]
        evidence_root: Option<PathBuf>,
        #[cfg(feature = "application")]
        #[arg(long, requires = "evidence_root")]
        run_id: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Error {
    Source,
    SourceLimit,
    InvalidSource,
    Output,
    InventoryInput,
    ImportInput,
    ImportOutput,
    WaveInput,
    #[cfg(feature = "application")]
    ResourceInput,
    #[cfg(feature = "application")]
    ResourceNativeFit,
    #[cfg(feature = "application")]
    UnqualifiedSchemaArtifact,
    #[cfg(feature = "application")]
    Evidence(evidence::Failure),
}

impl Error {
    const fn code(self) -> &'static str {
        match self {
            Self::Source => "bidder_source_unavailable",
            Self::SourceLimit => "bidder_source_limit",
            Self::InvalidSource => "bidder_source_invalid",
            Self::Output => "bidder_inventory_output",
            Self::InventoryInput => "bidder_inventory_input",
            Self::ImportInput => "bidder_import_input",
            Self::ImportOutput => "bidder_import_output",
            Self::WaveInput => "bidder_wave_input",
            #[cfg(feature = "application")]
            Self::ResourceInput => "bidder_resource_input",
            #[cfg(feature = "application")]
            Self::ResourceNativeFit => "bidder_resource_native_fit",
            #[cfg(feature = "application")]
            Self::UnqualifiedSchemaArtifact => "unqualified_schema_artifact",
            #[cfg(feature = "application")]
            Self::Evidence(failure) => failure.code(),
        }
    }
}

#[cfg(feature = "application")]
impl From<auctioneer_runtime::BudgetError> for Error {
    fn from(error: auctioneer_runtime::BudgetError) -> Self {
        use auctioneer_runtime::BudgetError;
        match error {
            BudgetError::UnqualifiedSchemaArtifact => Self::UnqualifiedSchemaArtifact,
            BudgetError::BudgetArithmeticOverflow
            | BudgetError::AuxWork(_)
            | BudgetError::AuxRoutePeakBelowDerived { .. }
            | BudgetError::AuxRoutePeakBelowExtractionFloor { .. }
            | BudgetError::BrotliWbitsOutOfRange { .. }
            | BudgetError::BudgetOverCeiling { .. }
            | BudgetError::LimitExceedsUsize { .. }
            | BudgetError::LimitBelowMinimum { .. }
            | BudgetError::LimitAboveMaximum { .. }
            | BudgetError::FinalResponseBelowAssemblyFloor { .. }
            | BudgetError::HeaderKnobNotCeilingPlusOne { .. }
            | BudgetError::PseudoheaderAllowanceTooSmall { .. }
            | BudgetError::H1BufSizeTooSmall { .. }
            | BudgetError::H1HeaderCountMismatch { .. }
            | BudgetError::ParserEntryCountMismatch { .. } => Self::ResourceInput,
        }
    }
}

fn publish(output: &Path, bytes: &[u8]) -> Result<(), Error> {
    if bytes.len() > 1 << 20_usize {
        return Err(Error::Output);
    }
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut file = NamedTempFile::new_in(parent).map_err(|_error| Error::Output)?;
    file.write_all(bytes).map_err(|_error| Error::Output)?;
    file.as_file().sync_all().map_err(|_error| Error::Output)?;
    file.persist_noclobber(output)
        .map_err(|_error| Error::Output)?;
    Ok(())
}

#[cfg(feature = "application")]
fn write_output(bytes: &[u8]) -> Result<(), Error> {
    stdout()
        .lock()
        .write_all(bytes)
        .map_err(|_error| Error::Output)
}

#[cfg(feature = "application")]
fn compare_catalog_resources(path: &Path, require_fit: bool) -> Result<i32, Error> {
    let baseline = check::read_input(path).map_err(|_error| Error::ResourceInput)?;
    let (bytes, exit_code) = resources::compare_catalog_baseline(&baseline, require_fit)?;
    write_output(&bytes)?;
    Ok(exit_code)
}

async fn check_inventory(
    source: &Path,
    inventory: &Path,
    require_complete: bool,
    wave: Option<&Path>,
    factory_bindings: Option<&Path>,
) -> Result<i32, Error> {
    let report = check::run(source, inventory, require_complete, wave, factory_bindings).await?;
    let mut writer = stdout().lock();
    serde_json::to_writer(&mut writer, &report).map_err(|_error| Error::Output)?;
    writer.write_all(b"\n").map_err(|_error| Error::Output)?;
    Ok(report.exit_code())
}

async fn publish_inventory(source: &Path, output: &Path) -> Result<i32, Error> {
    let data = inventory::inspect(source).await?;
    let bytes = toml::to_string_pretty(&data).map_err(|_error| Error::Output)?;
    publish(output, bytes.as_bytes())?;
    Ok(0_i32)
}

async fn execute(command: Command) -> Result<i32, Error> {
    match command {
        #[cfg(feature = "application")]
        Command::Evidence {
            command: evidence_command,
        } => evidence::run(evidence_command).await,
        #[cfg(feature = "application")]
        Command::Resources {
            adapter,
            capture_audit,
            require_fit,
            catalog_baseline,
            review_commit,
            staged,
            head,
            audit,
            plan,
            ..
        } => {
            if let Some(path) = catalog_baseline {
                return compare_catalog_resources(&path, require_fit);
            }
            if capture_audit {
                let bytes = resources::capture_audit()?;
                write_output(&bytes)?;
                return Ok(0_i32);
            }
            if let Some(audit_path) = audit {
                let audit_bytes =
                    check::read_input(&audit_path).map_err(|_error| Error::ResourceInput)?;
                let bytes = if let Some(plan_path) = plan {
                    let plan_bytes =
                        check::read_input(&plan_path).map_err(|_error| Error::ResourceInput)?;
                    resources::check_current_records(&audit_bytes, &plan_bytes)?
                } else {
                    resources::summarize_audit(&audit_bytes)?
                };
                write_output(&bytes)?;
                return Ok(0_i32);
            }
            if let Some(review_sha) = review_commit {
                let selection = if staged {
                    resources::review::Selection::Staged
                } else {
                    resources::review::Selection::Head(head.as_deref().ok_or(Error::ResourceInput)?)
                };
                let report = resources::review::run(Path::new("."), &review_sha, selection).await?;
                let bytes = resources::review::serialize(&report)?;
                write_output(&bytes)?;
                return Ok(0_i32);
            }
            let selection = adapter.map_or(resources::Selection::AllAdapters, |selected| {
                resources::Selection::Adapter { adapter: selected }
            });
            let report = resources::build_report_for(selection)?;
            let bytes = resources::serialize(&report)?;
            write_output(&bytes)?;
            Ok(report.exit_code(require_fit))
        }
        Command::Import {
            source,
            staging,
            check,
        } => import::run(&source, &staging, check).await,
        Command::Inventory { source, output } => publish_inventory(&source, &output).await,
        Command::Check {
            source,
            inventory,
            require_complete,
            wave,
            factory_bindings,
            #[cfg(feature = "application")]
            evidence_root,
            #[cfg(feature = "application")]
            run_id,
        } => {
            #[cfg(feature = "application")]
            if let (Some(root), Some(id)) = (evidence_root, run_id) {
                return evidence::consume(&source, &inventory, &root, &id).await;
            }
            check_inventory(
                &source,
                &inventory,
                require_complete,
                wave.as_deref(),
                factory_bindings.as_deref(),
            )
            .await
        }
    }
}

pub(crate) fn run(command: Command) -> i32 {
    let result = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_error| Error::Source)
        .and_then(|runtime| runtime.block_on(execute(command)));
    match result {
        Ok(code) => code,
        Err(error) => {
            let _diagnostic = writeln!(stderr().lock(), "{}", error.code());
            2
        }
    }
}
