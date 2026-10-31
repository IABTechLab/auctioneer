//! `OpenRTB` 2.6 types + the `prepare(req, headers, limits)` gate.
//!
//! NOTE: no consent-string parsing here.
//! Consent parsers live in `auctioneer-privacy` from M3+; `prepare()` does not
//! touch TCF v2 / US-Privacy / GPP strings. The
//! `WarningCode::InvalidPrivacyConsent` variant is defined here (since
//! `NonFatalWarning` is) but populated by `PrivacyContext::resolve` downstream.

// Re-export surface is the crate's flat public API: NormalizedBidRequest et
// al. live in the private `normalized` module so only prepare() can construct
// one; RequestHeaders is prepare()'s input contract; ValidationError is
// validate()'s error type. Re-exporting is the only way to expose these
// without leaking the module boundary.

pub mod ext;
pub mod field_metadata;
pub mod prepare;
pub mod request_headers;
pub mod types;
pub mod validate;

mod normalized;

#[cfg(test)]
mod test_support;

pub use normalized::{NonFatalWarning, NormalizedBidRequest, WarningCode};
pub use prepare::{PrepareLimits, prepare};
pub use request_headers::RequestHeaders;
pub use validate::ValidationError;
