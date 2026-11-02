//! Yahoo Ads protocol; reviewed source differences live in bidder.toml.

mod request;
mod response;

use crate::{
    Bidder, BidderError, BidderHttpResponse, BidderRequestInfo, BidderResponse, DebugBodySchema,
    OutboundRequestMeta, OutboundRequestSink,
};
use auctioneer_openrtb::types::bid_request::BidRequest;

pub struct YahooAdsBidder;

pub(crate) fn create_bidder() -> Box<dyn Bidder> {
    Box::new(YahooAdsBidder)
}

impl Bidder for YahooAdsBidder {
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

    use super::{YahooAdsBidder, create_bidder};
    use crate::{BidderName, BidderRequestInfo, DebugBodySchema, EntryPoint};
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};
    use std::mem::size_of;

    pub(super) const ENDPOINT: &str = "https://bidder.example.test/yahoo";

    pub(super) fn info() -> BidderRequestInfo<'static> {
        BidderRequestInfo::new(
            BidderName::new("yahooAdvertising"),
            BidderName::new("yahoo_ads"),
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
        json!({
            "id":id, "banner":{"format":[{"w":728,"h":90}]},
            "ext":{"bidder":{"dcn":"site", "pos":"position"}}
        })
    }

    pub(super) fn mixed_request() -> BidRequest {
        request(json!([{
            "id":"i", "banner":{"format":[{"w":728,"h":90}]},
            "video":{"mimes":["video/mp4"], "w":640,"h":360},
            "ext":{"bidder":{"dcn":"site", "pos":"position"}}
        }]))
    }

    #[test]
    fn factory_is_stateless_and_declares_openrtb_debug_projection() {
        assert_eq!(size_of::<YahooAdsBidder>(), 0);
        assert_eq!(
            create_bidder().debug_body_schema(),
            DebugBodySchema::OpenRtb
        );
    }
}
