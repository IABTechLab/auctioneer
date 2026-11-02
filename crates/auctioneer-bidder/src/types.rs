//! Transport-neutral types passed across the `Bidder` trait.

use crate::bidder::{BidderError, BidderErrorKind};
use crate::capped_writer::BodyWriteError;
use crate::correlation::CorrelationError;
use crate::debug_projection::{
    DebugBodyEligibility, DebugBodyProjection, DebugBodySchema, project_response_body,
};
use crate::endpoint_validator::EndpointRejectReason;
use crate::finalize::finalize;
use crate::meta::{MetaField, OutboundRequestMeta, SinkAbort};
use crate::native_adm::{project_value, unwrap_in_place};
use crate::response_decode::{CandidatePolicy, DecodeReceipt, decode_source_with_policy};
use auctioneer_config::types::MediaType;
use auctioneer_openrtb::types::bid_response::{Bid, BidResponse};
use auctioneer_runtime::bounded_json::{self, BoundedJsonError};
use serde_json::Value;
use std::borrow::Borrow;
use std::fmt;
use std::sync::Arc;

/// Interned-style alias-aware bidder identifier. Round-trips through serde
/// as a plain string so it can appear in `BidderInfo.name` and on the wire.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BidderName(Box<str>);

impl BidderName {
    /// Allocate from any owned or borrowed string.
    #[must_use]
    pub fn new<T>(name: T) -> Self
    where
        T: Into<Box<str>>,
    {
        Self(name.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for BidderName {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for BidderName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for BidderName {
    fn from(name: &str) -> Self {
        Self::new(name)
    }
}

/// Outbound HTTP request shape a bidder's `make_requests` produces.
/// Constructed only by the enforcing sink. Transport finalization owns
/// the only mutation, and dispatch consumes the body without copying it.
#[derive(PartialEq, Eq)]
pub struct BidderHttpRequest {
    meta: Arc<OutboundRequestMeta>,
    body: Vec<u8>,
}

impl BidderHttpRequest {
    pub(crate) fn from_sink(meta: OutboundRequestMeta, body: Vec<u8>) -> Self {
        Self {
            meta: Arc::new(meta),
            body,
        }
    }

    pub(crate) fn finalize(
        mut self,
        opts: &crate::TransportFinalize<'_>,
    ) -> (Arc<OutboundRequestMeta>, Result<Self, crate::FinalizeError>) {
        let result = Arc::get_mut(&mut self.meta)
            .ok_or(crate::FinalizeError::ReserveOverflow)
            .and_then(|meta| finalize(meta, &mut self.body, opts));
        let sent = Arc::clone(&self.meta);
        (sent, result.map(|()| self))
    }

    #[must_use]
    pub fn method(&self) -> &str {
        self.meta.method()
    }

    #[must_use]
    pub fn url(&self) -> &str {
        self.meta.url()
    }

    pub fn headers(&self) -> impl Iterator<Item = (&str, &str)> + '_ {
        self.meta.headers()
    }

    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub fn covered_imp_ids(&self) -> impl Iterator<Item = &str> + '_ {
        self.meta.covered_imp_ids()
    }

    #[must_use]
    pub fn meta(&self) -> &OutboundRequestMeta {
        &self.meta
    }

    /// Move the body into transport and retain the same immutable metadata pool.
    #[must_use]
    pub fn into_parts(self) -> (Arc<OutboundRequestMeta>, Vec<u8>) {
        (self.meta, self.body)
    }
}

impl fmt::Debug for BidderHttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BidderHttpRequest")
            .field("header_count", &self.meta.header_count())
            .field("body_bytes", &self.body.len())
            .field("imp_id_count", &self.meta.imp_id_count())
            .finish_non_exhaustive()
    }
}

