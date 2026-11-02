//! Privacy-crate error type.
//!
//! `auctioneer-exchange` already depends on `auctioneer-privacy`, so the
//! privacy crate cannot name `AuctionError` — the reverse edge would be
//! a Cargo cycle. The mapping to `AuctionError::BadInput` lives on the
//! exchange side, the direction the graph allows.

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PrivacyError {
    /// `regs.gdpr` carried a value other than 0 or 1 (§7.1 rule 2).
    #[error("regs.gdpr must be 0 or 1, got {got}")]
    InvalidRegsGdpr { got: i32 },
    /// No-sale exemptions name an unknown identity or mix a wildcard with names.
    #[error("ext.prebid.nosale contains an invalid bidder exemption")]
    InvalidNoSale,
    /// Bounded GPP decoding exhausted an input, output, arithmetic, or work limit.
    #[error("regs.gpp exceeds privacy parsing limits")]
    GppLimitExceeded,
}
