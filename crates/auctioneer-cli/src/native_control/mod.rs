//! Bounded native file, process and task controls shared by CLI commands.

pub mod io;
pub mod process;

use serde::Serialize;
use std::result::Result as StdResult;

/// Workspace-relative source namespace included in harness input identities.
pub const SOURCE_ROOT: &str = "crates/auctioneer-cli/src/native_control";

/// Fixed diagnostic category and optional stage, without input or child contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct HarnessError {
    /// Fixed diagnostic code selected by the caller or native control.
    pub code: &'static str,
    /// Optional fixed stage label, omitted from serialization when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<&'static str>,
}

impl HarnessError {
    /// Create an unstaged diagnostic with the supplied fixed code.
    #[must_use]
    pub const fn new(code: &'static str) -> Self {
        Self { code, stage: None }
    }

    /// Attach a fixed stage label without changing the diagnostic code.
    #[must_use]
    pub const fn at(self, stage: &'static str) -> Self {
        Self {
            stage: Some(stage),
            ..self
        }
    }
}

/// Result carrying only a fixed native-control diagnostic.
pub type Result<T> = StdResult<T, HarnessError>;
