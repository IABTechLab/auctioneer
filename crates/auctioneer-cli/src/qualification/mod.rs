mod evaluate;
mod profile;
mod report;

mod harness;

use std::fs::{self, File};
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

use clap::Subcommand;
use serde::{Serialize, de::DeserializeOwned};

const MAX_INPUT_BYTES: usize = 1 << 20;
const MAX_NODES: u32 = 8_192;
const MAX_ROWS: usize = 64;
const MAX_OUTPUT_BYTES: usize = 256 << 10;

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Validate a bounded, predeclared resource profile offline.
    Validate {
        #[arg(long)]
        profile: PathBuf,
    },
    /// Evaluate safe observations against a predeclared resource profile.
    Evaluate {
        #[arg(long)]
        profile: PathBuf,
        #[arg(long)]
        observations: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Prepare and validate isolated provider qualification artifacts offline.
    #[command(subcommand)]
    Providers(harness::commands::ProviderCommand),
    /// Prepare or measure a digest-bound native resource qualification.
    Native(harness::commands::NativeArgs),
    #[command(hide = true)]
    Fixture,
    #[command(hide = true)]
    FrozenProbe {
        #[arg(long)]
        evidence: PathBuf,
    },
}

enum OfflineCommand {
    Validate {
        profile: PathBuf,
    },
    Evaluate {
        profile: PathBuf,
        observations: PathBuf,
        output: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Error {
    InputIo,
    InputLimit,
    NodeLimit,
    InvalidJson,
    InvalidProfile,
    ProfileCapacity,
    InvalidObservations,
    OutputLimit,
    OutputIo,
}

impl Error {
    const fn code(self) -> &'static str {
        match self {
            Self::InputIo => "qualification_input_io",
            Self::InputLimit => "qualification_input_limit",
            Self::NodeLimit => "qualification_node_limit",
            Self::InvalidJson => "qualification_invalid_json",
            Self::InvalidProfile => "qualification_invalid_profile",
            Self::ProfileCapacity => "qualification_profile_capacity",
            Self::InvalidObservations => "qualification_invalid_observations",
            Self::OutputLimit => "qualification_output_limit",
            Self::OutputIo => "qualification_output_io",
        }
    }
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(Error::InputLimit);
    }
    // Count all syntactic values/keys before a typed deserializer can discard
    // unknown subtrees. Never surface parser errors or arbitrary source keys.
    auctioneer_runtime::count_json_nodes(bytes, MAX_NODES).map_err(|error| match error {
        auctioneer_runtime::BoundedJsonError::NodeBudget(_) => Error::NodeLimit,
        auctioneer_runtime::BoundedJsonError::Syntax(_)
        | auctioneer_runtime::BoundedJsonError::RawValue(_) => Error::InvalidJson,
    })?;
    serde_json::from_slice(bytes).map_err(|_error| Error::InvalidJson)
}

fn read_input(path: &Path) -> Result<Vec<u8>, Error> {
    let metadata = fs::metadata(path).map_err(|_error| Error::InputIo)?;
    if !metadata.is_file() {
        return Err(Error::InputIo);
    }
    if metadata.len() > u64::try_from(MAX_INPUT_BYTES).map_err(|_error| Error::InputLimit)? {
        return Err(Error::InputLimit);
    }
    let file = File::open(path).map_err(|_error| Error::InputIo)?;
    let limit = u64::try_from(MAX_INPUT_BYTES)
        .map_err(|_error| Error::InputLimit)?
        .saturating_add(1);
    let mut bytes = Vec::new();
    file.take(limit)
        .read_to_end(&mut bytes)
        .map_err(|_error| Error::InputIo)?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(Error::InputLimit);
    }
    Ok(bytes)
}

fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    let size =
        auctioneer_runtime::serialized_json_bytes(value).map_err(|_error| Error::OutputLimit)?;
    let capacity = usize::try_from(size).map_err(|_error| Error::OutputLimit)?;
    if capacity > MAX_OUTPUT_BYTES {
        return Err(Error::OutputLimit);
    }
    let mut output = Vec::with_capacity(capacity);
    serde_json::to_writer(&mut output, value).map_err(|_error| Error::OutputLimit)?;
    if output.len() > MAX_OUTPUT_BYTES {
        return Err(Error::OutputLimit);
    }
    Ok(output)
}

