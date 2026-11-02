//! The ROOT document shape of `auctioneer.toml` — the single source
//! artifact an operator edits.
//!
//! `auctioneer.toml` carries two explicit namespaces:
//!
//! ```toml
//! [baked.host]     # compiled in by build.rs — REBUILD REQUIRED
//! [baked.account]  # compiled in by build.rs — REBUILD REQUIRED
//! [runtime]        # pushed by `auctioneer-cli config push`
//! ```
//!
//! Two consumers read that one document and each needs a different
//! half of it typed, so the root is generic over all three subtrees:
//!
//! - `auctioneer-config`'s build script instantiates
//!   `AuctioneerConfigSource<HostDefaultRaw, AccountRaw, toml::Value>` —
//!   both baked tables fully strict, the runtime subtree parsed only
//!   loosely enough to prove it EXISTS.
//! - `auctioneer-cli` instantiates
//!   `AuctioneerConfigSource<toml::Value, toml::Value, AuctioneerAppConfig>` —
//!   the mirror image. A typo inside `[baked.host]` is caught at
//!   BUILD, not at push: the CLI cannot see the baked types without a
//!   dependency cycle, so it accepts those tables as opaque values.
//!   Both namespaces must still be PRESENT, which the strict root
//!   enforces.
//!
//! Strict SUBTREE parsing alone cannot reject an extra ROOT namespace
//! — a `[runtim]` typo or a stray `[baked.other]` would simply never
//! be read. `deny_unknown_fields` on both structs here is what closes
//! that, and hosting ONE root definition for both consumers is what
//! stops them drifting.
//!
//! The default root schema depends on `serde` only. `config-model` owns
//! shared serving DTOs and validation; `build-support` adds native manifest
//! admission and code generation. Neither feature may depend on
//! `auctioneer-core` or `auctioneer-config`: `auctioneer-core` depends on
//! `auctioneer-config`, whose build script deserializes through this type,
//! so any such edge would make that build script cyclic.

use serde::Deserialize;

#[cfg(feature = "config-model")]
pub mod activity;
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
pub mod usersync;

/// The whole `auctioneer.toml` document. Both namespaces are REQUIRED:
/// a document missing either one is a build failure rather than a
/// silently-defaulted config.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuctioneerConfigSource<Host, Account, Runtime> {
    /// Compiled into the binary by `auctioneer-config`'s build script.
    /// Changing anything under it requires a REBUILD; `config push`
    /// cannot reach it and neither can a restart.
    pub baked: BakedSection<Host, Account>,

    /// Pushed as a blob envelope by `auctioneer-cli config push` and
    /// effective on the next request. Every field is an `Option<_>`
    /// overlay over the baked layer, so a pushed blob stays PARTIAL.
    pub runtime: Runtime,
}

/// The `[baked]` namespace: the build-time defaults, one table per
/// resolution scope.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BakedSection<Host, Account> {
    /// `[baked.host]` — the host-scoped defaults `host_default()`
    /// returns.
    pub host: Host,

    /// `[baked.account]` — the account-scoped defaults
    /// `account_default()` returns.
    pub account: Account,
}
