//! Owned engine results, independent of their HTTP wire projection.

use crate::AuctionError;
use crate::response::{BidderResultBundle, ResponseInput, build_response};
use crate::targeting::Winner;
use auctioneer_bidder::{BidderName, TypedBid};
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_openrtb::types::bid_response::BidResponse;
use std::collections::BTreeMap;
use std::fmt;

/// The engine's original selection, borrowing the complete retained bid.
pub struct SelectedBid<'a> {
    pub requested_bidder: &'a BidderName,
    pub implementation: Option<&'static str>,
    /// Index in the bidder's full bid vector, not an impression-filtered view.
    pub bid_index: usize,
    pub bid: &'a TypedBid,
}

/// Completed auction facts. Construction and selection are engine-owned.
///
/// Low-level callers remain responsible for retaining their resource funding.
/// Consuming this value projects the existing response without rerunning the
/// reducer, targeting, random rounding or bidder decoding.
pub struct AuctionOutcome {
    request_id: String,
    impression_ids: Vec<String>,
    winners: Vec<Option<Winner>>,
    response: ResponseInput,
    debug: Option<serde_json::Value>,
}

impl AuctionOutcome {
    /// The caller has already admitted identifier and selection storage through
    /// the response bookkeeping budget before reaching this constructor.
    pub(crate) fn new(
        request: &BidRequest,
        response: ResponseInput,
        winners: Option<Vec<Option<Winner>>>,
        debug: Option<serde_json::Value>,
    ) -> Result<Self, AuctionError> {
        if let Some(selected) = &winners {
            let valid = selected.len() == request.imp.len()
                && selected.iter().zip(&request.imp).all(|(selection, imp)| {
                    selection.as_ref().is_none_or(|winner| {
                        response
                            .bundles
                            .get(&winner.bidder)
                            .and_then(|bundle| bundle.bids.get(winner.bid_index))
                            .is_some_and(|bid| bid.bid.impid == imp.id)
                    })
                });
            if !valid {
                return Err(AuctionError::Internal("invalid engine selection".into()));
            }
        }
        Ok(Self {
            request_id: request.id.clone(),
            impression_ids: request.imp.iter().map(|imp| imp.id.clone()).collect(),
            winners: winners.unwrap_or_else(|| vec![None; request.imp.len()]),
            response,
            debug,
        })
    }

    #[must_use]
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    #[must_use]
    pub fn impression_ids(&self) -> &[String] {
        &self.impression_ids
    }

    /// All surviving candidates, including non-winning bids and browser output.
    #[must_use]
    pub fn bidders(&self) -> &BTreeMap<BidderName, BidderResultBundle> {
        &self.response.bundles
    }

    /// Original reducer decisions aligned with `impression_ids()`.
    #[must_use]
    pub fn winners(&self) -> &[Option<Winner>] {
        &self.winners
    }

    #[must_use]
    pub fn selected_bid(&self, impression: usize) -> Option<SelectedBid<'_>> {
        let winner = self.winners.get(impression)?.as_ref()?;
        let (bidder, bundle) = self.response.bundles.get_key_value(&winner.bidder)?;
        Some(SelectedBid {
            requested_bidder: bidder,
            implementation: bundle.implementation,
            bid_index: winner.bid_index,
            bid: bundle.bids.get(winner.bid_index)?,
        })
    }

    /// Borrow typed timing, diagnostics, non-bid rows and browser results.
    #[must_use]
    pub fn response_material(&self) -> &ResponseInput {
        &self.response
    }

    /// Finalized permitted debug, without retaining the clock or capture scope.
    #[must_use]
    pub fn debug(&self) -> Option<&serde_json::Value> {
        self.debug.as_ref()
    }

    /// Move retained bids through the existing deterministic wire builder.
    ///
    /// # Errors
    /// Returns an internal auction error if response extensions cannot serialize.
    pub fn into_bid_response(self) -> Result<BidResponse, AuctionError> {
        let mut response = build_response(self.response)
            .map_err(|error| AuctionError::Internal(format!("build_response: {error}")))?;
        response.id = self.request_id;
        if let Some(debug) = self.debug
            && let Some(ext) = response
                .ext
                .as_mut()
                .and_then(serde_json::Value::as_object_mut)
        {
            ext.insert("debug".into(), debug);
        }
        Ok(response)
    }
}

