//! Triplelift protocol candidate; serving and migration gates remain separate.

mod request;
mod response;

use crate::{
    Bidder, BidderError, BidderHttpResponse, BidderRequestInfo, BidderResponse, DebugBodySchema,
    OutboundRequestMeta, OutboundRequestSink,
};
use auctioneer_openrtb::types::bid_request::BidRequest;

pub struct TripleliftBidder;

pub(crate) fn create_bidder() -> Box<dyn Bidder> {
    Box::new(TripleliftBidder)
}

impl Bidder for TripleliftBidder {
    fn debug_body_schema(&self) -> DebugBodySchema {
        DebugBodySchema::OpenRtb
    }

    fn make_requests(
        &self,
        request: &BidRequest,
        info: &BidderRequestInfo<'_>,
        sink: &mut OutboundRequestSink<'_>,
    ) -> Result<(), Vec<BidderError>> {
        request::make_requests(request, info, sink)
    }

    fn make_bids(
        &self,
        request: &BidRequest,
        sent: &OutboundRequestMeta,
        response: &BidderHttpResponse,
    ) -> Result<BidderResponse, Vec<BidderError>> {
        response::make_bids(request, sent, response)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(target_arch = "wasm32"))]
    mod conformance;

    use super::{Bidder as _, DebugBodySchema, TripleliftBidder};

    #[test]
    fn transformed_body_uses_the_existing_openrtb_debug_projection() {
        assert_eq!(
            TripleliftBidder.debug_body_schema(),
            DebugBodySchema::OpenRtb
        );
    }
}
