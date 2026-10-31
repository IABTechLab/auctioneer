//! `OpenX` protocol candidate; source and decoder boundaries live in `bidder.toml`.

mod request;
mod response;

use crate::{
    Bidder, BidderError, BidderHttpResponse, BidderRequestInfo, BidderResponse, DebugBodySchema,
    OutboundRequestMeta, OutboundRequestSink,
};
use auctioneer_openrtb::types::bid_request::BidRequest;

pub struct OpenxBidder;

pub(crate) fn create_bidder() -> Box<dyn Bidder> {
    Box::new(OpenxBidder)
}

impl Bidder for OpenxBidder {
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

    use super::OpenxBidder;
    use crate::Bidder as _;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};

    pub(super) const ENDPOINT: &str = "https://bidder.example.test/prebid";

    pub(super) fn request(impressions: Value) -> BidRequest {
        BidRequest {
            id: "fake_request".into(),
            imp: serde_json::from_value(impressions).unwrap(),
            ..BidRequest::default()
        }
    }

    pub(super) fn imp(id: &str, media: &str) -> Value {
        json!({"id":id, media:{}, "ext":{"bidder":{"unit":"123", "delDomain":"fake.example.test"}}})
    }

    #[test]
    fn debug_body_schema_identifies_openrtb_without_enabling_any_output() {
        assert_eq!(
            OpenxBidder.debug_body_schema(),
            crate::DebugBodySchema::OpenRtb
        );
    }
}
