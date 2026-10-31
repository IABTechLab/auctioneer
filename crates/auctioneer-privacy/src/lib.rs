//! Privacy & consent API surface.
//!
//! The GDPR auction path is real end to end: the TCF v2 parser, the
//! GVL/GCL loaders, the §7.1 GDPR-applies signal math, the per-purpose
//! legal-basis tables in [`purposes`], `PrivacyContext::resolve`, and
//! `PrivacyContext::permissions`, which answers per bidder from the
//! resolved context. GPP/legacy source normalization feeds the same TCF
//! authorization path. Activity, CCPA, LMT, and COPPA decisions remain
//! independent of GDPR scope. The exchange owns request transformations.
//! The sync entry point `resolve_sync_privacy` is still deferred to M5;
//! it returns a context where every gate is `Allowed`.

mod activity;
mod auction_context;
mod auction_permissions;
pub mod ccpa;
mod decision;
mod error;
pub mod gcl;
pub mod gpp;
pub mod gvl;
pub mod purposes;
mod resolve;
mod signal;
pub mod sources;
mod sync;
pub mod tcf;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
#[cfg(test)]
mod testkit;
pub mod usp;
pub mod warnings;

pub use activity::{Activity, Component, activity_allowed};
pub use auction_context::PrivacyContext;
pub use auction_permissions::AuctionPermissions;
pub use auctioneer_openrtb::NonFatalWarning; // re-exported for convenience
pub use decision::{BidderDecision, FetchDenial};
pub use error::PrivacyError;
pub use resolve::{ResolveOptions, ResolvedPrivacy};
pub use signal::{
    AuctionGeoInput, GeoView, Signal, WireSignalInput, auction_geo_signal, resolve_wire_signal,
    validate_wire_signal_input,
};
pub use sync::{
    ActivityCallback, GdprSignalIn, SyncCallback, SyncDecision, SyncEndpoint, SyncPrivacyContext,
    SyncPrivacyError, SyncPrivacyInput, SyncerLookup, resolve_sync_privacy,
};
pub use usp::{Usp, UspParseError};
