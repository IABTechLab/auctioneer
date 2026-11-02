//! Shared source identity and bounded native controls for the operator CLI.

pub mod native_control;
pub mod resource_identity;

pub use resource_identity::{BuildIdentity, Snapshot, snapshot};
