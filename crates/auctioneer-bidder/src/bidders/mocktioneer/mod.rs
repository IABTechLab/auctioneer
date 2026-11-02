//! Mocktioneer bidder — snapshot E2E backend.
//!
//! **Feature-gated.** Mocktioneer points at `http://127.0.0.1` which fails
//! `validate_endpoint`'s `https://`-only + localhost-reject rule in default
//! builds. The relaxation lives behind `#[cfg(feature =
//! "insecure-bidders")]`; in default builds this module does not exist.

mod request;
mod response;

use crate::debug_projection::DebugBodySchema;
use crate::{
    OutboundRequestMeta, OutboundRequestSink,
    bidder::{Bidder, BidderError},
    request_info::BidderRequestInfo,
    types::{BidderHttpResponse, BidderResponse},
};
use auctioneer_openrtb::types::bid_request::BidRequest;

pub struct MocktioneerBidder;

pub(crate) fn create_bidder() -> Box<dyn Bidder> {
    Box::new(MocktioneerBidder)
}

impl Bidder for MocktioneerBidder {
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
        _sent: &OutboundRequestMeta,
        response: &BidderHttpResponse,
    ) -> Result<BidderResponse, Vec<BidderError>> {
        response::make_bids(request, response)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn declares_the_reviewed_debug_body_schema() {
        use super::{Bidder as _, MocktioneerBidder};
        use crate::debug_projection::DebugBodySchema;

        assert_eq!(
            MocktioneerBidder.debug_body_schema(),
            DebugBodySchema::OpenRtb
        );
    }
}
