//! Unruly protocol candidate; source and delivery boundaries live in bidder.toml.

mod request;
mod response;

use crate::debug_projection::DebugBodySchema;
use crate::{
    Bidder, BidderError, BidderHttpResponse, BidderRequestInfo, BidderResponse,
    OutboundRequestMeta, OutboundRequestSink,
};
use auctioneer_openrtb::types::bid_request::BidRequest;

pub struct UnrulyBidder;

pub(crate) fn create_bidder() -> Box<dyn Bidder> {
    Box::new(UnrulyBidder)
}

impl Bidder for UnrulyBidder {
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

    use super::{BidRequest, BidderRequestInfo, UnrulyBidder, create_bidder};
    use crate::debug_projection::DebugBodySchema;
    use crate::{BidderName, EntryPoint};
    use serde_json::{Value, json};
    use std::mem::size_of;

    pub(super) fn info() -> BidderRequestInfo<'static> {
        BidderRequestInfo::new(
            BidderName::new("unruly-alias"),
            BidderName::new("unruly"),
            "account",
            None,
            "https://bidder.example.test/unruly_prebid_server",
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        )
    }

    pub(super) fn request(impressions: Value) -> BidRequest {
        let mut document = json!({"id":"r"});
        document["imp"] = impressions;
        serde_json::from_value(document).unwrap()
    }

    pub(super) fn parameters(params: Value) -> BidRequest {
        let mut impressions = json!([{"id":"i", "banner":{}, "ext":{}}]);
        impressions[0]["ext"]["bidder"] = params;
        request(impressions)
    }

    #[test]
    fn stateless_factory_declares_the_openrtb_debug_projection() {
        assert_eq!(size_of::<UnrulyBidder>(), 0);
        assert_eq!(
            create_bidder().debug_body_schema(),
            DebugBodySchema::OpenRtb
        );
    }
}
