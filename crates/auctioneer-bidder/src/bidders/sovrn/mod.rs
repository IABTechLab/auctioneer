//! Sovrn bidding protocol.

mod request;
mod response;

use crate::debug_projection::DebugBodySchema;
use crate::{
    Bidder, BidderError, BidderHttpResponse, BidderRequestInfo, BidderResponse,
    OutboundRequestMeta, OutboundRequestSink,
};
use auctioneer_openrtb::types::bid_request::BidRequest;

pub struct SovrnBidder;

pub(crate) fn create_bidder() -> Box<dyn Bidder> {
    Box::new(SovrnBidder)
}

impl Bidder for SovrnBidder {
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

    use super::BidRequest;
    use serde_json::{Value, json};

    pub(super) fn request(impressions: Value) -> BidRequest {
        let mut document = json!({"id":"r"});
        document["imp"] = impressions;
        serde_json::from_value(document).unwrap()
    }
}
