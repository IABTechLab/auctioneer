//! `Bidder` trait — bidder contract.

use crate::{
    debug_projection::DebugBodySchema,
    meta::OutboundRequestMeta,
    request_info::BidderRequestInfo,
    sink::OutboundRequestSink,
    types::{BidderHttpResponse, BidderResponse},
};
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_runtime::diagnostic::bounded;

/// The bound on one `BidderError` message.
///
/// Sized so the per-bidder error vector stays well inside the
/// assembly allowance even at the retained-error cap: these entries
/// are cloned during response assembly, so the message length is
/// multiplied by the number retained.
const MAX_ERROR_MESSAGE_BYTES: usize = 1_024;

/// Trusted caller context, not a value inferred from request fields or media.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryPoint {
    Auction,
    Amp,
    Video,
}

/// Internal failure provenance; public response codes and messages stay unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BidderErrorKind {
    Other,
    ResponseParse,
}

/// Per-bidder error / warning. Numeric code; severity is
/// caller-decided (the exchange maps code 6 to a warning, everything
/// else to `ext.errors`).
/// Messages are public diagnostics: never include response values or raw
/// parser errors, which can contain bidder payloads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BidderError {
    pub code: u32,
    pub message: String,
    pub kind: BidderErrorKind,
}

impl BidderError {
    /// Every constructor funnels through here, so no call site holds
    /// the bound.
    ///
    /// The shared response parser uses a fixed diagnostic, but every
    /// constructor still needs a bound on bidder-provided messages:
    /// response assembly clones these entries, and the response body
    /// cap cannot bound a string built from that body. This size bound
    /// does not sanitize messages; callers must not include payloads.
    ///
    /// Private, and taking the code positionally, so adding a variant
    /// cannot accidentally reintroduce an unbounded `message`.
    fn bounded_at<S>(code: u32, msg: S) -> Self
    where
        S: Into<String>,
    {
        let raw = msg.into();
        let message = if raw.len() <= MAX_ERROR_MESSAGE_BYTES {
            raw
        } else {
            bounded(&raw, MAX_ERROR_MESSAGE_BYTES)
        };
        Self {
            code,
            message,
            kind: BidderErrorKind::Other,
        }
    }

    #[must_use]
    pub fn bad_input<S>(msg: S) -> Self
    where
        S: Into<String>,
    {
        Self::bounded_at(2, msg)
    }

    #[must_use]
    pub fn bad_server_response<S>(msg: S) -> Self
    where
        S: Into<String>,
    {
        Self::bounded_at(4, msg)
    }

    #[must_use]
    pub fn failed_to_request_bids<S>(msg: S) -> Self
    where
        S: Into<String>,
    {
        Self::bounded_at(5, msg)
    }

    #[must_use]
    pub fn failed_to_unmarshal<S>(msg: S) -> Self
    where
        S: Into<String>,
    {
        Self::bounded_at(14, msg)
    }

    #[must_use]
    pub fn unknown<S>(msg: S) -> Self
    where
        S: Into<String>,
    {
        Self::bounded_at(999, msg)
    }
}