/// One bidder's HTTP response, with its body sealed.
///
/// `body` is PRIVATE, and that is the whole contract. A bidder
/// receives a `&BidderHttpResponse` and there is no accessor on it that
/// hands back the bytes — the only way to see the content is
/// [`Self::parse_bid_response`], which parses through the node-budgeted
/// parser and refuses a body whose structure exceeds the budget the
/// exchange attached. A bidder author working inside
/// `bidders/<name>/mod.rs` cannot reach the bytes to parse them any other
/// way, so the bound is structural rather than a convention four files
/// must each remember.
///
/// # Why this type is not `Clone`
///
/// It was, and `Clone` defeated the seal outright. The reasoning behind
/// the two escape hatches is that they consume `self`:
/// [`Self::into_body`] moves the bytes out and
/// [`Self::within_node_budget`] replaces the budget, so a bidder
/// holding only a `&` cannot call either and the borrow checker was
/// taken to be the enforcement. `Clone` turns a `&` into an owned
/// value, which makes both reachable from inside a bidder file:
///
/// ```text
/// let bytes   = borrowed.clone().into_body();                   // the sealed bytes
/// let widened = borrowed.clone().within_node_budget(u32::MAX);   // an unlimited budget
/// ```
///
/// Both lines compiled. So the derive is gone, and
/// `response_body_is_sealed.rs` has a compile-fail route for each of
/// them — the borrow-checker argument is only sound while nothing hands
/// out an owned copy, and that is a property of the derive list rather
/// than of the signatures, so it needs a guard of its own.
///
/// A borrowed byte getter would also defeat the seal: its bytes could
/// be copied into [`Self::new`] and given an unlimited budget. Correlation
/// assertions use [`Self::body_equals`] instead, which returns only a
/// boolean and cannot supply the bytes for reconstruction.
///
/// Nothing in production cloned a response. The callers that did were
/// fixtures storing one template response and handing out a copy per
/// call; each now stores the status and body bytes and builds a fresh
/// response, which is what a real transport does anyway.
///
/// This is the deliberate dual of `OutboundRequestSink`. That type
/// exists because a bidder that returned a `Vec` of built requests
/// would have allocated every body before anything could refuse one;
/// this one exists because a bidder handed `&[u8]` would have parsed
/// the whole tree before anything could refuse it. In both directions
/// the enforcing type performs the work and the bidder never holds the
/// budget decision.
///
/// Headers and original media eligibility are immutable together. They are
/// readable, but a later header edit must not reuse evidence about a different
/// original candidate. Debug projection also stays behind the body seal.
///
/// The budget is carried HERE rather than passed to the parse call,
/// because the alternative puts a limit argument in bidder hands —
/// the same bypass in a new spelling — and because `make_bids` keeps
/// its signature, which 16 implementations across five crates would
/// otherwise have had to change.
pub struct BidderHttpResponse {
    status: u16,
    headers: Vec<(String, String)>,
    debug_body_eligibility: DebugBodyEligibility,
    /// Sealed. See the type doc: no accessor returns these bytes.
    body: Vec<u8>,
    /// Syntactic JSON node budget for parsing `body`, attached by the
    /// exchange from `ResolvedIsolateLimits::max_parsed_nodes`.
    ///
    /// A response the transport built carries `0` until the exchange
    /// attaches the real budget at the `make_bids` call site. `0`
    /// refuses every body, so a path that forgot to attach one fails
    /// closed: it cannot parse anything, rather than parsing unbounded.
    max_parsed_nodes: u32,
    decode_receipt: DecodeReceipt,
}

// The original scalar tail occupied eight bytes after alignment. The initial
// claim byte must fit that existing layout, not consume unrelated parse margin.
// A larger receipt requires explicit owning accounting before it can compile.
const _: () = {
    assert!(
        size_of::<DecodeReceipt>() == 1,
        "decode claim must occupy one byte"
    );
    assert!(
        align_of::<DecodeReceipt>() == 1,
        "decode claim must not add alignment"
    );
    assert!(
        size_of::<BidderHttpResponse>()
            <= size_of::<Vec<(String, String)>>()
                + size_of::<Vec<u8>>()
                + size_of::<(u32, u16, DebugBodyEligibility)>(),
        "decode claim exceeds the formerly funded response layout"
    );
};

impl PartialEq for BidderHttpResponse {
    fn eq(&self, other: &Self) -> bool {
        // Decode admission is control state, not part of transport identity.
        self.status == other.status
            && self.headers == other.headers
            && self.debug_body_eligibility == other.debug_body_eligibility
            && self.body == other.body
            && self.max_parsed_nodes == other.max_parsed_nodes
    }
}

impl Eq for BidderHttpResponse {}

impl fmt::Debug for BidderHttpResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Formatting must not provide an alternate route to the sealed bytes.
        f.debug_struct("BidderHttpResponse")
            .field("status", &self.status)
            .field("header_count", &self.headers.len())
            .field("body_bytes", &self.body.len())
            .field("max_parsed_nodes", &self.max_parsed_nodes)
            .finish_non_exhaustive()
    }
}

impl BidderHttpResponse {
    /// Build a response from what a transport received.
    ///
    /// The node budget starts at `0` — refuse-everything — because a
    /// transport does not know the host's limits. The exchange attaches
    /// the real budget with [`Self::within_node_budget`] before any
    /// bidder sees the response.
    #[must_use]
    pub const fn new(status: u16, headers: Vec<(String, String)>, body: Vec<u8>) -> Self {
        Self {
            status,
            headers,
            debug_body_eligibility: DebugBodyEligibility::Ineligible,
            body,
            max_parsed_nodes: 0,
            decode_receipt: DecodeReceipt::new(),
        }
    }