fn execute(command: OfflineCommand) -> Result<bool, Error> {
    match command {
        OfflineCommand::Validate { profile: path } => {
            profile::parse(&read_input(&path)?)?;
            let validation = serialize(&ValidationResult {
                profile_validation: "valid",
            })?;
            let mut stdout = io::stdout().lock();
            stdout
                .write_all(&validation)
                .map_err(|_error| Error::OutputIo)?;
            stdout.write_all(b"\n").map_err(|_error| Error::OutputIo)?;
            Ok(true)
        }
        OfflineCommand::Evaluate {
            profile: path,
            observations: observations_path,
            output,
        } => {
            let profile_bytes = read_input(&path)?;
            let profile = profile::parse(&profile_bytes)?;
            let observations = report::parse(&read_input(&observations_path)?)?;
            let result =
                evaluate::evaluate(profile, observations, profile::Digest::of(&profile_bytes))?;
            let qualified = result.profile_qualification == report::Qualification::Qualified;
            let bytes = serialize(&result)?;
            // Refusals cannot truncate an input: serialize before creating a
            // fresh output destination.
            let mut file = File::options()
                .write(true)
                .create_new(true)
                .open(output)
                .map_err(|_error| Error::OutputIo)?;
            file.write_all(&bytes)
                .and_then(|()| file.write_all(b"\n"))
                .map_err(|_error| Error::OutputIo)?;
            file.sync_all().map_err(|_error| Error::OutputIo)?;
            Ok(qualified)
        }
    }
}

#[derive(Serialize)]
struct ValidationResult {
    profile_validation: &'static str,
}

pub(crate) fn run(command: Command) -> i32 {
    let offline = match command {
        Command::Validate { profile } => OfflineCommand::Validate { profile },
        Command::Evaluate {
            profile,
            observations,
            output,
        } => OfflineCommand::Evaluate {
            profile,
            observations,
            output,
        },
        Command::Providers(provider) => {
            return harness::commands::run(harness::commands::Invocation::Provider(provider));
        }
        Command::Native(native) => {
            return harness::commands::run(harness::commands::Invocation::Native(native));
        }
        Command::Fixture => return harness::commands::run(harness::commands::Invocation::Fixture),
        Command::FrozenProbe { evidence } => {
            return harness::commands::run(harness::commands::Invocation::FrozenProbe(evidence));
        }
    };
    match execute(offline) {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(error) => {
            let _diagnostic = diagnostic(error.code());
            2
        }
    }
}

pub(crate) fn diagnostic(code: &str) -> io::Result<()> {
    let mut stderr = io::stderr().lock();
    stderr.write_all(code.as_bytes())?;
    stderr.write_all(b"\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualification_input_byte_limit_is_inclusive() {
        let mut bytes = vec![b' '; MAX_INPUT_BYTES];
        bytes[0] = b'0';
        assert_eq!(decode::<u64>(&bytes).unwrap(), 0);
        bytes.push(b' ');
        assert_eq!(decode::<u64>(&bytes).unwrap_err(), Error::InputLimit);
    }

    #[test]
    fn qualification_node_limit_counts_unknown_keys_before_typed_parsing() {
        let bytes = serde_json::to_vec(&vec![0_i32; 8_191]).unwrap();
        decode::<Vec<u8>>(&bytes).unwrap();
        let overflow_bytes = serde_json::to_vec(&vec![0_i32; 8_192]).unwrap();
        assert_eq!(
            decode::<Vec<u8>>(&overflow_bytes).unwrap_err(),
            Error::NodeLimit
        );
        let unknown_key_bytes = format!(
            "{{\"SECRET_CANARY\":{}}}",
            String::from_utf8(overflow_bytes).unwrap()
        );
        assert_eq!(
            decode::<profile::Profile>(unknown_key_bytes.as_bytes()).unwrap_err(),
            Error::NodeLimit
        );
    }

    #[test]
    fn qualification_output_limit_is_inclusive_and_fails_without_truncation() {
        let bytes = serialize(&"x".repeat(MAX_OUTPUT_BYTES - 2)).unwrap();
        assert_eq!(bytes.len(), MAX_OUTPUT_BYTES);
        assert_eq!(
            serialize(&"x".repeat(MAX_OUTPUT_BYTES - 1)).unwrap_err(),
            Error::OutputLimit
        );
    }
}
