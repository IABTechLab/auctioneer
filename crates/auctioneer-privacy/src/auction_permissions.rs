//! Per-bidder permissions outcome.

/// Per-bidder request and disclosure permissions. `DenyBidRequest` prevents
/// dispatch; `Partial` carries independent identifier and geo decisions to the
/// bidder-scoped scrub matrix.
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
    /// Independent request, identifier and geo permission decisions.
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
