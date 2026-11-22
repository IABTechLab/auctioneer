//! Shared configuration leaves and the direct versioned runtime policy.
//!
//! With `config-model`, `policy::OperatorPolicy` is the single operator
//! authoring and runtime payload, generic over generated bidder settings.
//! There are no startup, baked or runtime document wrappers. Build tooling
//! owns compiled product artifacts separately from operator policy.
//!
//! The default feature set remains Serde-only. `config-model` adds portable
//! models and validation; `build-support` adds native manifest admission and
//! code generation. Neither feature depends on `auctioneer-core` or
//! `auctioneer-config-data`, which consume this crate.

#[cfg(feature = "config-model")]
pub mod activity;
#[cfg(feature = "config-model")]
pub mod currency;
#[cfg(feature = "config-model")]
mod object;
#[cfg(feature = "config-model")]
pub use currency::{FloorCurrencyConfig, MAX_FLOOR_CURRENCY_RATES};
#[cfg(feature = "config-model")]
mod bidder_settings_wire;
#[cfg(feature = "config-model")]
mod model_validation;
#[cfg(feature = "config-model")]
pub mod override_value;
#[cfg(feature = "config-model")]
pub mod policy;
#[cfg(feature = "config-model")]
pub mod source;
#[cfg(feature = "config-model")]
pub use source::*;
#[cfg(feature = "build-support")]
pub mod bidder_discovery;
#[cfg(feature = "build-support")]
pub mod bidder_endpoint_codegen;
#[cfg(feature = "build-support")]
pub mod bidder_manifest;
#[cfg(feature = "build-support")]
pub mod bidder_reconcile;
#[cfg(feature = "build-support")]
pub mod bidder_settings_codegen;
#[cfg(feature = "config-model")]
pub mod bidder_storage;
#[cfg(feature = "build-support")]
pub mod bindings;
#[cfg(feature = "build-support")]
pub mod build_catalog;
#[cfg(feature = "build-support")]
pub mod build_disclosure;
#[cfg(feature = "build-support")]
pub mod codegen;
pub mod disclosure_types;
#[cfg(feature = "build-support")]
pub mod endpoint_contract;
#[cfg(feature = "config-model")]
pub mod limits;
/// Non-serving schema artifact planning, available only to native build tooling.
#[cfg(feature = "build-support")]
pub mod schema_staging;
#[cfg(feature = "build-support")]
pub mod schema_staging_build;
#[cfg(feature = "build-support")]
pub mod schema_staging_codegen;
#[cfg(feature = "config-model")]
pub mod types;
/// Build-facing URL admission and macro validation. Serving policy is internal.
#[cfg(feature = "build-support")]
pub mod url_safety;
#[cfg(all(feature = "config-model", not(feature = "build-support")))]
mod url_safety;
#[cfg(feature = "config-model")]
pub use url_safety::validate_configured_bidder_endpoint;
#[cfg(feature = "config-model")]
pub mod usersync;