    /// Seal transport-provided original header evidence with the delivered bytes.
    ///
    /// The caller must derive eligibility before any lossy header normalization.
    /// A normalized textual list alone is not evidence; [`Self::new`] therefore
    /// defaults to ineligible. Missing or ambiguous delivered candidates can only
    /// narrow supplied evidence, never authorize capture.
    #[must_use]
    pub fn from_transport(
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
        eligibility: DebugBodyEligibility,
    ) -> Self {
        let delivered = DebugBodyEligibility::from_complete_headers(
            headers
                .iter()
                .map(|(name, value)| (name.as_str(), Some(value.as_str()))),
        );
        let mut response = Self::new(status, headers, body);
        if eligibility == DebugBodyEligibility::SingleTextValue
            && delivered == DebugBodyEligibility::SingleTextValue
        {
            response.debug_body_eligibility = DebugBodyEligibility::SingleTextValue;
        }
        response
    }

    /// Produce only the fixed sanitized projection within the attached node cap.
    /// The caller must first pass the account/base-bidder debug gates. There is
    /// no raw-byte callback or caller-supplied wider parsing budget.
    #[must_use]
    pub fn project_debug_body(&self, schema: DebugBodySchema) -> DebugBodyProjection {
        project_response_body(
            schema,
            &self.body,
            &self.headers,
            self.debug_body_eligibility,
            self.max_parsed_nodes,
        )
    }

    /// Attach the node budget this response's body may be parsed within.
    ///
    /// Called by the exchange, where the host's resolved limits are in
    /// scope. Takes `self` by value so attaching a budget is a
    /// transformation of the response rather than a mutation a bidder
    /// holding a `&` could reach. Once any decode attempt claims the receipt,
    /// the original limit is frozen, including after a failed or unbudgeted
    /// attempt. A later attachment changes neither that limit nor the receipt.
    #[must_use]
    pub fn within_node_budget(mut self, max_parsed_nodes: u32) -> Self {
        if self.decode_receipt.permits_limit_attachment() {
            self.max_parsed_nodes = max_parsed_nodes;
        }
        self
    }

    /// The response status.
    #[must_use]
    pub const fn status(&self) -> u16 {
        self.status
    }

    /// The response headers, in receipt order with duplicates kept.
    #[must_use]
    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    /// Whether the response carried no body at all.
    ///
    /// A length, not content: every bidder needs this for its "no
    /// bids" early return, and it cannot be used to read or retain
    /// anything.
    #[must_use]
    pub const fn body_is_empty(&self) -> bool {
        self.body.is_empty()
    }

    /// The body's size in bytes, for the decoded-response cap.
    ///
    /// A count, not content. `classify` in `auctioneer-core` compares this
    /// against `max_decoded_bytes` BEFORE any bidder runs.
    #[must_use]
    pub const fn body_len(&self) -> usize {
        self.body.len()
    }

    /// The node budget attached to this response, for a test or a
    /// diagnostic that needs to state it.
    #[must_use]
    pub const fn max_parsed_nodes(&self) -> u32 {
        self.max_parsed_nodes
    }

    /// The body bytes, for a caller that must move them WITHOUT
    /// parsing.
    ///
    /// Not reachable from a bidder, because a bidder is handed a
    /// `&BidderHttpResponse` and this consumes `self` by value. That
    /// argument holds only while the type hands out no owned copy of
    /// itself, which is why `Clone` is not derived — see the type doc.
    /// Transport owners use this to relocate a body without parsing it.
    #[must_use]
    pub fn into_body(self) -> Vec<u8> {
        self.body
    }

    /// Whether the body exactly matches caller-supplied bytes.
    ///
    /// Correlation assertions can inspect repeated results without
    /// extracting bytes to parse or reconstruct under a wider budget.
    #[must_use]
    pub fn body_equals(&self, expected: &[u8]) -> bool {
        self.body == expected
    }

    /// Parse the body into a `BidResponse` within the attached node
    /// budget.
    ///
    /// The whole-response standard DTO route. A byte cap alone does
    /// not bound what a parse RETAINS — `{"a":{"a":…}}` fits a few KiB
    /// while expanding into one heap node per level, and `Bid`'s
    /// `#[serde(flatten)]` catch-all means the inflation need not even
    /// be off-schema.
    ///
    /// # Errors
    ///
    /// An over-budget body is a code-4 `BadServerResponse` (Appendix G:
    /// "bidder returned an unparseable response") — the response's
    /// STRUCTURE is off-protocol, which is the class the spec already
    /// assigns `OversizeResponse`. A malformed or off-schema body is
    /// code-14 `FailedToUnmarshal`, which is the decode failure it
    /// actually is.
    /// Both classes use fixed diagnostics without echoing upstream values
    /// or third-party parser messages.
    /// The first attempt consumes this response's decode claim before parsing;
    /// subsequent attempts return a fixed code-4 error, even after failure.
    ///
    /// The budget message names neither the budget nor the body: the
    /// count is a server sizing decision and the body is
    /// upstream-controlled, so the inbound path's pinned-message rule
    /// applies here too.
    ///
    /// A refused raw-value capture joins the over-budget case rather
    /// than the decode case. It means the TARGET TYPE asked for a span
    /// this parser cannot charge, so nothing is known about the body's
    /// structure — which is the over-budget verdict, reached before any
    /// counting rather than after it. `BidResponse` carries its `ext`
    /// as a `serde_json::Value` and so cannot raise it today; the arm
    /// exists so a future field spelled `Box<RawValue>` fails closed on
    /// a code the wire already defines instead of needing this match
    /// revisited.
    pub fn parse_bid_response(&self) -> Result<BidResponse, Vec<BidderError>> {
        self.decode_receipt.claim().map_err(|error| vec![error])?;
        bounded_json::from_slice_within(&self.body, self.max_parsed_nodes)
            .map_err(|err| vec![response_parse_error(&err)])
    }