/// Pure, synchronous bidder contract. No async, no I/O, no `EdgeZero` types.
/// The exchange owns the network call between `make_requests` and `make_bids`.
pub trait Bidder: Send + Sync {
    /// Select a configured credential reference after pure parameter preflight.
    /// The exchange, not the bidder, loads its bounded atomic bundle.
    ///
    /// # Errors
    /// Return a fixed bidder-local error when required settings or parameters
    /// cannot produce a request. The default requires no credentials.
    fn credential_key<'a>(
        &'a self,
        _request: &BidRequest,
        _info: &'a BidderRequestInfo<'_>,
    ) -> Result<Option<&'a str>, BidderError> {
        Ok(None)
    }

    /// Only reviewed schema projections may disclose debug body content.
    fn debug_body_schema(&self) -> DebugBodySchema {
        DebugBodySchema::Unsupported
    }

    /// Push this auction's outbound HTTP calls into `sink`.
    ///
    /// The sink, not a returned `Vec`, is the output channel, and that
    /// is the whole contract: `sink.push_request` reserves a call slot
    /// from the auction-global budget BEFORE it allocates either buffer,
    /// so a call the budget cannot fund never has its body written. An
    /// bidder that returned a `Vec` would have built every body before
    /// anything could refuse one, and truncating the result afterwards
    /// is not a construction-time bound.
    ///
    /// `push_request`'s own `Result` is ADVISORY — every rejection is
    /// already recorded in the sink before it is returned, so an
    /// bidder may ignore it and keep pushing, or stop early. The
    /// `Result` THIS method returns is the separate SEMANTIC channel:
    /// business failures that belong to the bidder rather than to one
    /// request, such as a missing required param.
    ///
    /// # Errors
    /// Returns a non-empty `Vec<BidderError>` when the bidder cannot
    /// produce any outbound calls (e.g. missing required params). The
    /// exchange discards whatever the sink accepted in that case, so
    /// every reserved slot is refunded.
    fn make_requests(
        &self,
        request: &BidRequest,
        info: &BidderRequestInfo<'_>,
        sink: &mut OutboundRequestSink<'_>,
    ) -> Result<(), Vec<BidderError>>;

    /// Decode a single bidder HTTP response into a `BidderResponse`.
    ///
    /// `request` is this bidder's post-FPD, privacy-scrubbed auction context,
    /// with the same host `tmax` applied during construction. Classify formats
    /// against that context, not the publisher's unscoped request.
    ///
    /// `sent` is the metadata of the request this response answers —
    /// method, URL, headers and the covered imp ids. It is NOT the
    /// request body: the body was moved into the transport and does not
    /// survive the dispatch. A bidder that needed to re-read its own
    /// request body to parse a response would be re-deriving state it
    /// should have encoded in the request.
    ///
    /// `response`'s body is SEALED. A bidder response is
    /// upstream-controlled JSON of arbitrary structure, and
    /// `BidderHttpResponse` exposes no accessor handing back its
    /// bytes — the only way to read it is
    /// [`BidderHttpResponse::parse_bid_response`], which parses within
    /// the node budget the exchange attached. That is the same
    /// structural bound `make_requests` gets from the sink, in the
    /// other direction: the bidder never holds the budget decision,
    /// so it cannot bypass it.
    ///
    /// # Errors
    /// Returns a non-empty `Vec<BidderError>` when the response is
    /// unparseable / off-protocol; the exchange folds those into
    /// `ext.errors`. Bid-local classification failures must not discard valid
    /// siblings: return them in `BidderResponse::errors` alongside the bids.
    /// If every returned bid is invalid, return their errors in `Err`.
    fn make_bids(
        &self,
        request: &BidRequest,
        sent: &OutboundRequestMeta,
        response: &BidderHttpResponse,
    ) -> Result<BidderResponse, Vec<BidderError>>;
}

#[cfg(test)]
mod tests {
    use super::{
        BidRequest, Bidder, BidderError, BidderHttpResponse, BidderRequestInfo, BidderResponse,
        OutboundRequestMeta, OutboundRequestSink,
    };

    struct PanickyBidder;
    #[expect(
        clippy::missing_trait_methods,
        reason = "this fixture must omit debug_body_schema to exercise its fail-closed default"
    )]
    impl Bidder for PanickyBidder {
        fn make_requests(
            &self,
            _r: &BidRequest,
            _i: &BidderRequestInfo<'_>,
            _s: &mut OutboundRequestSink<'_>,
        ) -> Result<(), Vec<BidderError>> {
            Err(vec![BidderError::failed_to_request_bids("not implemented")])
        }
        fn make_bids(
            &self,
            _r: &BidRequest,
            _q: &OutboundRequestMeta,
            _p: &BidderHttpResponse,
        ) -> Result<BidderResponse, Vec<BidderError>> {
            Err(vec![BidderError::failed_to_unmarshal("not implemented")])
        }
    }

    #[test]
    fn bidder_is_object_safe() {
        fn assert_object_safe(_: &dyn Bidder) {}
        assert_object_safe(&PanickyBidder);
    }

    #[test]
    fn a_bidder_without_a_debug_schema_defaults_to_unsupported() {
        use crate::debug_projection::DebugBodySchema;

        let bidder: &dyn Bidder = &PanickyBidder;
        assert_eq!(bidder.debug_body_schema(), DebugBodySchema::Unsupported);
    }

    #[test]
    fn bidder_error_constructors_have_correct_codes() {
        assert_eq!(BidderError::bad_input("x").code, 2);
        assert_eq!(BidderError::bad_server_response("x").code, 4);
        assert_eq!(BidderError::failed_to_request_bids("x").code, 5);
        assert_eq!(BidderError::failed_to_unmarshal("x").code, 14);
        assert_eq!(BidderError::unknown("x").code, 999);
    }

    #[test]
    fn ordinary_bidder_errors_cannot_be_mistaken_for_response_parse_failures() {
        use super::BidderErrorKind;

        for error in [
            BidderError::bad_input("x"),
            BidderError::bad_server_response("bidder response exceeded the JSON node budget"),
            BidderError::failed_to_request_bids("x"),
            BidderError::failed_to_unmarshal("invalid bidder response"),
            BidderError::unknown("x"),
        ] {
            assert_eq!(error.kind, BidderErrorKind::Other);
        }
    }
}
