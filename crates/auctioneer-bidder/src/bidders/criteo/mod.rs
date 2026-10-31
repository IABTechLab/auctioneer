//! Criteo `OpenRTB` protocol and optional browser-auction configurations.

mod request;
mod response;

use crate::{
    Bidder, BidderError, BidderHttpResponse, BidderRequestInfo, BidderResponse, DebugBodySchema,
    OutboundRequestMeta, OutboundRequestSink,
};
use auctioneer_openrtb::types::bid_request::BidRequest;

pub struct CriteoBidder;

pub(crate) fn create_bidder() -> Box<dyn Bidder> {
    Box::new(CriteoBidder)
}

impl Bidder for CriteoBidder {
    fn debug_body_schema(&self) -> DebugBodySchema {
        DebugBodySchema::OpenRtb
    }

    fn make_requests(
        &self,
        request: &BidRequest,
        info: &BidderRequestInfo<'_>,
        sink: &mut OutboundRequestSink<'_>,
    ) -> Result<(), Vec<BidderError>> {
        request::make_requests(request, info, sink);
        Ok(())
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

    use super::DebugBodySchema;

    #[test]
    fn factory_exposes_reviewed_openrtb_debug_projection() {
        assert_eq!(
            super::create_bidder().debug_body_schema(),
            DebugBodySchema::OpenRtb
        );
    }
}