    /// Decode standard response candidates, isolating malformed bid extensions.
    ///
    /// This closed decoder returns only the existing `BidResponse` DTO and
    /// fixed bid-local parse errors. Standard-field errors refuse the entire
    /// source. One source pass and one aggregate candidate pass share the
    /// attached node cap, with a 64-container source guard and checked byte
    /// allowances derived from the delivered length. No raw spans are returned.
    ///
    /// # Errors
    /// Structural/work/depth refusals return fixed code 4; syntax and standard
    /// DTO failures return fixed code 14. Every error has `ResponseParse`
    /// provenance. Any attempt spends the same receipt as `parse_bid_response`.
    pub fn parse_bid_candidates(
        &self,
    ) -> Result<(BidResponse, Vec<BidderError>), Vec<BidderError>> {
        self.parse_bid_candidates_with_policy(CandidatePolicy::OptionalExtension)
    }

    pub(crate) fn parse_bid_candidates_with_policy(
        &self,
        policy: CandidatePolicy,
    ) -> Result<(BidResponse, Vec<BidderError>), Vec<BidderError>> {
        self.parse_bid_candidates_with_prefilter(policy, |_bid| true)
    }

    /// Check owner DTO fields before local extension refusals discard candidates.
    /// The predicate borrows the existing typed Bid; it cannot select a decoder.
    pub(crate) fn parse_bid_candidates_with_prefilter(
        &self,
        policy: CandidatePolicy,
        prefilter: fn(&Bid) -> bool,
    ) -> Result<(BidResponse, Vec<BidderError>), Vec<BidderError>> {
        self.decode_receipt.claim().map_err(|error| vec![error])?;
        let decoded =
            decode_source_with_policy(&self.body, self.max_parsed_nodes, policy, prefilter)?;
        Ok(decoded.into_parts())
    }

    /// Validate an embedded native creative and remove its optional `native`
    /// wrapper in place. The delivered body's byte length and attached node
    /// ceiling bound this operation; no second owned JSON tree is retained.
    /// Normal Serde numeric-range and nesting checks apply to the full creative.
    ///
    /// # Errors
    /// Returns a fixed bid-local error for invalid or over-budget creative JSON.
    /// A refusal leaves the original string unchanged.
    pub fn unwrap_native_adm(&self, adm: &mut String) -> Result<(), BidderError> {
        unwrap_in_place(adm, self.body.len(), self.max_parsed_nodes)
    }

    /// Encode already-decoded creative JSON within this response's byte/node caps.
    /// No response bytes or second owned JSON tree are exposed.
    ///
    /// # Errors
    /// Returns a fixed bid-local error for an excessive creative projection.
    pub fn project_adm_value(&self, value: &Value) -> Result<String, BidderError> {
        project_value(value, self.body.len(), self.max_parsed_nodes)
    }
}

fn response_parse_error(error: &BoundedJsonError) -> BidderError {
    let mut diagnostic = match error {
        BoundedJsonError::NodeBudget(_) | BoundedJsonError::RawValue(_) => {
            BidderError::bad_server_response("bidder response exceeded the JSON node budget")
        }
        BoundedJsonError::Syntax(_) => BidderError::failed_to_unmarshal("invalid bidder response"),
    };
    diagnostic.kind = BidderErrorKind::ResponseParse;
    diagnostic
}