impl fmt::Debug for AuctionOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuctionOutcome")
            .field("impressions", &self.impression_ids.len())
            .field("bidders", &self.response.bundles.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::AuctionOutcome;
    use crate::AuctionError;
    use crate::response::{BidderResultBundle, ResponseInput};
    use crate::targeting::Winner;
    use auctioneer_bidder::{BidderName, TypedBid};
    use auctioneer_config_data::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_openrtb::types::bid_response::Bid;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn request() -> BidRequest {
        serde_json::from_value(json!({"id":"private-request", "imp":[{"id":"imp1"}]})).unwrap()
    }

    fn response(bids: Vec<TypedBid>) -> ResponseInput {
        ResponseInput {
            bidder_visible_tmax_ms: 700,
            auction_timestamp_ms: 42,
            return_all_bid_status: false,
            seatnonbid: BTreeMap::new(),
            bundles: BTreeMap::from([(
                BidderName::new("fake_alias"),
                BidderResultBundle {
                    implementation: Some("fake_bidder"),
                    bids,
                    dispatched: true,
                    ..BidderResultBundle::default()
                },
            )]),
            construction_diagnostics: BTreeMap::new(),
            prebid_errors: Vec::new(),
            prebid_warnings: Vec::new(),
        }
    }

    fn winner(index: usize) -> Winner {
        Winner {
            bidder: BidderName::new("fake_alias"),
            bid_index: index,
            price: 2.0,
            has_deal: false,
        }
    }

    #[test]
    fn outcome_rejects_invalid_selection_shape_and_coordinates() {
        for selected in [vec![], vec![Some(winner(0))], vec![None, None]] {
            let result =
                AuctionOutcome::new(&request(), response(Vec::new()), Some(selected), None);
            assert!(
                matches!(result, Err(AuctionError::Internal(message)) if message == "invalid engine selection")
            );
        }
        let bid = TypedBid {
            bid: Bid {
                impid: "different-imp".into(),
                ..Bid::default()
            },
            media_type: MediaType::Banner,
            source_seat: None,
        };
        let result = AuctionOutcome::new(
            &request(),
            response(vec![bid]),
            Some(vec![Some(winner(0))]),
            None,
        );
        assert!(
            matches!(result, Err(AuctionError::Internal(message)) if message == "invalid engine selection")
        );
    }

    #[test]
    fn consuming_projection_moves_creative_debug_and_drops_source_only_owners() {
        let source: Arc<str> = Arc::from("private-upstream");
        let weak = Arc::downgrade(&source);
        let bid = TypedBid {
            bid: Bid {
                id: "private-bid".into(),
                impid: "imp1".into(),
                price: 2.0,
                adm: Some("private-creative".repeat(1024)),
                ..Bid::default()
            },
            media_type: MediaType::Banner,
            source_seat: Some(source),
        };
        let creative = bid.bid.adm.as_ref().unwrap().as_ptr();
        let debug = json!({"private-body":"private-value".repeat(1024)});
        let debug_string = debug["private-body"].as_str().unwrap().as_ptr();
        let outcome = AuctionOutcome::new(
            &request(),
            response(vec![bid]),
            Some(vec![Some(winner(0))]),
            Some(debug),
        )
        .unwrap();
        assert_eq!(
            outcome.selected_bid(0).unwrap().implementation,
            Some("fake_bidder")
        );
        assert_eq!(
            outcome.selected_bid(0).unwrap().bid.source_seat.as_deref(),
            Some("private-upstream")
        );
        let diagnostic = format!("{outcome:?}");
        for private in [
            "private-request",
            "private-bid",
            "private-creative",
            "private-upstream",
            "private-body",
            "private-value",
        ] {
            assert!(!diagnostic.contains(private));
        }
        assert!(weak.upgrade().is_some());
        let projected = outcome.into_bid_response().unwrap();
        assert!(weak.upgrade().is_none());
        assert_eq!(projected.id, "private-request");
        assert_eq!(projected.seatbid[0].seat, "fake_alias");
        assert_eq!(
            projected.seatbid[0].bid[0].adm.as_ref().unwrap().as_ptr(),
            creative
        );
        assert_eq!(
            projected.ext.as_ref().unwrap()["debug"]["private-body"]
                .as_str()
                .unwrap()
                .as_ptr(),
            debug_string
        );
        assert_eq!(projected.ext.as_ref().unwrap()["tmaxrequest"], 700_u32);
        assert_eq!(
            projected.ext.as_ref().unwrap()["prebid"]["auctiontimestamp"],
            42_u64
        );
    }

    #[test]
    fn early_exit_retains_owned_ids_without_inventing_a_selection() {
        let outcome = AuctionOutcome::new(&request(), response(Vec::new()), None, None).unwrap();
        assert_eq!(outcome.request_id(), "private-request");
        assert_eq!(outcome.impression_ids(), ["imp1"]);
        assert_eq!(outcome.winners().len(), 1);
        assert!(outcome.winners()[0].is_none());
        assert!(outcome.selected_bid(0).is_none());
        assert!(outcome.selected_bid(1).is_none());
        assert!(outcome.debug().is_none());
        assert!(
            outcome.bidders()[&BidderName::new("fake_alias")]
                .bids
                .is_empty()
        );
        assert_eq!(outcome.response_material().bidder_visible_tmax_ms, 700);
    }
}
