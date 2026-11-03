//! Borrowed Rubicon transforms with explicit regional policy; public promotion remains separate.

mod config;
mod floor;
mod native;
mod parameters;
mod projection;
mod request;
mod response;
mod validation;

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod test_support;

use crate::debug_projection::DebugBodySchema;
use crate::{
    Bidder, BidderError, BidderHttpResponse, BidderRequestInfo, BidderResponse,
    OutboundRequestMeta, OutboundRequestSink,
};
use auctioneer_openrtb::types::bid_request::BidRequest;

pub struct RubiconBidder;

pub(crate) fn create_bidder() -> Box<dyn Bidder> {
    Box::new(RubiconBidder)
}

pub(super) fn validate_config(
    input: crate::BidderConfigInput<'_>,
) -> Result<(), crate::BidderConfigError> {
    use validator::Validate as _;
    config::Config::from(input)
        .validate()
        .map_err(|errors| config::error(&errors))
}

impl Bidder for RubiconBidder {
    fn credential_key<'a>(
        &'a self,
        request: &BidRequest,
        info: &'a BidderRequestInfo<'_>,
    ) -> Result<Option<&'a str>, BidderError> {
        request::credential_key(request, info)
    }

    fn debug_body_schema(&self) -> DebugBodySchema {
        DebugBodySchema::Unsupported
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
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    mod conformance;

    use super::{RubiconBidder, create_bidder};
    use crate::BidderRequestInfo;
    use crate::debug_projection::DebugBodySchema;

    pub(super) fn with_source_context<T>(run: impl FnOnce(&BidderRequestInfo<'_>) -> T) -> T {
        conformance::with_source_context(run)
    }

    #[test]
    fn stateless_factory_keeps_debug_body_unsupported() {
        assert_eq!(size_of::<RubiconBidder>(), 0);
        assert_eq!(
            create_bidder().debug_body_schema(),
            DebugBodySchema::Unsupported
        );
    }
}