/// Why the sink refused to accept one outbound request.
///
/// Every variant is PER-REQUEST and non-fatal to that bidder's siblings:
/// the sink rolls back exactly this request's call reservation, records
/// the error, and construction continues. The semantic channel —
/// business failures a bidder reports through its own `Result` — is
/// separate and unaffected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutboundBuildError {
    /// An invalid call-local protocol context; no rejected data is retained.
    Correlation(CorrelationError),
    /// The raw body crossed `max_outbound_raw_bytes`.
    OversizeRaw { limit: u64 },
    /// A metadata append crossed the meta byte pool's bidder region.
    MetaOverCap { limit: u64 },
    /// A header name the transport or the exchange owns.
    ReservedHeader { name: String },
    /// The auction-global call budget had no slot left. Recorded once per
    /// sink; the ATTEMPT count lives in `SinkOutcome.rejected_call_count`.
    CallBudgetExhausted { max_calls: u32 },
    /// The deployment's endpoint policy refused the finalized URL.
    EndpointRejected { reason: EndpointRejectReason },
    /// The finalized URL did not parse, so no policy could be applied.
    MalformedUrl,
    /// `build_metadata` returned `Ok` without calling `set_method`.
    MissingMethod,
    /// `build_metadata` returned `Ok` without calling `set_url`.
    MissingUrl,
    /// A second `set_method` or `set_url`.
    DuplicateMeta { field: MetaField },
    /// Past `MAX_BIDDER_HEADERS` bidder headers.
    TooManyHeaders { limit: u16 },
    /// Past `max_impressions_per_auction` covered imp ids.
    TooManyImpIds { limit: u16 },
    /// A body write failed for a reason that is not the cap.
    Body(BodyWriteError),
}

impl From<SinkAbort> for OutboundBuildError {
    fn from(abort: SinkAbort) -> Self {
        match abort {
            SinkAbort::Correlation(error) => Self::Correlation(error),
            SinkAbort::MetaOverCap { limit } => Self::MetaOverCap { limit },
            SinkAbort::ReservedHeader { name } => Self::ReservedHeader { name },
            SinkAbort::TooManyHeaders { limit } => Self::TooManyHeaders { limit },
            SinkAbort::TooManyImpIds { limit } => Self::TooManyImpIds { limit },
            SinkAbort::DuplicateMeta { field } => Self::DuplicateMeta { field },
            SinkAbort::Body(err) => Self::from(err),
        }
    }
}

impl From<BodyWriteError> for OutboundBuildError {
    fn from(err: BodyWriteError) -> Self {
        match err {
            // The writer's cap IS `max_outbound_raw_bytes`, so an
            // over-cap body write is the spec's raw-stage oversize.
            BodyWriteError::OverCap { limit } => Self::OversizeRaw { limit },
            other @ (BodyWriteError::Serialize | BodyWriteError::UnderCapacity { .. }) => {
                Self::Body(other)
            }
        }
    }
}

/// Failure channel of the buffered proxy send path. Co-located with
/// `BidderHttpRequest` / `BidderHttpResponse` so both the portable
/// `ProxyClientApi` boundary and the hosting integrations can name it.
///
/// The single variant is deliberate, not vestigial. The host's error
/// type carries no timeout / connect kind and every bidder collapses
/// transport failures into one opaque error, so auctioneer cannot currently
/// tell transport-failure kinds apart. Saying that in the type removes
/// the substring guessing the earlier classifier relied on, and leaves
/// an additive extension point for when the host surfaces richer
/// gateway errors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProxyError {
    /// Any transport / decode failure from `send_one`. The string is
    /// DIAGNOSTIC ONLY — no code path matches on its content, and it is
    /// never logged nor placed on the wire.
    Transport(String),
}

/// What a bidder's `make_bids` returns. Currency is the seatbid currency
/// the bidder saw; MVP enforces USD in the exchange.
///
/// `BidderResponse` and `TypedBid` deliberately do NOT derive `PartialEq` —
/// `auctioneer_openrtb::types::bid_response::Bid` does not (it carries
/// `serde_json::Value` slots which are not Eq-comparable byte-for-byte).
/// Comparisons in tests go through `serde_json::to_string` round-trips.
#[derive(Clone, Debug)]
pub struct BidderResponse {
    pub bids: Vec<TypedBid>,
    pub currency: String,
    /// Bid-local failures; valid siblings remain eligible for the auction.
    pub errors: Vec<BidderError>,
    /// Optional browser-auction payloads; the exchange supplies authoritative
    /// bidder/adapter identities and validates exact sent-call participation.
    pub auction_configs: Vec<BrowserAuctionConfig>,
}

/// An optional configuration moved from a node-budgeted bidder response.
/// Provider-controlled identity labels are deliberately absent.
#[derive(Clone)]
pub struct BrowserAuctionConfig {
    pub imp_id: String,
    pub config: serde_json::Value,
}

impl fmt::Debug for BrowserAuctionConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserAuctionConfig")
            .finish_non_exhaustive()
    }
}

/// A bid plus its classified media type, validated against its impression.
#[derive(Clone, Debug)]
pub struct TypedBid {
    pub bid: Bid,
    pub media_type: MediaType,
}

#[cfg(test)]
mod tests {
    use super::{
        BidderHttpResponse, BidderName, BodyWriteError, BrowserAuctionConfig, MetaField,
        OutboundBuildError, SinkAbort,
    };
    use crate::test_support;
    use std::borrow::Borrow;
    use std::hash::{DefaultHasher, Hash as _, Hasher as _};
    use std::ptr;
    use std::sync::Arc;

