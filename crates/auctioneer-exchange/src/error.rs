//! Engine-domain `AuctionError`.

use auctioneer_privacy::PrivacyError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuctionError {
    /// Terminal resource refusal, never a consent syntax warning or fallback.
    #[error("auction resource limit exceeded")]
    Resource(#[from] auctioneer_runtime::ResourceError),
    /// Registry-dependent validation failure (alias unknown / no-op /
    /// `aliasgvlids` shape / imp-no-bidders after disabled-bidder removal).
    /// Caller maps to `400 Invalid request: <err>`.
    #[error("BadInput: {0}")]
    BadInput(String),
    /// Bidder-scoped FPD merge produces an invalid `Imp`.
    /// Caller maps to `500` (code 15).
    #[error("InvalidImpFirstPartyData: {0}")]
    InvalidImpFirstPartyData(String),
    /// Fan-out itself could not run (no outbound HTTP). Caller maps to `500`.
    #[error("FanoutUnavailable")]
    FanoutUnavailable,
    /// Genuine internal fault. Caller maps to `5xx`.
    #[error("Internal: {0}")]
    Internal(String),
    /// Account resolved but its config-store record is unusable (future
    /// remote-account fetch path; the single-TOML resolver never
    /// emits this — both sides are typed structs). Caller maps to
    /// `500 Invalid request: <err>` (spec §9 line 3890).
    #[error("MalformedAcct: {0}")]
    MalformedAcct(String),
    /// Account resolved to a `[accounts.<id>]` entry with `disabled =
    /// true`. Caller maps to `503 Invalid request: <err>` (spec §9).
    #[error("AccountDisabled: {0}")]
    AccountDisabled(String),
}

impl AuctionError {
    /// Prebid Server `errortypes` taxonomy code for this variant.
    /// Internal bookkeeping only — never serialized into an HTTP body
    /// (the plain-text `HandlerError` bodies carry the display message,
    /// not this numeric code).
    #[must_use]
    pub const fn code(&self) -> u32 {
        match self {
            Self::BadInput(_) => 2,
            Self::InvalidImpFirstPartyData(_) => 15,
            Self::FanoutUnavailable | Self::Internal(_) | Self::Resource(_) => 999,
            Self::MalformedAcct(_) => 10,
            Self::AccountDisabled(_) => 7,
        }
    }
}

/// `auctioneer-privacy` cannot name `AuctionError` — the reverse edge
/// would be a Cargo cycle — so the mapping lives here, on the side the
/// dependency graph already allows. The match is exhaustive on
/// purpose: a new `PrivacyError` variant must state its HTTP fate.
impl From<PrivacyError> for AuctionError {
    fn from(err: PrivacyError) -> Self {
        match err {
            PrivacyError::InvalidRegsGdpr { .. }
            | PrivacyError::InvalidNoSale
            | PrivacyError::GppLimitExceeded => Self::BadInput(err.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AuctionError, PrivacyError};
    use auctioneer_config_data::types::{AccountConfig, HostConfig};
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_openrtb::types::inventory::Site;
    use auctioneer_openrtb::types::regs::Regs;
    use auctioneer_privacy::PrivacyContext;
    use auctioneer_privacy::gcl::Gcl;
    use auctioneer_privacy::gvl::GvlStore;
    use time::OffsetDateTime;

    #[test]
    fn display_carries_message() {
        let err = AuctionError::BadInput("imp foo has no valid bidders".into());
        assert!(err.to_string().contains("imp foo has no valid bidders"));
    }

    #[test]
    fn malformed_acct_variant_carries_code_10() {
        let err = AuctionError::MalformedAcct("acct-9".into());
        assert_eq!(err.code(), 10);
    }

    #[test]
    fn account_disabled_variant_carries_code_7() {
        let err = AuctionError::AccountDisabled("acct-42".into());
        assert_eq!(err.code(), 7);
    }

    #[test]
    fn invalid_regs_gdpr_maps_to_bad_input() {
        // The handler wraps this message with `Invalid request: ` and a
        // trailing newline, so the comma form is part of the 400 body.
        let err = AuctionError::from(PrivacyError::InvalidRegsGdpr { got: 2_i32 });
        assert!(matches!(err, AuctionError::BadInput(_)));
        assert_eq!(err.to_string(), "BadInput: regs.gdpr must be 0 or 1, got 2");
    }

    #[test]
    fn privacy_errors_map_to_fixed_bad_input() {
        for (privacy, expected) in [
            (
                PrivacyError::InvalidNoSale,
                "ext.prebid.nosale contains an invalid bidder exemption",
            ),
            (
                PrivacyError::GppLimitExceeded,
                "regs.gpp exceeds privacy parsing limits",
            ),
        ] {
            let err = AuctionError::from(privacy);
            assert!(matches!(&err, AuctionError::BadInput(message) if message == expected));
        }
    }

    #[test]
    fn regs_gdpr_two_returns_bad_input_400_via_privacy_resolve() {
        // The whole propagation chain: an out-of-domain `regs.gdpr` with
        // no `gpp_sid` to disambiguate is the one fatal privacy input,
        // so resolution refuses rather than warning, and the exchange
        // boundary turns that into the 400 body.
        let req = BidRequest {
            id: "req-1".to_owned(),
            site: Some(Site::default()),
            regs: Some(Regs {
                gdpr: Some(2_i32),
                ..Regs::default()
            }),
            ..BidRequest::default()
        };
        let mut host = HostConfig::default();
        host.gdpr.enabled = true;

        let err = PrivacyContext::resolve(
            &req,
            &AccountConfig::default(),
            &host,
            &GvlStore::for_test(Vec::new()),
            Gcl::baked(),
            OffsetDateTime::UNIX_EPOCH,
            auctioneer_privacy::ResolveOptions::default(),
        )
        .expect_err("regs.gdpr = 2 with no gpp_sid is fatal");
        assert_eq!(err, PrivacyError::InvalidRegsGdpr { got: 2_i32 });

        let mapped = AuctionError::from(err);
        assert!(matches!(mapped, AuctionError::BadInput(_)));
        assert_eq!(
            mapped.to_string(),
            "BadInput: regs.gdpr must be 0 or 1, got 2"
        );
    }

    #[test]
    fn regs_gdpr_two_with_gpp_sid_is_not_fatal() {
        // A non-empty `gpp_sid` outranks `regs.gdpr`, so the same
        // out-of-domain value never gets consulted and never 400s.
        let req = BidRequest {
            id: "req-1".to_owned(),
            site: Some(Site::default()),
            regs: Some(Regs {
                gdpr: Some(2_i32),
                gpp_sid: Some(vec![2_i32]),
                ..Regs::default()
            }),
            ..BidRequest::default()
        };
        let mut host = HostConfig::default();
        host.gdpr.enabled = true;

        PrivacyContext::resolve(
            &req,
            &AccountConfig::default(),
            &host,
            &GvlStore::for_test(Vec::new()),
            Gcl::baked(),
            OffsetDateTime::UNIX_EPOCH,
            auctioneer_privacy::ResolveOptions::default(),
        )
        .expect("gpp_sid outranks the out-of-domain regs.gdpr");
    }
}
