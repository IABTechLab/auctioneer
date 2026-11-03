//! `NormalizedBidRequest` — the single artifact `prepare()` produces.

use crate::types::bid_request::BidRequest;
use std::ops::Deref;

/// The single type-safe artifact produced by `prepare()`.
/// Only `prepare()` can construct one — the inner field is private.
#[expect(
    clippy::field_scoped_visibility_modifiers,
    reason = "pub(crate) inner + warnings are the seam between prepare() and into_inner(); \
              external callers route through warnings()/into_inner() accessors"
)]
#[derive(Debug)]
pub struct NormalizedBidRequest {
    pub(crate) inner: BidRequest,
    pub(crate) warnings: Vec<NonFatalWarning>,
}

impl Deref for NormalizedBidRequest {
    type Target = BidRequest;
    #[inline]
    fn deref(&self) -> &BidRequest {
        &self.inner
    }
}

impl NormalizedBidRequest {
    /// Transfer the prepared request to the exchange.
    #[must_use]
    #[inline]
    pub fn into_inner(self) -> BidRequest {
        self.inner
    }

    #[must_use]
    #[inline]
    pub fn warnings(&self) -> &[NonFatalWarning] {
        &self.warnings
    }
}

/// Non-fatal warning surfaced under `ext.warnings.prebid[]` during response build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NonFatalWarning {
    pub code: WarningCode,
    pub message: String,
    /// Optional: which field triggered the warning, for debugging.
    pub scope: Option<String>,
}

/// Numeric warning codes shared by request preparation and downstream policy.
/// `prepare()` does not parse consent strings and leaves its warnings empty.
/// Privacy resolution supplies consent warnings to the exchange, which exposes
/// them through the response warning surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum WarningCode {
    /// 10001 — emitted by `auctioneer-privacy::PrivacyContext`, NOT by
    /// `auctioneer-openrtb::prepare()`. Kept here because `NonFatalWarning`
    /// lives in this crate.
    InvalidPrivacyConsent = 10001,
    /// 10018 — the GVL revision the TC string declared is not in the
    /// bundled archive, or the blob found there disagrees with the
    /// string about the TCF policy version. Consent cannot be evaluated
    /// against a vendor list, so the request falls back to the default
    /// permission row with identifiers denied. A deployment condition,
    /// not a request one, and the signal the `/_auctioneer/health/gvl`
    /// counter watches.
    GvlVersionUnavailable = 10018,
    /// 10019 — the bundled Global CMP List is past its freshness window,
    /// so no TC string's CMP can be authenticated and consent falls back
    /// to the default permission row. A deployment condition, not a
    /// request one: the fix is rebuilding against a fresh GVL archive.
    GclStale = 10019,
    /// 10020 - per-bidder response caps exceeded the funded boot envelope
    /// and were clamped down. Independent of privacy deployment warnings.
    OutboundBudgetClamp = 10020,
    /// 10021 - invalid request price granularity fell back to Med.
    InvalidPriceGranularity = 10021,
    /// 10999 — uncategorized.
    Unknown = 10999,
}

impl WarningCode {
    #[must_use]
    #[inline]
    #[expect(
        clippy::as_conversions,
        reason = "repr(u32) makes the enum-to-wire conversion exact"
    )]
    pub fn as_u32(self) -> u32 {
        // Enum discriminants reject every duplicate at compile time, including
        // future variants; a test's separate variant list cannot do that.
        self as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warning_codes_match_the_wire_contract() {
        assert_eq!(WarningCode::InvalidPrivacyConsent.as_u32(), 10001_u32);
        assert_eq!(WarningCode::GvlVersionUnavailable.as_u32(), 10018_u32);
        assert_eq!(WarningCode::GclStale.as_u32(), 10019_u32);
        assert_eq!(WarningCode::OutboundBudgetClamp.as_u32(), 10020_u32);
        assert_eq!(WarningCode::InvalidPriceGranularity.as_u32(), 10021_u32);
        assert_eq!(WarningCode::Unknown.as_u32(), 10999_u32);
    }

    #[test]
    fn warnings_default_to_empty() {
        let norm = NormalizedBidRequest {
            inner: BidRequest::default(),
            warnings: Vec::new(),
        };
        assert!(norm.warnings().is_empty());
    }
}