    #[test]
    fn browser_configuration_debug_never_discloses_identity_or_payload() {
        let record = BrowserAuctionConfig {
            imp_id: "IMPRESSION_CANARY".into(),
            config: serde_json::json!({"secret":"CONFIG_CANARY"}),
        };
        let diagnostic = format!("{record:?}");
        assert!(!diagnostic.contains("IMPRESSION_CANARY"));
        assert!(!diagnostic.contains("CONFIG_CANARY"));
    }

    #[test]
    fn bidder_name_borrow_agrees_with_owned_identity() {
        let names = [
            "",
            "fake_ax",
            "fake_AX",
            "fake_caf\u{e9}",
            "fake_cafe\u{301}",
            "fake_\u{6771}\u{4eac}",
        ]
        .map(BidderName::new);

        for name in &names {
            let borrowed = <BidderName as Borrow<str>>::borrow(name);
            assert!(ptr::eq(borrowed, name.as_str()));

            let mut owned_hash = DefaultHasher::new();
            name.hash(&mut owned_hash);
            let mut borrowed_hash = DefaultHasher::new();
            borrowed.hash(&mut borrowed_hash);
            assert_eq!(owned_hash.finish(), borrowed_hash.finish());

            for other in &names {
                let other_borrowed = <BidderName as Borrow<str>>::borrow(other);
                assert_eq!(name == other, borrowed == other_borrowed);
                assert_eq!(name.cmp(other), borrowed.cmp(other_borrowed));
            }
        }
    }

    #[test]
    fn response_body_equals_compares_exact_bytes_without_consuming() {
        let body = b"\0\xffbody";
        let response = BidderHttpResponse::new(200, Vec::new(), body.to_vec());
        assert!(response.body_equals(body));
        assert!(!response.body_equals(b"\0\xffBody"));
        assert!(!response.body_equals(b"\0\xffbod"));
        assert!(!response.body_equals(b"\0\xffbody!"));
        assert!(!response.body_equals(b""));
        assert!(response.body_equals(body));
        assert_eq!(response.into_body(), body);
    }

    #[test]
    fn response_body_equals_matches_an_empty_body() {
        let response = BidderHttpResponse::new(204, Vec::new(), Vec::new());
        assert!(response.body_equals(b""));
        assert!(!response.body_equals(b"\0"));
        assert!(response.body_is_empty());
        assert_eq!(response.body_len(), 0);
    }

