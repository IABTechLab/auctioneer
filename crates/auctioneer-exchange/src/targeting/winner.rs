//! Winner selection.

use auctioneer_bidder::{BidderName, TypedBid};

#[derive(Clone, Debug)]
pub struct Winner {
    pub bidder: BidderName,
    pub bid_index: usize,
    pub price: f64,
    pub has_deal: bool,
}

/// Returns `true` if `candidate` beats `incumbent`.
#[must_use]
pub fn is_new_winning_bid(
    incumbent: Option<&Winner>,
    candidate: &Winner,
    prefer_deals: bool,
) -> bool {
    let Some(inc) = incumbent else {
        return true;
    };
    if prefer_deals {
        match (inc.has_deal, candidate.has_deal) {
            (false, true) => return true,
            (true, false) => return false,
            _ => {}
        }
    }
    candidate.price > inc.price
}

/// Per-imp winner reduction. Bidders/bids are presented in registry
/// order (caller's responsibility).
#[must_use]
pub fn pick_winner(
    bidder_bids: &[(BidderName, &[TypedBid])],
    imp_id: &str,
    prefer_deals: bool,
) -> Option<Winner> {
    let mut incumbent: Option<Winner> = None;
    for (bidder, bids) in bidder_bids {
        for (idx, typed) in bids.iter().enumerate() {
            if typed.bid.impid != imp_id {
                continue;
            }
            let cand = Winner {
                bidder: bidder.clone(),
                bid_index: idx,
                price: typed.bid.price,
                has_deal: typed.bid.deal_id.as_deref().is_some_and(|d| !d.is_empty()),
            };
            if is_new_winning_bid(incumbent.as_ref(), &cand, prefer_deals) {
                incumbent = Some(cand);
            }
        }
    }
    incumbent
}

#[cfg(test)]
mod tests {
    use super::{BidderName, TypedBid, pick_winner};
    use auctioneer_config_data::types::MediaType;
    use auctioneer_openrtb::types::bid_response::Bid;

    /// Borrow an owned fixture as the slice form `pick_winner` takes.
    ///
    /// The owned `Vec` per bidder is what a fixture naturally builds;
    /// the function takes slices because its production caller has
    /// bundles it must not clone.
    fn as_slices(owned: &[(BidderName, Vec<TypedBid>)]) -> Vec<(BidderName, &[TypedBid])> {
        owned
            .iter()
            .map(|(name, bids)| (name.clone(), bids.as_slice()))
            .collect()
    }

    fn typed_bid(impid: &str, price: f64, deal: Option<&str>) -> TypedBid {
        TypedBid {
            bid: Bid {
                id: "b".into(),
                impid: impid.into(),
                price,
                deal_id: deal.map(String::from),
                ..Bid::default()
            },
            media_type: MediaType::Banner,
        }
    }

    #[test]
    fn higher_price_wins_without_prefer_deals() {
        let bids = vec![
            (
                BidderName::new("fake_a"),
                vec![typed_bid("imp1", 1.0, None)],
            ),
            (
                BidderName::new("fake_b"),
                vec![typed_bid("imp1", 2.0, None)],
            ),
        ];
        let winner = pick_winner(&as_slices(&bids), "imp1", false).unwrap();
        assert_eq!(winner.bidder.as_str(), "fake_b");
        assert!((winner.price - 2.0_f64).abs() < f64::EPSILON);
    }

    #[test]
    fn tie_keeps_incumbent_via_registry_order() {
        let bids = vec![
            (
                BidderName::new("fake_a"),
                vec![typed_bid("imp1", 1.0, None)],
            ),
            (
                BidderName::new("fake_b"),
                vec![typed_bid("imp1", 1.0, None)],
            ),
        ];
        let winner = pick_winner(&as_slices(&bids), "imp1", false).unwrap();
        // 'a' is the incumbent (lex-smaller); tie keeps it.
        assert_eq!(winner.bidder.as_str(), "fake_a");
    }

    #[test]
    fn prefer_deals_beats_higher_no_deal() {
        let bids = vec![
            (
                BidderName::new("fake_a"),
                vec![typed_bid("imp1", 5.0, None)],
            ),
            (
                BidderName::new("fake_b"),
                vec![typed_bid("imp1", 1.0, Some("D-1"))],
            ),
        ];
        let winner = pick_winner(&as_slices(&bids), "imp1", true).unwrap();
        assert_eq!(winner.bidder.as_str(), "fake_b");
    }
}
