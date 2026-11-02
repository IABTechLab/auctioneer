//! Application commands are excluded from archive-independent maintenance builds.

use crate::config_source::AuctioneerPushSource;
use crate::qualification;
use clap::Subcommand;
use edgezero_cli::DiffExit;
use edgezero_cli::args::{
    ActiveVersionArgs, AuthArgs, BuildArgs, ConfigDiffArgs, ConfigGcArgs, ConfigPushArgs,
    ConfigValidateArgs, DeployArgs, HealthcheckArgs, NewArgs, ProvisionArgs, RollbackArgs,
    ServeArgs,
};

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// Resolve and print the currently-active service version.
    ActiveVersion(ActiveVersionArgs),
    /// Sign in, out, or query status using the hosting platform's native CLI.
    Auth(AuthArgs),
    /// Build the project for a target edge.
    Build(BuildArgs),
    /// Inspect or mutate the typed app config.
    #[command(subcommand)]
    Config(ConfigCommand),
    /// Deploy to a target edge, including a staging draft.
    Deploy(DeployArgs),
    /// Probe a deployed or staged version's health.
    Healthcheck(HealthcheckArgs),
    /// Create a new `EdgeZero` app skeleton.
    New(NewArgs),
    /// Create platform resources backing the declared stores.
    Provision(ProvisionArgs),
    /// Validate and evaluate deployment resource qualification offline.
    #[command(subcommand)]
    Qualify(qualification::Command),
    /// Roll a service back or deactivate a staging version.
    Rollback(RollbackArgs),
    /// Run a hosting-platform-specific local simulation.
    Serve(ServeArgs),
}

#[derive(Subcommand, Debug)]
pub(crate) enum ConfigCommand {
    /// Diff typed app config against the live store.
    Diff(ConfigDiffArgs),
    /// Reclaim config-store chunks that no live pointer references.
    Gc(ConfigGcArgs),
    /// Push typed app config as a blob envelope.
    Push(ConfigPushArgs),
    /// Validate the manifest and typed app config using the push contract.
    Validate(ConfigValidateArgs),
}

pub(crate) fn run(command: Command) -> i32 {
    let result: Result<(), String> = match command {
        Command::ActiveVersion(args) => edgezero_cli::run_active_version(&args),
        Command::Auth(args) => edgezero_cli::run_auth(&args),
        Command::Build(args) => edgezero_cli::run_build(&args),
        Command::Config(ConfigCommand::Push(args)) => {
            edgezero_cli::run_config_push_typed::<AuctioneerPushSource>(&args)
        }
        Command::Config(ConfigCommand::Diff(args)) => {
            match edgezero_cli::run_config_diff_typed::<AuctioneerPushSource>(&args) {
                Ok(DiffExit { code: 0 }) => Ok(()),
                Ok(DiffExit { code }) => return code,
                Err(error) => Err(error),
            }
        }
        Command::Config(ConfigCommand::Gc(args)) => edgezero_cli::run_config_gc(&args),
        Command::Config(ConfigCommand::Validate(args)) => {
            edgezero_cli::run_config_validate_typed::<AuctioneerPushSource>(&args)
        }
        Command::Deploy(args) => edgezero_cli::run_deploy(&args),
        Command::Healthcheck(args) => edgezero_cli::run_healthcheck(&args),
        Command::New(args) => edgezero_cli::run_new(&args),
        Command::Provision(args) => edgezero_cli::run_provision(&args),
        Command::Qualify(qualification_command) => {
            return qualification::run(qualification_command);
        }
        Command::Rollback(args) => edgezero_cli::run_rollback(&args),
        Command::Serve(args) => edgezero_cli::run_serve(&args),
    };
    if let Err(error) = result {
        log::error!("[auctioneer] {error}");
        2
    } else {
        0
    }
}