    #[test]
    fn debug_projection_requires_carried_header_evidence_and_the_installed_budget() {
        use crate::debug_projection::{DebugBodyEligibility, DebugBodySchema, DebugBodyStatus};

        let body = serde_json::to_vec(&serde_json::json!({
            "id": "private-response-id", "cur": "USD", "secret-key": "secret-value"
        }))
        .unwrap();
        let headers = vec![("content-type".into(), "application/json".into())];
        let unknown =
            BidderHttpResponse::new(200, headers.clone(), body.clone()).within_node_budget(100);
        let omitted = unknown.project_debug_body(DebugBodySchema::OpenRtb);
        assert_eq!(omitted.body_status, DebugBodyStatus::OmittedType);
        assert_eq!(omitted.body, None);
        let trusted = BidderHttpResponse::from_transport(
            200,
            headers,
            body.clone(),
            DebugBodyEligibility::SingleTextValue,
        )
        .within_node_budget(100);
        let captured = trusted.project_debug_body(DebugBodySchema::OpenRtb);
        assert_eq!(captured.body_status, DebugBodyStatus::Captured);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&captured.body.unwrap()).unwrap(),
            serde_json::json!({"id":"[REDACTED]", "cur":"USD"}),
        );
        assert!(trusted.body_equals(&body));
        let narrowed = trusted.within_node_budget(1);
        let refused = narrowed.project_debug_body(DebugBodySchema::OpenRtb);
        assert_eq!(refused.body_status, DebugBodyStatus::OmittedStructure);
        assert_eq!(refused.body, None);
        assert_eq!(narrowed.max_parsed_nodes(), 1);
    }

    #[test]
    fn debug_projection_cannot_upgrade_an_ambiguous_delivered_header_list() {
        use crate::debug_projection::{DebugBodyEligibility, DebugBodySchema, DebugBodyStatus};

        let body = serde_json::to_vec(&serde_json::json!({"id":"private"})).unwrap();
        for headers in [
            vec![],
            vec![("x-type".into(), "application/json".into())],
            vec![
                ("content-type".into(), "application/json".into()),
                ("Content-Type".into(), "application/json".into()),
            ],
        ] {
            let response = BidderHttpResponse::from_transport(
                200,
                headers,
                body.clone(),
                DebugBodyEligibility::SingleTextValue,
            )
            .within_node_budget(100);
            let result = response.project_debug_body(DebugBodySchema::OpenRtb);
            assert_eq!(result.body_status, DebugBodyStatus::OmittedType);
            assert_eq!(result.body, None);
        }
        let empty = BidderHttpResponse::new(204, Vec::new(), Vec::new());
        let result = empty.project_debug_body(DebugBodySchema::Unsupported);
        assert_eq!(result.body_status, DebugBodyStatus::Empty);
        assert_eq!(result.body, None);
    }

    #[test]
    fn response_body_equals_preserves_the_attached_node_budget() {
        let body = serde_json::to_vec(&serde_json::json!({
            "id": "response",
            "seatbid": []
        }))
        .expect("serialize response");
        let response = BidderHttpResponse::new(200, Vec::new(), body.clone()).within_node_budget(1);
        assert!(response.body_equals(&body));
        assert!(!response.body_equals(b"{}"));
        assert_eq!(response.max_parsed_nodes(), 1);
        let errors = response
            .parse_bid_response()
            .expect_err("equality must not widen the parse budget");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 4);
        assert!(response.body_equals(&body));
        assert_eq!(response.into_body(), body);
    }

    #[test]
    fn a_marker_in_an_invalid_price_remains_an_unmarshal_error() {
        let body = serde_json::to_vec(&serde_json::json!({
            "id": "response",
            "seatbid": [{"seat": "fake_bidder", "bid": [{
                "id": "bid", "impid": "imp",
                "price": "auctioneer-runtime/bounded_json: unbudgetable raw value"
            }]}]
        }))
        .expect("response fixture serializes");
        let response = BidderHttpResponse::new(200, Vec::new(), body).within_node_budget(100);
        let errors = response
            .parse_bid_response()
            .expect_err("price must be numeric");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 14);
    }

    #[test]
    fn candidate_prefilter_refusal_spends_the_shared_response_receipt() {
        use crate::BidderErrorKind;
        use crate::response_decode::CandidatePolicy;

        let body = serde_json::to_vec(&serde_json::json!({
            "id":"r","seatbid":[{"bid":[{"id":"b","impid":"i","price":1_i32}]}]
        }))
        .unwrap();
        let response = BidderHttpResponse::new(200, Vec::new(), body).within_node_budget(100);
        let errors = response
            .parse_bid_candidates_with_prefilter(CandidatePolicy::OptionalExtension, |_bid| false)
            .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 14);
        assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
        let widened_response = response.within_node_budget(u32::MAX);
        for later_errors in [
            widened_response.parse_bid_response().unwrap_err(),
            widened_response.parse_bid_candidates().unwrap_err(),
        ] {
            assert_eq!(later_errors.len(), 1);
            assert_eq!(later_errors[0].code, 4);
            assert_eq!(later_errors[0].kind, BidderErrorKind::ResponseParse);
        }
    }

    #[test]
    fn response_parser_failures_keep_typed_provenance_and_public_diagnostics() {
        use crate::{BidderError, BidderErrorKind};

        let cases = [
            (b"{".to_vec(), 100, 14, "invalid bidder response"),
            (
                serde_json::to_vec(&serde_json::json!({"id":"r","seatbid":"invalid"})).unwrap(),
                100,
                14,
                "invalid bidder response",
            ),
            (
                serde_json::to_vec(&serde_json::json!({"id":"r","seatbid":[]})).unwrap(),
                1,
                4,
                "bidder response exceeded the JSON node budget",
            ),
        ];
        for (body, nodes, code, message) in cases {
            let response = BidderHttpResponse::new(200, Vec::new(), body).within_node_budget(nodes);
            let errors = response.parse_bid_response().unwrap_err();
            assert_eq!(errors.len(), 1);
            let error = &errors[0];
            assert_eq!(error.kind, BidderErrorKind::ResponseParse);
            assert_eq!(error.code, code);
            assert_eq!(error.message, message);
            let ordinary = if code == 4 {
                BidderError::bad_server_response(message)
            } else {
                BidderError::failed_to_unmarshal(message)
            };
            assert_eq!(ordinary.code, code);
            assert_eq!(ordinary.message, message);
            assert_eq!(ordinary.kind, BidderErrorKind::Other);
        }
    }

    #[test]
    fn refused_raw_value_parse_has_response_parser_provenance() {
        use crate::BidderErrorKind;
        use auctioneer_runtime::bounded_json;
        use serde_json::value::RawValue;

        let raw = bounded_json::from_slice_within::<Box<RawValue>>(b"{}", 100).unwrap_err();
        assert!(matches!(raw, bounded_json::BoundedJsonError::RawValue(_)));
        let error = super::response_parse_error(&raw);
        assert_eq!(error.kind, BidderErrorKind::ResponseParse);
        assert_eq!(error.code, 4);
        assert_eq!(
            error.message,
            "bidder response exceeded the JSON node budget"
        );
    }

    #[test]
    fn response_debug_cannot_recover_sealed_body_or_headers() {
        let body = serde_json::to_vec(&serde_json::json!({"secret": "sealed-body-sentinel"}))
            .expect("body fixture");
        let response = BidderHttpResponse::new(
            200,
            vec![("set-cookie".into(), "credential-sentinel".into())],
            body.clone(),
        )
        .within_node_budget(17);
        assert_eq!(
            format!("{response:?}"),
            format!(
                "BidderHttpResponse {{ status: 200, header_count: 1, body_bytes: {}, max_parsed_nodes: 17, .. }}",
                body.len()
            )
        );
        assert_eq!(
            format!("{response:#?}"),
            format!(
                "BidderHttpResponse {{\n    status: 200,\n    header_count: 1,\n    body_bytes: {},\n    max_parsed_nodes: 17,\n    ..\n}}",
                body.len()
            )
        );
    }

    #[test]
    fn request_debug_omits_payload_url_headers_and_identifiers() {
        let mut request = test_support::request(&[("authorization", "header-sentinel")]);
        request.body = b"body-sentinel".to_vec();
        assert_eq!(
            format!("{request:?}"),
            "BidderHttpRequest { header_count: 1, body_bytes: 13, imp_id_count: 1, .. }"
        );
        assert_eq!(
            format!("{request:#?}"),
            "BidderHttpRequest {\n    header_count: 1,\n    body_bytes: 13,\n    imp_id_count: 1,\n    ..\n}"
        );
    }

    #[test]
    fn bidder_name_is_ord_for_registry_ordering() {
        let mut names = [
            BidderName::new("fake_zzz"),
            BidderName::new("fake_one"),
            BidderName::new("fake_two"),
        ];
        names.sort();
        assert_eq!(names[0].as_str(), "fake_one");
        assert_eq!(names[1].as_str(), "fake_two");
        assert_eq!(names[2].as_str(), "fake_zzz");
    }

    #[test]
    fn bidder_http_request_preserves_headers_order() {
        let request = test_support::request(&[("X-A", "1"), ("X-B", "2"), ("X-A", "3")]);
        assert_eq!(
            request.headers().collect::<Vec<_>>(),
            [("X-A", "1"), ("X-B", "2"), ("X-A", "3")]
        );
    }

    #[test]
    fn bidder_http_request_clone_preserves_headers_order() {
        let request = test_support::request(&[("X-A", "1"), ("X-B", "2"), ("X-A", "3")]);
        // Requests cannot clone their bodies; only retained metadata is shared.
        let (meta, _body) = request.into_parts();
        let cloned = Arc::clone(&meta);
        assert_eq!(
            cloned.headers().collect::<Vec<_>>(),
            meta.headers().collect::<Vec<_>>()
        );
    }

    /// Every `SinkAbort` reaches the exchange as its own build error —
    /// no variant collapses onto another, so a code-5 entry names the
    /// real cause.
    #[test]
    fn sink_abort_maps_variant_for_variant_onto_build_error() {
        let cases = [
            (
                SinkAbort::MetaOverCap { limit: 3_328 },
                OutboundBuildError::MetaOverCap { limit: 3_328 },
            ),
            (
                SinkAbort::ReservedHeader {
                    name: "Host".to_owned(),
                },
                OutboundBuildError::ReservedHeader {
                    name: "Host".to_owned(),
                },
            ),
            (
                SinkAbort::TooManyHeaders { limit: 32 },
                OutboundBuildError::TooManyHeaders { limit: 32 },
            ),
            (
                SinkAbort::TooManyImpIds { limit: 100 },
                OutboundBuildError::TooManyImpIds { limit: 100 },
            ),
            (
                SinkAbort::DuplicateMeta {
                    field: MetaField::Url,
                },
                OutboundBuildError::DuplicateMeta {
                    field: MetaField::Url,
                },
            ),
            (
                SinkAbort::Body(BodyWriteError::Serialize),
                OutboundBuildError::Body(BodyWriteError::Serialize),
            ),
        ];
        for (abort, expected) in cases {
            assert_eq!(OutboundBuildError::from(abort), expected);
        }
    }

    /// An over-cap body write is the raw-stage oversize the resource
    /// limits table owns, not an opaque body failure.
    #[test]
    fn over_cap_body_write_becomes_oversize_raw() {
        assert_eq!(
            OutboundBuildError::from(BodyWriteError::OverCap { limit: 0x0004_0000 }),
            OutboundBuildError::OversizeRaw { limit: 0x0004_0000 }
        );
        assert_eq!(
            OutboundBuildError::from(SinkAbort::Body(BodyWriteError::OverCap { limit: 4 })),
            OutboundBuildError::OversizeRaw { limit: 4 }
        );
        assert_eq!(
            OutboundBuildError::from(BodyWriteError::UnderCapacity {
                capacity: 1,
                cap: 2
            }),
            OutboundBuildError::Body(BodyWriteError::UnderCapacity {
                capacity: 1,
                cap: 2
            })
        );
    }
}
