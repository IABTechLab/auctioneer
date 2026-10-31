//! Generic `OpenRTB` passthrough bidder.

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

pub struct GenericBidder;

pub(crate) fn create_bidder() -> Box<dyn Bidder> {
    Box::new(GenericBidder)
}

impl Bidder for GenericBidder {
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
    use crate::bidders::harness::assert_factory_contract;

    #[test]
    fn declares_the_reviewed_debug_body_schema() {
        use super::{Bidder as _, GenericBidder};
        use crate::debug_projection::DebugBodySchema;

        assert_eq!(GenericBidder.debug_body_schema(), DebugBodySchema::OpenRtb);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn generic_fixture() -> serde_json::Value {
        serde_json::from_str(include_str!("fixtures/factory.json")).unwrap()
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[should_panic(expected = "factory fixture has no cases")]
    fn empty_factory_contract_cannot_pass_without_exercising_the_bidder() {
        let mut fixture = generic_fixture();
        fixture["cases"] = serde_json::json!([]);
        assert_factory_contract("generic", &serde_json::to_string(&fixture).unwrap());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[should_panic(expected = "factory manifest digest")]
    fn factory_contract_rejects_a_replaced_metadata_witness() {
        let mut fixture = generic_fixture();
        fixture["manifest_sha256"] = serde_json::json!("0".repeat(64));
        assert_factory_contract("generic", &serde_json::to_string(&fixture).unwrap());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[should_panic(expected = "factory outbound body")]
    fn factory_contract_compares_independent_wire_expectations() {
        let mut fixture = generic_fixture();
        fixture["cases"][0]["outbound"][0]["body"]["id"] = serde_json::json!("wrong");
        assert_factory_contract("generic", &serde_json::to_string(&fixture).unwrap());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[should_panic(expected = "factory fixture version")]
    fn factory_contract_rejects_an_unknown_fixture_version() {
        let mut fixture = generic_fixture();
        fixture["schema_version"] = serde_json::json!(2_u8);
        assert_factory_contract("generic", &serde_json::to_string(&fixture).unwrap());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[should_panic(expected = "factory fixture owner")]
    fn factory_contract_rejects_another_implementations_fixture() {
        let mut fixture = generic_fixture();
        fixture["implementation"] = serde_json::json!("fake_other");
        assert_factory_contract("generic", &serde_json::to_string(&fixture).unwrap());
    }
}
