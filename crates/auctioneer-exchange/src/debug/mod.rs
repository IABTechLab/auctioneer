//! Pure debug permission and wire policies. Engine capture is not enabled here.

pub mod capture;
pub mod dto;
pub mod metadata;

use crate::{AliasMap, response::ErrorEntry};
use auctioneer_bidder::{BidderName, BidderRegistry};
use auctioneer_config_data::types::AccountConfig;
use auctioneer_openrtb::ext::request_prebid::DebugSignal;
use auctioneer_openrtb::types::bid_request::BidRequest;

/// Inspect before allocating tentative capture. Warning emission still waits
/// for the engine's auction/finalized-call boundary, not this pure decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugPermission {
    Unrequested,
    AccountDenied,
    Allowed,
}

impl DebugPermission {
    #[must_use]
    pub fn requested(request: &BidRequest, caller_requested: bool) -> bool {
        let body_debug = request
            .ext
            .as_ref()
            .and_then(|ext| ext.prebid.as_ref())
            .and_then(|prebid| prebid.debug);
        caller_requested
            || request.test == Some(1_i32)
            || matches!(
                body_debug,
                Some(DebugSignal::Int(1_i32) | DebugSignal::Bool(true))
            )
    }

    #[must_use]
    pub fn resolve(request: &BidRequest, caller_requested: bool, account: &AccountConfig) -> Self {
        if !Self::requested(request, caller_requested) {
            Self::Unrequested
        } else if account.debug_allow {
            Self::Allowed
        } else {
            Self::AccountDenied
        }
    }

    /// An alias uses its compiled base's policy, never a shadowed registry row.
    /// Unknown base metadata cannot authorize capture. This does not establish
    /// that the bidder survived privacy, sampling or transport finalization.
    #[must_use]
    pub fn bidder_allowed(
        self,
        observable: &BidderName,
        aliases: &AliasMap,
        registry: &BidderRegistry,
    ) -> bool {
        self == Self::Allowed
            && registry
                .get(aliases.bases.get(observable).unwrap_or(observable).as_str())
                .is_some_and(|(info, _bidder)| info.debug_allow)
    }
}

#[must_use]
pub fn account_denied_warning() -> ErrorEntry {
    ErrorEntry {
        code: 10002,
        message: "debug disabled for account".into(),
    }
}

#[must_use]
pub fn bidder_denied_warning() -> ErrorEntry {
    ErrorEntry {
        code: 10003,
        message: "debug disabled for bidder".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{DebugPermission, account_denied_warning, bidder_denied_warning};
    use crate::AliasMap;
    use crate::fake_bidder::FakeBidder;
    use crate::response::ErrorEntry;
    use auctioneer_bidder::{BidderName, BidderRegistry};
    use auctioneer_config_data::types::{AccountConfig, BidderInfo};
    use auctioneer_openrtb::ext::request_prebid::DebugSignal;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::json;

    #[test]
    fn independent_body_and_caller_signals_never_override_account_permission() {
        for test in [
            None,
            Some(-1_i32),
            Some(0_i32),
            Some(1_i32),
            Some(2_i32),
            Some(i32::MAX),
        ] {
            for signal in [
                None,
                Some(DebugSignal::Int(-1)),
                Some(DebugSignal::Int(0)),
                Some(DebugSignal::Int(1)),
                Some(DebugSignal::Int(2)),
                Some(DebugSignal::Bool(false)),
                Some(DebugSignal::Bool(true)),
            ] {
                let request: BidRequest = serde_json::from_value(json!({
                    "id":"r", "test": test, "ext": {"prebid": {"debug": signal}}
                }))
                .unwrap();
                for caller in [false, true] {
                    for allow in [false, true] {
                        let account = AccountConfig {
                            debug_allow: allow,
                            ..AccountConfig::default()
                        };
                        let requested = caller
                            || test == Some(1_i32)
                            || matches!(
                                signal,
                                Some(DebugSignal::Int(1_i32) | DebugSignal::Bool(true))
                            );
                        let expected = if !requested {
                            DebugPermission::Unrequested
                        } else if allow {
                            DebugPermission::Allowed
                        } else {
                            DebugPermission::AccountDenied
                        };
                        assert_eq!(
                            DebugPermission::resolve(&request, caller, &account),
                            expected
                        );
                    }
                }
            }
        }
    }

    fn register(registry: &mut BidderRegistry, name: &str, allow: bool) {
        registry
            .register(
                Box::leak(Box::new(BidderInfo {
                    name: name.into(),
                    debug_allow: allow,
                    ..BidderInfo::default()
                })),
                Box::new(FakeBidder::default()),
            )
            .expect("unique test bidder");
    }

    #[test]
    fn compiled_base_permission_wins_even_when_an_alias_shadows_a_registered_bidder() {
        let mut registry = BidderRegistry::new();
        register(&mut registry, "fake_allowed", true);
        register(&mut registry, "fake_denied", false);
        register(&mut registry, "fake_shadowed", true);
        let mut aliases = AliasMap::default();
        for (alias, base) in [
            ("fake_alias_allowed", "fake_allowed"),
            ("fake_alias_denied", "fake_denied"),
            ("fake_shadowed", "fake_denied"),
            ("fake_missing_base", "fake_absent"),
        ] {
            aliases
                .bases
                .insert(BidderName::new(alias), BidderName::new(base));
        }
        for (permission, expected) in [
            (DebugPermission::Unrequested, [false; 7]),
            (DebugPermission::AccountDenied, [false; 7]),
            (
                DebugPermission::Allowed,
                [true, true, false, false, false, false, false],
            ),
        ] {
            let observed = [
                "fake_allowed",
                "fake_alias_allowed",
                "fake_denied",
                "fake_alias_denied",
                "fake_shadowed",
                "fake_missing_base",
                "fake_absent",
            ]
            .map(|name| permission.bidder_allowed(&BidderName::new(name), &aliases, &registry));
            assert_eq!(observed, expected, "{permission:?}");
        }
    }

    #[test]
    fn denial_warnings_have_only_fixed_codes_and_messages() {
        assert_eq!(
            account_denied_warning(),
            ErrorEntry {
                code: 10002,
                message: "debug disabled for account".into()
            }
        );
        assert_eq!(
            bidder_denied_warning(),
            ErrorEntry {
                code: 10003,
                message: "debug disabled for bidder".into()
            }
        );
    }
}
