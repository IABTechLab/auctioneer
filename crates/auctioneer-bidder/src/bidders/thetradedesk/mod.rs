//! The Trade Desk protocol construction and response normalization.

mod config;
mod request;
mod response;

use crate::{
    Bidder, BidderError, BidderHttpResponse, BidderRequestInfo, BidderResponse, DebugBodySchema,
    OutboundRequestMeta, OutboundRequestSink,
};
use auctioneer_openrtb::types::bid_request::BidRequest;

#[cfg(all(test, not(target_arch = "wasm32")))]
use request::invalid_parameters;

pub struct TheTradeDeskBidder;

pub(crate) fn create_bidder() -> Box<dyn Bidder> {
    Box::new(TheTradeDeskBidder)
}

pub(super) fn validate_config(
    input: crate::BidderConfigInput<'_>,
) -> Result<(), crate::BidderConfigError> {
    use validator::Validate as _;
    config::Config::from(input)
        .validate()
        .map_err(config::error)
}

impl Bidder for TheTradeDeskBidder {
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

    use super::TheTradeDeskBidder;
    use crate::BidderSettings;
    use crate::bidders::harness;
    use crate::{BidderHttpRequest, BidderName, BidderRequestInfo, EntryPoint};
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};

    pub(super) const ENDPOINT: &str = "https://bidder.example.test/bid/{{.SupplyId}}";

    pub(super) fn info<'a>(
        requested: Option<&'a BidderSettings>,
        base: Option<&'a BidderSettings>,
    ) -> BidderRequestInfo<'a> {
        let mut info = BidderRequestInfo::new(
            BidderName::new("ttd"),
            BidderName::new("thetradedesk"),
            "account",
            None,
            ENDPOINT,
            &["SupplyId"],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        info.set_settings(Some("thetradedesk"), requested, base)
            .unwrap();
        info
    }

    pub(super) fn request(imps: Value) -> BidRequest {
        let mut document = json!({
            "id":"r",
            "site":{"page":"https://publisher.example/", "publisher":{"id":"existing"}},
        });
        document["imp"] = imps;
        serde_json::from_value(document).unwrap()
    }

    pub(super) fn minimal() -> BidRequest {
        request(json!([{"id":"i", "banner":{},
            "ext":{"bidder":{"publisherId":"publisher", "supplySourceId":"route"}}}]))
    }

    pub(super) fn calls(request: &BidRequest) -> Vec<BidderHttpRequest> {
        harness::drive_ok(&TheTradeDeskBidder, request, &info(None, None))
    }
}
