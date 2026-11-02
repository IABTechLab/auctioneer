//! Per-bidder permissions outcome.

/// Per-bidder permission summary the M2 stub returns `AllowAll`; M3 swaps in
/// a structured `Partial { … }` carrying purpose/feature decisions.
/// Phase-1 callers consume `AllowAll` / `DenyBidRequest` directly; `Partial`
/// flows through to the bidder-scoped scrub matrix.
///
/// Variant order is incidentally alphabetical here so no
/// `arbitrary_source_item_ordering` annotation is needed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuctionPermissions {
    /// `AllowBidRequest && PassID && PassGeo && every special feature opted-in`.
    AllowAll,
    /// `AllowBidRequest == false` — bidder is dropped with a non-fatal warning
    /// (code 10017 `BidderBlockedByPrivacySettings`).
    DenyBidRequest,
    /// M3+ — placeholder for the structured mapping. Tagged so
    /// adding fields later is non-breaking.
    Partial {
        allow_bid_request: bool,
        pass_id: bool,
        pass_geo: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::AuctionPermissions;

    #[test]
    fn allow_all_is_distinct_from_deny() {
        assert_ne!(
            AuctionPermissions::AllowAll,
            AuctionPermissions::DenyBidRequest
        );
    }
}
