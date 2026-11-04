//! Auctioneer operator CLI. Maintenance builds do not depend on embedded application data.
//!
//! Default builds retain typed `EdgeZero` application commands and the bounded
//! qualification harness. Without default features, repair and reference tools
//! remain available even when embedded configuration cannot build.

#[cfg(feature = "application")]
mod application;
mod bidders;
mod maintenance;
#[cfg(feature = "application")]
mod qualification;

#[cfg(feature = "application")]
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};
#[cfg(feature = "application")]
use std::env::args_os;
use std::process;

#[derive(Parser, Debug)]
#[command(name = "auctioneer", about = "Auctioneer operator CLI")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    #[cfg(feature = "application")]
    #[command(flatten)]
    Application(application::Command),
    /// Inspect pinned bidder reference data without changing runtime definitions.
    #[command(subcommand)]
    Bidders(bidders::Command),
    /// Maintain the validated vendor and CMP reference archive.
    #[command(subcommand)]
    Gvl(maintenance::gvl::Command),
    /// Inspect pinned `OpenRTB` standard membership.
    #[command(subcommand)]
    Openrtb(maintenance::openrtb::Command),
    /// Verify externally captured reference evidence without writing it.
    #[command(subcommand)]
    Reference(maintenance::reference::Command),
}

fn main() {
    #[cfg(feature = "application")]
    edgezero_cli::init_cli_logger();
    let args = Args::try_parse().unwrap_or_else(|error| {
        #[cfg(feature = "application")]
        if args_os().nth(1).is_some_and(|arg| arg == "qualify")
            && !matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            )
        {
            let _diagnostic = qualification::diagnostic("qualification_invalid_invocation");
            process::exit(2);
        }
        error.exit()
    });
    match args.command {
        #[cfg(feature = "application")]
        Command::Application(command) => process::exit(application::run(command)),
        Command::Bidders(command) => process::exit(bidders::run(command)),
        Command::Gvl(command) => process::exit(maintenance::gvl::dispatch(command)),
        Command::Openrtb(command) => process::exit(maintenance::openrtb::dispatch(command)),
        Command::Reference(command) => process::exit(maintenance::reference::dispatch(command)),
    }
}
