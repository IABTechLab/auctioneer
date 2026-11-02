//! Kargo protocol candidate; source and delivery boundaries live in bidder.toml.

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

pub struct KargoBidder;

pub(crate) fn create_bidder() -> Box<dyn Bidder> {
    Box::new(KargoBidder)
}

impl Bidder for KargoBidder {
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
    #[cfg(not(target_arch = "wasm32"))]
    mod conformance;

    use super::{Bidder as _, DebugBodySchema, KargoBidder};

    #[test]
    fn passthrough_declares_the_existing_openrtb_debug_projection() {
        assert_eq!(KargoBidder.debug_body_schema(), DebugBodySchema::OpenRtb);
    }
}
