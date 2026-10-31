//! `AppNexus` bidder — custom request transform exemplar.

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

pub struct AppnexusBidder;

pub(crate) fn create_bidder() -> Box<dyn Bidder> {
    Box::new(AppnexusBidder)
}

impl Bidder for AppnexusBidder {
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
        use super::{AppnexusBidder, Bidder as _};
        use crate::debug_projection::DebugBodySchema;

        assert_eq!(AppnexusBidder.debug_body_schema(), DebugBodySchema::OpenRtb);
    }
}
