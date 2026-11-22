//! Source-bound ownership descriptions; never physical costs or serving admission.

mod auxiliary;
mod catalog_reference;
pub(crate) mod descriptor;
pub(crate) mod expanded;
mod growth;
mod initialization;
mod literal_construction;
mod metadata_registry;
mod observations;
mod options;
mod program_construction;
mod retained;
pub(super) mod runtime_sources;
mod storage;
