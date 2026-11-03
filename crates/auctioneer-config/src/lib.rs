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
//! - `auctioneer-config-data`'s build script instantiates
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
//! `auctioneer-core` or `auctioneer-config-data`: `auctioneer-core` depends on
//! `auctioneer-config-data`, whose build script deserializes through this type,
//! so any such edge would make that build script cyclic.

use core::fmt;
use serde::{Deserialize, Deserializer};

/// Strict operator authoring document, separate from compiled product data.
///
/// Both sections are required even when a caller chooses optional subtree
/// types. This staged root replaces [`AuctioneerConfigSource`] at cutover;
/// it never accepts that type's retired `baked` namespace.
#[derive(Deserialize)]
#[serde(
    deny_unknown_fields,
    bound(deserialize = "Startup: Deserialize<'de>, Runtime: Deserialize<'de>")
)]
pub struct OperatorConfigSource<Startup, Runtime> {
    /// Deployment settings validated before policy extraction or admission.
    #[serde(deserialize_with = "required_section")]
    pub startup: Startup,
    /// Complete policy publication, projected by the CLI's push wrapper.
    #[serde(deserialize_with = "required_section")]
    pub runtime: Runtime,
}

fn required_section<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer)
}

impl<Startup, Runtime> fmt::Debug for OperatorConfigSource<Startup, Runtime> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OperatorConfigSource")
            .field("startup", &"<redacted>")
            .field("runtime", &"<redacted>")
            .finish()
    }
}

#[cfg(test)]
mod operator_source_tests {
    use super::OperatorConfigSource;

    #[test]
    fn debug_does_not_require_subtrees_to_implement_debug() {
        struct Private;
        let source = OperatorConfigSource {
            startup: Private,
            runtime: Private,
        };
        assert_eq!(
            format!("{source:?}"),
            "OperatorConfigSource { startup: \"<redacted>\", runtime: \"<redacted>\" }"
        );
    }

    #[test]
    fn normal_and_alternate_debug_are_independent_of_private_values() {
        let source = OperatorConfigSource {
            startup: "private-startup-prefix-secret-suffix",
            runtime: "private-policy-prefix-secret-suffix",
        };
        assert_eq!(
            format!("{source:?}"),
            "OperatorConfigSource { startup: \"<redacted>\", runtime: \"<redacted>\" }"
        );
        assert_eq!(
            format!("{source:#?}"),
            "OperatorConfigSource {\n    startup: \"<redacted>\",\n    runtime: \"<redacted>\",\n}"
        );
    }
}

#[cfg(feature = "config-model")]
pub mod activity;
#[cfg(feature = "config-model")]
pub mod currency;
#[cfg(feature = "config-model")]
pub use currency::{FloorCurrencyConfig, MAX_FLOOR_CURRENCY_RATES};
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
pub mod usersync;

/// The whole `auctioneer.toml` document. Both namespaces are REQUIRED:
/// a document missing either one is a build failure rather than a
/// silently-defaulted config.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuctioneerConfigSource<Host, Account, Runtime> {
    /// Compiled into the binary by `auctioneer-config-data`'s build script.
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
