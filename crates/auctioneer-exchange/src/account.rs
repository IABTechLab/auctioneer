//! Caller-resolved account identity.

use auctioneer_config::types::AccountConfig;

/// The id [`ResolvedAccount`] carries when the request named no
/// publisher at all — prebid's `PublisherUnknown` convention.
///
/// It is a SYNTHESIZED identity, not a request-supplied one, so host
/// policy lists keyed by account id must never match it: a host that
/// wrote `non_standard_publishers = ["unknown"]` would otherwise opt
/// every publisher-less request out of GDPR enforcement in one line.
/// A publisher whose real id is literally `"unknown"` is therefore not
/// host-listable; that is the deliberate trade.
///
/// The account resolver applies the same reasoning on its own side, but
/// keyed on how the id AROSE rather than on its spelling: a request that
/// named no publisher returns before the `[accounts.<id>]` lookup runs,
/// so an `[accounts.unknown]` section cannot capture publisher-less
/// traffic. A request that literally carries `publisher.id = "unknown"`
/// DOES select that section and resolves as `Configured` — which is
/// correct (the operator addressed that publisher by name), and still
/// fail-closed here, because this gate refuses the id either way.
pub const UNRESOLVED_ACCOUNT_ID: &str = "unknown";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountSource {
    Provided,
    DefaultFallback,
    /// Account matched a `[accounts.<id>]` entry in `AuctioneerAppConfig`
    /// (single-TOML deploy — same blob the `AppConfig` extractor loads
    /// at boot; no per-request store read).
    Configured,
}

pub struct ResolvedAccount<'a> {
    /// Preserves the request-supplied id (or [`UNRESOLVED_ACCOUNT_ID`]
    /// if empty/absent) even on `DefaultFallback` — used for debug/log
    /// attribution.
    pub id: String,
    pub source: AccountSource,
    /// Embedded default-account when `source == DefaultFallback`.
    pub config: &'a AccountConfig,
}

#[cfg(test)]
mod tests {
    use super::{AccountSource, ResolvedAccount, UNRESOLVED_ACCOUNT_ID};
    use auctioneer_config::types::AccountConfig;

    #[test]
    fn carries_id_through_default_fallback() {
        let cfg = AccountConfig::default();
        let acc = ResolvedAccount {
            id: UNRESOLVED_ACCOUNT_ID.to_owned(),
            source: AccountSource::DefaultFallback,
            config: &cfg,
        };
        assert_eq!(acc.id, UNRESOLVED_ACCOUNT_ID);
    }

    #[test]
    fn configured_source_variant_pattern_matches() {
        let source = AccountSource::Configured;
        let discriminator = match source {
            AccountSource::Provided => "provided",
            AccountSource::DefaultFallback => "default_fallback",
            AccountSource::Configured => "configured",
        };
        assert_eq!(discriminator, "configured");
    }
}
