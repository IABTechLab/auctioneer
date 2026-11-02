//! `PubMatic` protocol transforms through the shared bounded bidder contract.

mod parameters;
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

pub struct PubmaticBidder;

pub(crate) fn create_bidder() -> Box<dyn Bidder> {
    Box::new(PubmaticBidder)
}

impl Bidder for PubmaticBidder {
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

    use super::PubmaticBidder;
    use crate::bidders::harness;
    use crate::{
        Bidder as _, BidderError, BidderName, BidderRequestInfo, EndpointRejectReason,
        EndpointValidator, EntryPoint, SinkLimits, SinkOutcome,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};
    use url::Url;

    pub(super) struct AllowEndpoint;

    impl EndpointValidator for AllowEndpoint {
        fn validate(
            &self,
            _url: &Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            Ok(())
        }
    }

    pub(super) fn info() -> BidderRequestInfo<'static> {
        BidderRequestInfo::new(
            BidderName::new("pubmatic-requested"),
            BidderName::new("pubmatic"),
            "account",
            None,
            "https://hbopenbid.pubmatic.com/translator?source=prebid-server",
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        )
    }

    pub(super) fn input(impressions: &Value) -> BidRequest {
        serde_json::from_value(json!({"id":"r", "imp":impressions})).unwrap()
    }

    pub(super) fn banner(id: &str, params: &Value) -> Value {
        json!({
            "id":id, "banner":{"format":[{"w":300_i32,"h":250_i32}]},
            "ext":{"bidder":params},
        })
    }

    pub(super) fn drive(request: &BidRequest) -> (Result<(), Vec<BidderError>>, SinkOutcome, u32) {
        harness::drive_with_limits(
            &PubmaticBidder,
            request,
            &info(),
            SinkLimits::new(256 * 1024, 100),
            1,
            &AllowEndpoint,
        )
    }

    pub(super) fn body(request: &BidRequest) -> Value {
        let (semantic, outcome, remaining) = drive(request);
        semantic.unwrap();
        assert!(outcome.errors.is_empty(), "{:?}", outcome.errors);
        assert_eq!(remaining, 0);
        assert_eq!(outcome.accepted.len(), 1);
        serde_json::from_slice(outcome.accepted[0].body()).unwrap()
    }

    #[test]
    fn declares_the_reviewed_debug_body_schema() {
        use crate::debug_projection::DebugBodySchema;
        assert_eq!(PubmaticBidder.debug_body_schema(), DebugBodySchema::OpenRtb);
    }
}
