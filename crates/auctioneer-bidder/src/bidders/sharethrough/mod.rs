//! Sharethrough protocol; reviewed source boundaries live in bidder.toml.

mod request;
mod response;

use crate::debug_projection::DebugBodySchema;
use crate::{
    Bidder, BidderError, BidderHttpResponse, BidderRequestInfo, BidderResponse,
    OutboundRequestMeta, OutboundRequestSink,
};
use auctioneer_openrtb::types::bid_request::BidRequest;

pub struct SharethroughBidder;

pub(crate) fn create_bidder() -> Box<dyn Bidder> {
    Box::new(SharethroughBidder)
}

impl Bidder for SharethroughBidder {
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

    use super::{SharethroughBidder, create_bidder};
    use crate::bidders::harness;
    use crate::debug_projection::DebugBodySchema;
    use crate::{BidderHttpRequest, BidderName, BidderRequestInfo, EntryPoint};
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};

    pub(super) const ENDPOINT: &str = "https://bidder.example.test/sharethrough";

    pub(super) fn info() -> BidderRequestInfo<'static> {
        BidderRequestInfo::new(
            BidderName::new("sharethrough"),
            BidderName::new("sharethrough"),
            "account",
            None,
            ENDPOINT,
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

    pub(super) fn banner(id: &str) -> Value {
        json!({"id":id, "banner":{"format":[{"w":300_i32,"h":250_i32}]},
            "ext":{"bidder":{"pkey":"placement"}}})
    }

    pub(super) fn mixed_request() -> BidRequest {
        let mut imp = banner("i");
        imp["video"] = json!({"mimes":["video/mp4"], "w":640_i32,"h":480_i32});
        imp["native"] = json!({"request":"native payload", "ver":"1.2"});
        imp["audio"] = json!({"mimes":["audio/mp4"]});
        request(json!([imp]))
    }

    pub(super) fn calls(request: &BidRequest) -> Vec<BidderHttpRequest> {
        harness::drive_ok(&SharethroughBidder, request, &info())
    }

    #[test]
    fn stateless_factory_declares_openrtb_debug_projection() {
        assert_eq!(size_of::<SharethroughBidder>(), 0);
        assert_eq!(
            create_bidder().debug_body_schema(),
            DebugBodySchema::OpenRtb
        );
    }
}
