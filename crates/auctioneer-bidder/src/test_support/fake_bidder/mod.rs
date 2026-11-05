//! Synthetic bidder capabilities for shared boundary tests. Never registered in a serving build.

mod controls;
mod request;
mod response;

pub use controls::{
    IdentityEnvelope, QueryMode, RequestMode, RequestObserver, ResponseCoverage, factory_calls,
    info,
};

use crate::{
    Bidder, BidderError, BidderHttpResponse, BidderRequestInfo, BidderResponse,
    ConstructionDiagnostic, DebugBodySchema, OutboundRequestMeta, OutboundRequestSink,
};
use auctioneer_openrtb::types::bid_request::BidRequest;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct FakeBidder<'a> {
    pub credential_reference: Option<&'a str>,
    pub construction_observer: Option<&'a (dyn Fn(&BidderRequestInfo<'_>) + Send + Sync)>,
    pub request_observer: Option<&'a RequestObserver>,
    pub credential_preflight_observer: Option<&'a RequestObserver>,
    pub request_mode: RequestMode,
    /// Explicit call coverage for exchange correlation controls, including empty or forged IDs.
    pub request_coverage: Option<Vec<Vec<&'a str>>>,
    pub response_coverage: ResponseCoverage,
    pub query: QueryMode<'a>,
    pub endpoint_values: &'a [(&'a str, &'a str)],
    pub selected_host: Option<&'a str>,
    pub headers: &'a [(&'a str, &'a str)],
    pub method: &'a str,
    pub reject_imp: Option<&'a str>,
    pub semantic_error: bool,
    pub rejected_imp_diagnostic: Option<ConstructionDiagnostic>,
    pub debug_schema: DebugBodySchema,
    pub record_correlation: bool,
    pub correlation_header: Option<&'a [u8]>,
    pub identity_envelope: Option<IdentityEnvelope<'a>>,
    pub metadata_writes: AtomicUsize,
    pub body_writes: AtomicUsize,
}

impl Default for FakeBidder<'_> {
    fn default() -> Self {
        Self {
            credential_reference: None,
            construction_observer: None,
            request_observer: None,
            credential_preflight_observer: None,
            request_mode: RequestMode::Aggregate,
            request_coverage: None,
            response_coverage: ResponseCoverage::SentCall,
            query: QueryMode::Preserve,
            endpoint_values: &[],
            selected_host: None,
            headers: &[],
            method: "POST",
            reject_imp: None,
            semantic_error: false,
            rejected_imp_diagnostic: None,
            debug_schema: DebugBodySchema::OpenRtb,
            record_correlation: false,
            correlation_header: None,
            identity_envelope: None,
            metadata_writes: AtomicUsize::new(0),
            body_writes: AtomicUsize::new(0),
        }
    }
}

#[must_use]
pub fn create_bidder() -> Box<dyn Bidder> {
    controls::record_factory_call();
    Box::<FakeBidder>::default()
}

impl FakeBidder<'_> {
    #[must_use]
    pub fn written_bodies(&self) -> usize {
        self.body_writes.load(Ordering::Relaxed)
    }
}

impl Bidder for FakeBidder<'_> {
    fn credential_key<'a>(
        &'a self,
        request: &BidRequest,
        info: &'a BidderRequestInfo<'_>,
    ) -> Result<Option<&'a str>, BidderError> {
        request::credential_key(self, request, info)
    }

    fn debug_body_schema(&self) -> DebugBodySchema {
        self.debug_schema
    }

    fn make_requests(
        &self,
        request: &BidRequest,
        info: &BidderRequestInfo<'_>,
        sink: &mut OutboundRequestSink<'_>,
    ) -> Result<(), Vec<BidderError>> {
        request::make_requests(self, request, info, sink)
    }

    fn make_bids(
        &self,
        request: &BidRequest,
        sent: &OutboundRequestMeta,
        response: &BidderHttpResponse,
    ) -> Result<BidderResponse, Vec<BidderError>> {
        response::make_bids(self, request, sent, response)
    }
}
