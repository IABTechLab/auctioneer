//! Offline `OpenRTB` maintenance commands.

mod catalogue;

use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    #[command(subcommand)]
    Catalogue(catalogue::Command),
}

pub(crate) fn dispatch(command: Command) -> i32 {
    match command {
        Command::Catalogue(catalogue_command) => catalogue::dispatch(catalogue_command),
    }
}
