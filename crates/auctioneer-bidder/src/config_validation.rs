//! Pure bidder-owned configuration validation, independent of serving construction.
//!
//! A callback borrows both supplied entries. The caller admits bindings and
//! resolves enablement; the bidder checks supplied values even when shadowed or
//! disabled, and intrinsic requiredness only for enabled, resolvable identities.
//! No callback may perform I/O, resolve secrets or install mutable global state.
//! Temporary validator errors remain local and must be funded before activation.

use crate::BidderSettings;
use auctioneer_config_data::types::BidderInfo;
use std::{error::Error, fmt};

/// One borrowed context for supplied and effective bidder-owned rules.
#[derive(Clone, Copy)]
pub struct BidderConfigInput<'a> {
    /// Explicit settings for the requested identity, before fieldwise fallback.
    pub requested: Option<&'a BidderSettings>,
    /// Independently supplied canonical-base settings, including shadowed values.
    pub base: Option<&'a BidderSettings>,
    /// Resolved metadata, or `None` for a future request-local identity.
    pub definition: Option<&'a BidderInfo>,
    /// Effective enablement, resolved by the shared admission owner.
    pub enabled: bool,
}

/// Thin adapter to the owner's borrowed `validator::Validate` view.
pub type BidderConfigValidator = for<'a> fn(BidderConfigInput<'a>) -> Result<(), BidderConfigError>;

/// Reviewed static identifiers only, never settings, instance names or secrets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BidderConfigError {
    /// Owner-declared field identifier.
    pub field: &'static str,
    /// Owner-declared refusal identifier.
    pub reason: &'static str,
}

impl fmt::Display for BidderConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "bidder configuration invalid: {} ({})",
            self.field, self.reason
        )
    }
}

impl Error for BidderConfigError {}

#[cfg(test)]
mod tests {
    use super::{BidderConfigError, BidderConfigInput, BidderConfigValidator};
    use auctioneer_config_data::types::BidderInfo;

    #[test]
    fn callback_accepts_independent_borrowed_context_without_factories() {
        fn check(input: BidderConfigInput<'_>) -> Result<(), BidderConfigError> {
            let expected = input.requested.is_none()
                && input.base.is_none()
                && input
                    .definition
                    .is_some_and(|info| info.name == "fake_definition")
                && input.enabled;
            Err(BidderConfigError {
                field: "fake_field",
                reason: if expected {
                    "fake_reason"
                } else {
                    "fake_unexpected_context"
                },
            })
        }
        let definition = BidderInfo {
            name: "fake_definition".into(),
            ..BidderInfo::default()
        };
        let callback: BidderConfigValidator = check;
        let error = callback(BidderConfigInput {
            requested: None,
            base: None,
            definition: Some(&definition),
            enabled: true,
        })
        .unwrap_err();
        assert_eq!(
            error,
            BidderConfigError {
                field: "fake_field",
                reason: "fake_reason"
            }
        );
        assert_eq!(
            error.to_string(),
            "bidder configuration invalid: fake_field (fake_reason)"
        );
        assert_eq!(
            size_of::<BidderConfigError>(),
            2 * size_of::<&'static str>()
        );
    }
}
