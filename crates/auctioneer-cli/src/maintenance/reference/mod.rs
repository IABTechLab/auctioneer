mod integrity;
mod provenance;

use std::{
    io::{self, Write as _},
    path::PathBuf,
};

use anyhow::Result;
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Verify frozen captures and provenance; optionally check source fingerprints.
    Check {
        #[arg(long)]
        root: PathBuf,
        #[arg(long)]
        source: Option<PathBuf>,
    },
}

fn run(command: Command) -> Result<()> {
    let Command::Check { root, source } = command;
    integrity::validate(&root, source.as_deref())?;
    writeln!(
        io::stdout().lock(),
        "verified 10 frozen captures; no files written"
    )?;
    Ok(())
}

pub(crate) fn dispatch(command: Command) -> i32 {
    match run(command) {
        Ok(()) => 0,
        Err(error) => {
            let _diagnostic = writeln!(io::stderr().lock(), "Error: {error:#}");
            1
        }
    }
}
