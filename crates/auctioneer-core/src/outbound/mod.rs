//! The bounded outbound facade: auctioneer's own view of one bidder call,
//! from deadline composition through dispatch to a classified slot.
//!
//! Everything here is auctioneer-owned. The types carry the caps auctioneer
//! charges against its isolate budget, the ONE absolute deadline instant
//! it composed, and the timeout cause it selected while composing — none
//! of which `EdgeZero` knows about. [`BoundedFetch`] is the thin bridge
//! that turns them into `edgezero_core::outbound::OutboundRequest`s and
//! lifts the results back.
//!
//! # Why the deadline is a single stored instant
//!
//! A per-call budget could be expressed as a relative timeout handed to
//! the platform, but a relative timeout RE-ANCHORS every time it is
//! dispatched: a call that loses gate contention and rides a later wave
//! would silently receive its full budget again. So composition happens
//! ONCE, at fanout entry ([`wave::compose_all`]), producing an absolute
//! [`ComposedDeadline`] that later waves race unchanged.
//!
//! # Why the cause travels with the call
//!
//! The upstream `Deadline` carries an instant and nothing else, so when
//! a slot expires upstream there is no way to ask which of the two
//! budgets — the per-bidder timeout or the whole-auction deadline — ran
//! out first. auctioneer answers that at composition, where both operands
//! are in hand, and stores the answer in
//! [`ComposedDeadline::selected_cause`]. Equality selects
//! [`TimeoutCause::Bidder`].

pub mod wave;

use async_trait::async_trait;
use auctioneer_bidder::debug_projection::DebugBodyEligibility;
use auctioneer_bidder::{BidderHttpRequest, BidderHttpResponse};
use auctioneer_exchange::{TimeoutCause, UndecodableEncoding};
use auctioneer_runtime::{MonotonicInstant, ResolvedIsolateLimits, brotli_state_bytes};
use edgezero_core::body::Body;
use edgezero_core::compression::{ContentEncoding, PassthroughReason, classify_content_encoding};
use edgezero_core::error::EdgeError;
use edgezero_core::http::{HeaderMap, Method, Uri};
use edgezero_core::outbound::{
    ContentTypeProvenance, HttpClient, OutboundBatch, OutboundCachePolicy, OutboundRequest,
    OutboundResponse, OutboundSlotResult,
};
use edgezero_core::time::Deadline;
use std::fmt;
use std::num::NonZeroU64;
use std::time::Duration;

/// Largest chunk auctioneer lets the transport hand a decoder at once.
///
/// `EdgeZero`'s builder takes a `NonZeroU64`, so the constant is stored in
/// that width rather than converted at every call site. The workspace
/// denies `unwrap`/`expect`/`panic` in non-test code, so the `match`
/// below is how a `NonZeroU64` constant is spelled; the assertion under
/// it makes the unreachable arm a compile error rather than a silent
/// substitution.
pub const MAX_CHUNK_BYTES: NonZeroU64 = match NonZeroU64::new(8 * 1024) {
    Some(bytes) => bytes,
    None => NonZeroU64::MIN,
};

const _: () = assert!(
    MAX_CHUNK_BYTES.get() == 8 * 1024,
    "MAX_CHUNK_BYTES fell through to its unreachable arm"
);

/// Summed name + value bytes auctioneer admits from one bidder's response
/// header block.
pub const MAX_RESPONSE_HEADER_BYTES: u32 = 8_192;

/// Header entries auctioneer admits from one bidder's response.
pub const MAX_RESPONSE_HEADER_COUNT: u32 = 64;

/// What a slot's `elapsed` actually measures on this target.
///
/// The distinction is load-bearing for `responsetimemillis`: a target
/// that measures from batch entry to completion reports the bidder's own
/// latency, while one that harvests slots serially in input order reports
/// OBSERVATION time, so a fast bidder behind a slow one looks slow.
/// Neither is wrong; a consumer just has to know which it has.
///
/// These profiles describe observed outcomes. An SDK timeout can instead
/// carry its logical expiry instant, including an earlier own deadline
/// observed after it expired. Forward that authoritative duration unchanged;
/// it is not a sample of when Auctioneer polled or when host work stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElapsedSemantics {
    /// From the batch's shared entry snapshot to this slot becoming
    /// terminal. Earlier-own-deadline expiry receipts use logical expiry.
    CompletionTime,
    /// From the batch's entry snapshot to this slot being OBSERVED,
    /// which on a serially-harvesting target includes its predecessors.
    /// Earlier-own-deadline expiry receipts instead use logical expiry;
    /// pre-expiry typed failures retain their observed terminal time.
    ObservationTime,
}

/// Documented support level, not an enforcement or negotiation mechanism.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupportLevel {
    /// The platform advertises native enforcement of this capability.
    Native,
    /// auctioneer asks and the platform may not deliver. The gap is
    /// declared, not silently papered over: auctioneer guarantees only its
    /// own half — that nothing late is published as on-time.
    BestEffort,
}

/// Static documentation and expectation-test data for an adapter's capabilities.
///
/// Each adapter owns a constant in `limits.rs`. Profiles are not installed at
/// boot or consumed by the scheduler, budget, or response classifier. Runtime
/// policy comes from resolved limits and upstream response metadata; elapsed
/// durations pass through unchanged. This descriptor adds no SDK guarantees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetOutboundProfile {
    /// What this target's per-slot `elapsed` measures.
    pub elapsed_semantics: ElapsedSemantics,
    /// Whether the platform honors the per-request deadline auctioneer sets.
    pub deadline_support: SupportLevel,
    /// Whether dropping the batch provably stops in-flight work.
    pub cancellation_support: SupportLevel,
}

/// Which cap-ownership row produced a response body.
///
/// The row is upstream's own `ContentEncoding` classification, adopted
/// rather than re-derived. `EdgeZero` decodes a single bare `br`, `gzip`,
/// or `deflate` and strips `Content-Encoding` when decoding is permitted;
/// a partial response retains its original coding and bytes instead.
/// auctioneer consumes that type, because a second parse of the same header
/// is guaranteed to drift from it — and did: it classified `deflate`,
/// which upstream now decodes, as still-encoded.
///
/// The distinction is not cosmetic. A [`Self::ReadyToParse`] body is
/// finished work a parser can consume. A [`Self::RawPassthrough`] body
/// is one NO decoder available to auctioneer can finish, which is why it
/// carries a cause rather than a pending decode: see
/// [`UndecodableEncoding`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CapRow {
    /// The body is plain bytes a parser can consume directly.
    ReadyToParse,
    /// The body is still encoded and undecodable here; the cause names
    /// why.
    RawPassthrough(UndecodableEncoding),
}

/// The per-call response caps auctioneer charges its isolate budget for.
///
/// Both byte caps are present and both are enforced: the transport cap
/// bounds what crosses the wire, the decoded cap bounds what a decode
/// can expand it into. A single cap cannot express that, because a
/// compressed response is small on the wire and large in memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResponseBodyPolicy {
    /// Compressed on-the-wire cap.
    pub max_transport_bytes: u64,
    /// Post-decode cap.
    pub max_decoded_bytes: u64,
    /// Largest brotli window auctioneer will decode.
    pub max_brotli_wbits: u8,
    /// Decode layers a stacked `Content-Encoding` may carry.
    pub max_brotli_layers: u8,
    /// Decoder heap `EdgeZero` may allocate for ANY coding it decodes —
    /// brotli, gzip, or deflate. An INDEPENDENT upstream limit, set to
    /// the same figure the isolate budget charges.
    ///
    /// The figure is derived from the brotli window because brotli is
    /// the most expensive decoder upstream allocates; it therefore
    /// bounds the cheaper flate states with room to spare, and is not
    /// brotli-specific despite its derivation.
    pub max_decoder_bytes: u64,
    /// Summed name + value response-header bytes.
    pub max_header_bytes: u32,
    /// Response-header entry count.
    pub max_header_count: u32,
    /// Largest chunk handed to a decoder at once.
    pub max_chunk_bytes: NonZeroU64,
}

/// The decode caps that travel WITH a slot.
///
/// A separate type from [`ResponseBodyPolicy`] because of ownership, not
/// taste: the transport conversion CONSUMES the call, and auctioneer's
/// decode runs after the slot comes back. Copying the decode-relevant
/// subset out at composition is what lets a per-slot cap — rather than
/// some batch-wide value — reach the decoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodePolicy {
    /// Post-decode cap for THIS slot.
    pub max_decoded_bytes: u64,
    /// Largest brotli window for THIS slot.
    pub max_brotli_wbits: u8,
    /// Decode layers for THIS slot.
    pub max_brotli_layers: u8,
    /// Decoder heap for THIS slot, for any coding.
    pub max_decoder_bytes: u64,
    /// Largest chunk for THIS slot.
    pub max_chunk_bytes: NonZeroU64,
}

impl From<&ResponseBodyPolicy> for DecodePolicy {
    fn from(policy: &ResponseBodyPolicy) -> Self {
        Self {
            max_decoded_bytes: policy.max_decoded_bytes,
            max_brotli_wbits: policy.max_brotli_wbits,
            max_brotli_layers: policy.max_brotli_layers,
            max_decoder_bytes: policy.max_decoder_bytes,
            max_chunk_bytes: policy.max_chunk_bytes,
        }
    }
}

/// One call's absolute deadline plus the cause auctioneer selected for it.
///
/// ONE instant is stored, not a pair of candidate instants: two fields
/// required to stay consistent invite drift, and the upstream
/// `Deadline` is DERIVED from this instant at dispatch — same clock
/// domain, no conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComposedDeadline {
    /// `min(fanout_entry_at + call.timeout, auction.deadline_at)`, on
    /// the one application monotonic clock.
    pub deadline_at: MonotonicInstant,
    /// Which budget produced `deadline_at`. Equality selects
    /// [`TimeoutCause::Bidder`].
    pub selected_cause: TimeoutCause,
}

/// One bidder call with everything auctioneer composed for it.
pub struct BoundedOutboundRequest {
    /// The already-finalized outbound request.
    pub request: BidderHttpRequest,
    /// This call's response caps.
    pub policy: ResponseBodyPolicy,
    /// This call's absolute deadline and stored cause.
    pub deadline: ComposedDeadline,
}

/// Why one slot failed, in auctioneer's own three-way taxonomy.
///
/// Three arms because there are three genuinely different owners of a
/// failure, and collapsing any of them into the others loses the fact a
/// classifier needs. A preflight failure is AUCTIONEER's and nothing was
/// sent; an expiry is AUCTIONEER's own clock over AUCTIONEER's own work; an
/// [`Self::Edge`] is upstream's terminal report, adopted verbatim.
#[derive(Debug)]
pub enum BoundedSlotError {
    /// auctioneer's conversion rejected the request at the `http` layer.
    /// Nothing reached the wire and no header was silently dropped.
    ///
    /// Distinct from [`Self::OversizeEncodedRequest`] even though both
    /// mean "nothing was sent": this one is a request auctioneer could not
    /// REPRESENT, which the spec's wire table maps to code 4, while the
    /// other is a request auctioneer could represent and had no budget to
    /// carry, which is code 5. Collapsing them cost both their codes.
    Preflight,
    /// The final encoded body crossed `max_outbound_request_bytes` in
    /// `finalize_for_transport`.
    ///
    /// The spec's `OversizeRequest { stage: Encoded }`: a positional
    /// per-call result, not a pre-fanout per-bidder entry — the
    /// construction-time `Raw` stage is already the sink's
    /// `OutboundBuildError::OversizeRaw`, reported before the fan-out
    /// exists. Both render code 5; the stage is what says where in the
    /// response the entry lands.
    OversizeEncodedRequest,
    /// auctioneer's own absolute check over AUCTIONEER-OWNED WORK — the wave
    /// wait and the slot settle. There is deliberately no
    /// post-batch and no publication recheck: an upstream slot outcome
    /// is TERMINAL, because a slow sibling must never convert an
    /// on-time bidder into a timeout.
    Expired,
    /// The bidder answered within every cap, in an encoding no decoder
    /// on this path can read.
    ///
    /// auctioneer's own verdict rather than an upstream failure: upstream
    /// reports a successful read and hands the bytes over encoded, and
    /// it is auctioneer that decides an unreadable body is not a response.
    /// Carries the cause upstream typed — see [`CapRow`].
    Undecodable(UndecodableEncoding),
    /// The upstream per-slot failure, adopted as reported.
    Edge(EdgeError),
}

/// A buffered bidder response that cleared every cap.
///
/// `Ok` means the WHOLE body was collected: there is no partial-body
/// success, because a truncated bid response is not a bid response.
pub struct BoundedProxyResponse {
    /// Upstream status.
    status: u16,
    /// Response headers, after upstream normalization.
    headers: HeaderMap,
    content_type_provenance: ContentTypeProvenance,
    /// The collected body.
    body: Vec<u8>,
    /// Which cap-ownership row produced `body`.
    cap_row: CapRow,
}

impl BoundedProxyResponse {
    /// Read headers without detaching or mutating their original provenance.
    ///
    /// ```compile_fail
    /// use auctioneer_core::outbound::BoundedProxyResponse;
    /// fn replace(response: &mut BoundedProxyResponse) {
    ///     response.headers.clear();
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use auctioneer_core::outbound::BoundedProxyResponse;
    /// fn detach(response: &BoundedProxyResponse) {
    ///     let _ = response.content_type_provenance;
    /// }
    /// ```
    #[must_use]
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    fn into_bidder_response(self) -> BidderHttpResponse {
        seal_bidder_response(
            self.status,
            &self.headers,
            self.body,
            self.content_type_provenance,
        )
    }
}

impl fmt::Debug for BoundedProxyResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoundedProxyResponse")
            .field("status", &self.status)
            .field("header_count", &self.headers.len())
            .field("body_bytes", &self.body.len())
            .finish_non_exhaustive()
    }
}

/// One slot's terminal outcome and the time it took.
///
/// `elapsed` is present on BOTH arms, and it is upstream's definition
/// adopted verbatim: from ONE batch-entry snapshot to this slot becoming
/// terminal, including upstream validation and preparation. The
/// [`StreamGate`](auctioneer_runtime::StreamGate) wave wait is excluded
/// because it happens before the batch is constructed.
#[derive(Debug)]
pub struct BoundedSlotResult {
    /// Time from batch entry to this slot becoming terminal.
    pub elapsed: Duration,
    /// This slot's terminal response or typed failure.
    pub outcome: Result<BoundedProxyResponse, BoundedSlotError>,
}

impl BoundedSlotResult {
    /// The positional result an unresolved-at-cutoff slot becomes.
    ///
    /// A cutoff is not a contract breach — the observation window simply
    /// closed — so the slot owes an outcome and gets the one auctioneer
    /// composed a cause for.
    #[must_use]
    pub fn expired(elapsed: Duration) -> Self {
        Self {
            elapsed,
            outcome: Err(BoundedSlotError::Expired),
        }
    }
}

/// The bounded outbound seam the wave scheduler drives.
///
/// Object-safe and `?Send`, matching the pinned upstream client. There
/// is no default implementation: a target that cannot start a batch
/// cannot host a fanout.
#[async_trait(?Send)]
pub trait BoundedProxyClientApi {
    /// Starts one wave's batch under one absolute observation cutoff.
    ///
    /// Returns the upstream handle rather than a collected vector, so
    /// the caller can observe each slot as it becomes terminal and
    /// overlap decode with siblings still in flight.
    ///
    /// `observation_cutoff` carries upstream's contract unchanged: it
    /// stops batch observation regardless of any remaining per-request
    /// budget or later per-request deadline, so an earlier value
    /// intentionally leaves unresolved slots.
    ///
    /// # Errors
    /// A per-slot conversion failure is reported through `preflight`
    /// rather than this method: the valid siblings still dispatch.
    fn start_wave(
        &self,
        requests: Vec<OutboundRequest>,
        observation_cutoff: Deadline,
    ) -> Result<OutboundBatch, EdgeError>;
}

/// [`BoundedProxyClientApi`] over `EdgeZero`'s own outbound client.
///
/// Holds the client and the resolved limits, so every number it charges
/// comes from [`ResolvedIsolateLimits`] and none from a target profile:
/// a profile describes what a platform CAN do, a limit set describes
/// what this isolate is allowed to spend.
pub struct BoundedFetch {
    client: HttpClient,
    limits: ResolvedIsolateLimits,
}

impl BoundedFetch {
    /// Wraps an outbound client with the limits it must charge against.
    #[must_use]
    pub fn new(client: HttpClient, limits: ResolvedIsolateLimits) -> Self {
        Self { client, limits }
    }

    /// The limits every conversion reads its caps from.
    #[must_use]
    pub fn limits(&self) -> &ResolvedIsolateLimits {
        &self.limits
    }
}

#[async_trait(?Send)]
impl BoundedProxyClientApi for BoundedFetch {
    fn start_wave(
        &self,
        requests: Vec<OutboundRequest>,
        observation_cutoff: Deadline,
    ) -> Result<OutboundBatch, EdgeError> {
        Ok(self.client.start_batch_until(requests, observation_cutoff))
    }
}

/// Builds the response policy one call carries from the isolate limits.
///
/// Every field is set. Nothing is left at an `EdgeZero` default, because a
/// default is a number the isolate budget did not charge for.
#[must_use]
pub fn policy_from_limits(limits: &ResolvedIsolateLimits) -> ResponseBodyPolicy {
    ResponseBodyPolicy {
        max_transport_bytes: limits.max_transport_bytes,
        max_decoded_bytes: limits.max_decoded_bytes,
        max_brotli_wbits: limits.max_brotli_wbits,
        max_brotli_layers: limits.max_brotli_layers,
        max_decoder_bytes: brotli_state_bytes(limits.max_brotli_wbits)
            .unwrap_or(limits.max_decoded_bytes),
        max_header_bytes: MAX_RESPONSE_HEADER_BYTES,
        max_header_count: MAX_RESPONSE_HEADER_COUNT,
        max_chunk_bytes: MAX_CHUNK_BYTES,
    }
}

/// Collapses any conversion failure into the one typed preflight error.
///
/// The four fallible steps report four unrelated error types, and every
/// one of them means the same thing to a caller: auctioneer could not
/// represent this request, so nothing was sent.
fn preflight<E>(_err: E) -> BoundedSlotError {
    BoundedSlotError::Preflight
}

/// Converts one composed call into the platform's outbound request.
///
/// Consumes the call: the body is MOVED into the request body, so the
/// only body allocation on this path is the sink's original one.
///
/// # Errors
/// [`BoundedSlotError::Preflight`] when the method, URI, or any header
/// cannot be represented at the `http` layer. Nothing is sent, and no
/// header is silently removed — dispatching a request with a header
/// dropped would send a different request from the one the adapter
/// built.
pub fn to_outbound(
    call: BoundedOutboundRequest,
    limits: &ResolvedIsolateLimits,
) -> Result<OutboundRequest, BoundedSlotError> {
    let BoundedOutboundRequest {
        request,
        policy,
        deadline,
    } = call;
    let (meta, body) = request.into_parts();
    let method: Method = meta.method().parse().map_err(preflight)?;
    let uri: Uri = meta.url().parse().map_err(preflight)?;
    let mut outbound = OutboundRequest::new(method, uri)
        .map_err(preflight)?
        .body(Body::from_bytes(body));
    for (name, value) in meta.headers() {
        outbound = outbound.header(name, value).map_err(preflight)?;
    }
    Ok(outbound
        // ONE precomposed absolute instant per call. Deliberately NOT
        // `.timeout()`: a relative timeout re-anchors when a later wave
        // dispatches, which would both extend the budget and corrupt the
        // cause attribution auctioneer already stored.
        .deadline(Deadline::at_instant(deadline.deadline_at))
        // A bid response is auction-specific and carries single-use
        // pricing and user signals. Serving one from a platform cache
        // would be a correctness bug AND a cross-request privacy leak,
        // and `PlatformDefault` — what an unset policy inherits — allows
        // exactly that. Every bidder call sets Bypass.
        .cache_policy(OutboundCachePolicy::Bypass)
        .max_request_body_bytes(limits.max_outbound_request_bytes)
        .max_encoded_response_bytes(policy.max_transport_bytes)
        .max_decoded_response_bytes(policy.max_decoded_bytes)
        .max_response_header_bytes(u64::from(policy.max_header_bytes))
        .max_response_header_count(u64::from(policy.max_header_count))
        .max_brotli_window_bits(policy.max_brotli_wbits)
        .max_decoder_bytes(policy.max_decoder_bytes)
        .max_chunk_bytes(policy.max_chunk_bytes)
        // Buffered mode's FINAL collection limit is the max of the two
        // caps: it bounds only the buffered memory, while each cap stays
        // independently enforced during the read.
        .max_response_bytes(policy.max_transport_bytes.max(policy.max_decoded_bytes)))
}

/// Lifts one upstream slot result into auctioneer's taxonomy.
///
/// The upstream outcome is TAKEN AS TERMINAL. An expiry upstream
/// observed already arrives as its own typed error, and auctioneer adds no
/// recheck on top: the batch yields each slot as it resolves, so a slow
/// sibling has no way to retroactively rewrite an on-time one.
pub async fn lift_slot(slot: OutboundSlotResult) -> BoundedSlotResult {
    let OutboundSlotResult {
        elapsed, outcome, ..
    } = slot;
    let lifted = match outcome {
        Ok(response) => lift_response(response).await,
        Err(err) => Err(BoundedSlotError::Edge(err)),
    };
    BoundedSlotResult {
        elapsed,
        outcome: lifted,
    }
}

/// Collects one upstream response into a bounded, cap-row-tagged body.
async fn lift_response(
    response: OutboundResponse,
) -> Result<BoundedProxyResponse, BoundedSlotError> {
    let cap_row = cap_row_of(response.headers());
    let (_method, status, headers, body, content_type_provenance) = response.into_parts();
    // Upstream already enforced this call's encoded and decoded caps
    // during the read, so the collection bound here only has to be wide
    // enough not to re-cap what it already bounded.
    let bytes = body
        .into_bytes_bounded(usize::MAX)
        .await
        .map_err(BoundedSlotError::Edge)?;
    Ok(BoundedProxyResponse {
        status: status.as_u16(),
        headers,
        content_type_provenance,
        body: bytes.to_vec(),
        cap_row,
    })
}

/// Only the SDK can attest the original field list. Auctioneer's textual conversion
/// may narrow that evidence further, but must never reconstruct it after loss.
pub(crate) fn seal_bidder_response(
    status: u16,
    headers: &HeaderMap,
    body: Vec<u8>,
    provenance: ContentTypeProvenance,
) -> BidderHttpResponse {
    let eligibility = if matches!(provenance, ContentTypeProvenance::SingleTextUnchanged) {
        DebugBodyEligibility::from_complete_headers(
            headers
                .iter()
                .map(|(name, value)| (name.as_str(), value.to_str().ok())),
        )
    } else {
        DebugBodyEligibility::Ineligible
    };
    let text_headers = headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|text| (name.as_str().to_owned(), text.to_owned()))
        })
        .collect();
    BidderHttpResponse::from_transport(status, text_headers, body, eligibility)
}

/// Adopts upstream's encoding classification as the cap-ownership row.
///
/// The header is classified ONCE, by `EdgeZero`, and read here. auctioneer
/// does not re-parse `Content-Encoding`: upstream's classifier is what
/// selected the decoder, so a second parse here could only agree by
/// accident and disagree by construction.
///
/// Only `Identity` is ready to parse. A surviving recognized coding is
/// original content, including the SDK's raw `206` path; successful SDK
/// decoding strips that header. Auctioneer does not run another decoder or
/// mislabel a recognized codec as unsupported.
///
/// # Why a passthrough is terminal rather than pending
///
/// Every surviving [`PassthroughReason`] names a body auctioneer cannot
/// finish either. A `Stacked` chain is one upstream declined to layer;
/// an `Unsupported` token is a coding no decoder in this workspace
/// implements; a `Malformed` value names no coding at all. None becomes
/// decodable by adding a decoder here, so the row carries the refusal
/// cause and the slot reports it, rather than deferring work that has
/// no implementation to wait for.
fn cap_row_of(headers: &HeaderMap) -> CapRow {
    match classify_content_encoding(headers) {
        ContentEncoding::Identity => CapRow::ReadyToParse,
        ContentEncoding::Brotli | ContentEncoding::Deflate | ContentEncoding::Gzip => {
            CapRow::RawPassthrough(UndecodableEncoding::Unclassified)
        }
        ContentEncoding::Passthrough(reason) => {
            CapRow::RawPassthrough(undecodable_encoding(reason))
        }
    }
}

/// Maps upstream's passthrough reason onto auctioneer's own cause.
///
/// [`PassthroughReason`] is `#[non_exhaustive]`, so the wildcard arm is
/// unavoidable. It lands on [`UndecodableEncoding::Unclassified`] rather
/// than on the nearest named cause, for the reason `oversize_origin` in
/// [`wave`] gives: a reason a future platform release adds must read as
/// "undecodable, cause auctioneer cannot name", never as a cause it is not.
///
/// The named reasons are listed individually rather than grouped so that
/// a rename upstream is a compile error here.
fn undecodable_encoding(reason: PassthroughReason) -> UndecodableEncoding {
    match reason {
        PassthroughReason::Malformed => UndecodableEncoding::Malformed,
        PassthroughReason::Stacked => UndecodableEncoding::Stacked,
        // The token MOVES out of upstream's classification rather than
        // being copied: it is already a bounded, lowercased HTTP token
        // and nothing else reads it.
        PassthroughReason::Unsupported(token) => UndecodableEncoding::Unsupported(token),
        _ => UndecodableEncoding::Unclassified,
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::outbound_request;

    use super::{
        BoundedOutboundRequest, CapRow, ComposedDeadline, DecodePolicy, MAX_CHUNK_BYTES,
        MAX_RESPONSE_HEADER_BYTES, MAX_RESPONSE_HEADER_COUNT, ResponseBodyPolicy, cap_row_of,
        to_outbound, wave,
    };
    use auctioneer_bidder::BidderHttpRequest;
    use auctioneer_exchange::{TimeoutCause, UndecodableEncoding};
    use auctioneer_runtime::{MonotonicInstant, ResolvedIsolateLimits};
    use edgezero_core::http::HeaderMap;
    use edgezero_core::outbound::{ContentTypeProvenance, OutboundCachePolicy};
    use std::num::NonZeroU64;
    use std::time::Duration;

    pub(crate) fn test_policy() -> ResponseBodyPolicy {
        ResponseBodyPolicy {
            max_transport_bytes: 256 * 1024,
            max_decoded_bytes: 256 * 1024,
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            max_decoder_bytes: 20_971_520,
            max_header_bytes: MAX_RESPONSE_HEADER_BYTES,
            max_header_count: MAX_RESPONSE_HEADER_COUNT,
            max_chunk_bytes: MAX_CHUNK_BYTES,
        }
    }

    fn limits() -> ResolvedIsolateLimits {
        wave::tests::test_limits()
    }

    fn normalize_fixture_headers(original: HeaderMap) -> (HeaderMap, ContentTypeProvenance) {
        use edgezero_core::http::{Method, StatusCode};
        use edgezero_core::outbound::{
            OutboundHeaderFidelity, OutboundHeaderInput, OutboundProxyMarker, ResponseHeaderLimiter,
        };

        let (normalized, facts, _) = OutboundHeaderInput::new(
            "fake",
            original,
            OutboundHeaderFidelity::AdapterVisibleFields,
        )
        .normalize(
            ResponseHeaderLimiter::new(Some(4096), Some(32)),
            &Method::POST,
            StatusCode::OK,
            OutboundProxyMarker::Off,
        )
        .unwrap();
        (normalized, facts.content_type_provenance)
    }

    #[test]
    fn original_content_type_evidence_survives_lifting_and_text_filtering() {
        use auctioneer_bidder::debug_projection::{DebugBodySchema, DebugBodyStatus};
        use edgezero_core::body::Body;
        use edgezero_core::http::{HeaderValue, Method, StatusCode};
        use edgezero_core::outbound::OutboundResponse;
        use edgezero_core::time::MonotonicClock;
        use futures::executor::block_on;

        let body = serde_json::to_vec(&serde_json::json!({"id":"secret", "cur":"USD"})).unwrap();
        let cases: &[(&[&[u8]], bool, bool, DebugBodyStatus)] = &[
            (
                &[b"application/json"],
                false,
                false,
                DebugBodyStatus::Captured,
            ),
            (&[], false, false, DebugBodyStatus::OmittedType),
            (
                &[b"application/json", b"application/json"],
                false,
                false,
                DebugBodyStatus::OmittedType,
            ),
            (
                &[b"application/json", b"text/plain"],
                false,
                false,
                DebugBodyStatus::OmittedType,
            ),
            (
                &[b"application/json", b"\xff"],
                false,
                false,
                DebugBodyStatus::OmittedType,
            ),
            (
                &[b"application/json; note=\"\xc3\xa9\""],
                false,
                false,
                DebugBodyStatus::OmittedType,
            ),
            (
                &[b"application/json"],
                true,
                false,
                DebugBodyStatus::OmittedType,
            ),
            (
                &[b"application/json"],
                false,
                true,
                DebugBodyStatus::OmittedType,
            ),
        ];
        for (values, unknown, mutate, expected) in cases {
            let mut headers = HeaderMap::new();
            for value in *values {
                headers.append("content-type", HeaderValue::from_bytes(value).unwrap());
            }
            let (normalized_headers, normalized_proof) = normalize_fixture_headers(headers);
            let proof = if *unknown {
                ContentTypeProvenance::Unknown
            } else {
                normalized_proof
            };
            let mut outbound = OutboundResponse::new(
                Method::POST,
                StatusCode::OK,
                normalized_headers,
                Body::from_bytes(body.clone()),
                MonotonicClock::new(MonotonicInstant::now),
                proof,
            );
            if *mutate {
                outbound
                    .headers_mut()
                    .insert("x-observer", HeaderValue::from_static("changed"));
            }
            let response = block_on(super::lift_response(outbound))
                .unwrap()
                .into_bidder_response()
                .within_node_budget(100);
            let projection = response.project_debug_body(DebugBodySchema::OpenRtb);
            assert_eq!(projection.body_status, *expected);
            assert!(response.body_equals(&body));
            if *expected == DebugBodyStatus::Captured {
                assert_eq!(
                    serde_json::from_str::<serde_json::Value>(&projection.body.unwrap()).unwrap(),
                    serde_json::json!({"id":"[REDACTED]", "cur":"USD"}),
                );
            } else {
                assert_eq!(projection.body, None);
            }
        }
    }

    #[test]
    fn every_sdk_ineligibility_reason_stays_closed_with_json_looking_headers() {
        use auctioneer_bidder::debug_projection::{DebugBodySchema, DebugBodyStatus};
        use edgezero_core::outbound::{ContentTypeIneligibility, ContentTypeProvenance};

        let body = serde_json::to_vec(&serde_json::json!({"id":"private"})).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert("content-type", "application/json".parse().unwrap());
        let reasons = [
            ContentTypeIneligibility::Missing,
            ContentTypeIneligibility::Modified,
            ContentTypeIneligibility::MultipleValues,
            ContentTypeIneligibility::NonText,
            ContentTypeIneligibility::Removed,
        ];
        for reason in reasons {
            let response = super::seal_bidder_response(
                200,
                &headers,
                body.clone(),
                ContentTypeProvenance::Ineligible(reason),
            )
            .within_node_budget(100);
            let projection = response.project_debug_body(DebugBodySchema::OpenRtb);
            assert_eq!(
                projection.body_status,
                DebugBodyStatus::OmittedType,
                "{reason:?}"
            );
            assert_eq!(projection.body, None);
            assert!(response.body_equals(&body));
        }
    }

    #[test]
    fn bounded_response_debug_keeps_body_and_header_values_private() {
        use edgezero_core::outbound::ContentTypeProvenance;

        let response = super::BoundedProxyResponse {
            status: 200,
            headers: HeaderMap::from_iter([(
                "x-secret".parse().unwrap(),
                "header-secret".parse().unwrap(),
            )]),
            content_type_provenance: ContentTypeProvenance::Unknown,
            body: b"body-secret".to_vec(),
            cap_row: CapRow::ReadyToParse,
        };
        assert_eq!(
            format!("{response:?}"),
            "BoundedProxyResponse { status: 200, header_count: 1, body_bytes: 11, .. }",
        );
        assert_eq!(response.headers().get("x-secret").unwrap(), "header-secret");
    }

    fn request(url: &str) -> BidderHttpRequest {
        outbound_request::request(
            "POST".into(),
            url.into(),
            vec![("X-Openrtb-Version".into(), "2.6".into())],
            b"{}".to_vec(),
            vec![],
        )
        .expect("sink-built fixture")
    }

    fn composed() -> ComposedDeadline {
        ComposedDeadline {
            deadline_at: MonotonicInstant::now()
                .checked_add(Duration::from_secs(1))
                .expect("a one-second horizon is representable"),
            selected_cause: TimeoutCause::Bidder,
        }
    }

    #[test]
    fn admitted_endpoint_spellings_reach_the_transport() {
        for (literal, expected) in [
            (
                "https://bidder.example/auction",
                "https://bidder.example/auction",
            ),
            (
                "https://xn--bcher-kva.example/auction",
                "https://xn--bcher-kva.example/auction",
            ),
            (
                "https://bidder.example/a%3Cb",
                "https://bidder.example/a%3Cb",
            ),
            (
                "https://bidder.example/a%60b",
                "https://bidder.example/a%60b",
            ),
            (
                "https://bidder.example/caf\u{e9}",
                "https://bidder.example/caf%C3%A9",
            ),
            (
                "HTTPS://BIDDER.EXAMPLE:443/auction?q=a%2Fb",
                "https://bidder.example/auction?q=a%2Fb",
            ),
        ] {
            let endpoint = auctioneer_bidder::ConfiguredEndpoint::new(literal).unwrap();
            let converted = to_outbound(
                BoundedOutboundRequest {
                    request: request(endpoint.as_str()),
                    policy: test_policy(),
                    deadline: composed(),
                },
                &limits(),
            )
            .expect("admitted literal must survive transport preflight");
            assert_eq!(converted.uri().to_string(), expected);
        }
    }

    /// Every converted request carries `Bypass`, never the
    /// `PlatformDefault` an unset policy would inherit: a cached bid
    /// response is both a stale-bid correctness bug and a cross-auction
    /// privacy leak.
    #[test]
    fn bidder_calls_set_cache_bypass() {
        let limits = limits();
        for url in ["https://a.example/rtb", "https://b.example/rtb?x=1"] {
            let converted = to_outbound(
                BoundedOutboundRequest {
                    request: request(url),
                    policy: test_policy(),
                    deadline: composed(),
                },
                &limits,
            )
            .expect("a well-formed call converts");
            let parts = converted.into_parts();
            assert_eq!(
                parts.cache_policy,
                OutboundCachePolicy::Bypass,
                "{url} was left cacheable",
            );
        }
    }

    /// The decode caps are copied from the call's policy, so the type
    /// exists and carries per-slot values rather than a batch-wide one.
    #[test]
    fn decode_policy_is_copied_from_the_call_policy() {
        let policy = test_policy();
        let decode = DecodePolicy::from(&policy);
        assert_eq!(decode.max_decoded_bytes, policy.max_decoded_bytes);
        assert_eq!(decode.max_brotli_wbits, policy.max_brotli_wbits);
        assert_eq!(decode.max_brotli_layers, policy.max_brotli_layers);
        assert_eq!(decode.max_decoder_bytes, policy.max_decoder_bytes);
        assert_eq!(decode.max_chunk_bytes, policy.max_chunk_bytes);
    }

    #[test]
    fn max_chunk_bytes_is_eight_kib() {
        assert_eq!(MAX_CHUNK_BYTES, NonZeroU64::new(8_192).expect("nonzero"));
    }

    #[test]
    fn transport_body_moves_the_sink_allocation_without_copying() {
        let req = request("https://a.example/rtb");
        let pointer = req.body().as_ptr();
        let outbound = to_outbound(
            BoundedOutboundRequest {
                request: req,
                policy: test_policy(),
                deadline: composed(),
            },
            &limits(),
        )
        .unwrap();
        let body = outbound.into_parts().body.into_bytes().unwrap();
        assert_eq!(body.as_ptr(), pointer);
        assert_eq!(body.as_ref(), b"{}");
    }

    /// A header the `http` layer cannot represent fails THIS slot before
    /// dispatch. Silently dropping it would send a different request.
    #[test]
    fn invalid_header_fails_slot_preflight_not_silently_dropped() {
        let req = outbound_request::request(
            "POST".into(),
            "https://a.example/rtb".into(),
            vec![("bad header name".into(), "v".into())],
            b"{}".to_vec(),
            vec![],
        )
        .unwrap();
        let outcome = to_outbound(
            BoundedOutboundRequest {
                request: req,
                policy: test_policy(),
                deadline: composed(),
            },
            &limits(),
        );
        assert!(
            matches!(outcome, Err(super::BoundedSlotError::Preflight)),
            "an unrepresentable header name is a preflight failure, NOT the \
             encoded-size overflow that shares the nothing-was-sent shape \
             and carries a different wire code",
        );
    }

    /// Decoded SDK responses have no coding header. An explicit identity
    /// has the same parse-ready meaning.
    #[test]
    fn cap_row_adopts_the_upstream_encoding_classification() {
        let mut headers = HeaderMap::new();
        assert_eq!(
            cap_row_of(&headers),
            CapRow::ReadyToParse,
            "no encoding means upstream handed back plain bytes",
        );
        headers.insert("content-encoding", "identity".parse().unwrap());
        assert_eq!(cap_row_of(&headers), CapRow::ReadyToParse);
    }

    /// Each refused reason reaches auctioneer as its own cause, and the
    /// unsupported one keeps its coding token. The three are never
    /// collapsed into one.
    #[test]
    fn each_passthrough_reason_maps_to_its_own_cause() {
        for (encoding, expected, why) in [
            (
                "zstd",
                UndecodableEncoding::Unsupported("zstd".to_owned()),
                "an unknown coding carries the token that names it",
            ),
            (
                "gzip, br",
                UndecodableEncoding::Stacked,
                "a stacked chain is its own cause, not an unsupported token",
            ),
            (
                "gzip;q=1",
                UndecodableEncoding::Malformed,
                "a parameterized value names no coding to decode",
            ),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert("content-encoding", encoding.parse().expect("valid"));
            assert_eq!(
                cap_row_of(&headers),
                CapRow::RawPassthrough(expected),
                "`{encoding}`: {why}",
            );
        }
    }

    #[test]
    fn original_partial_response_codings_are_never_parse_ready() {
        use bytes::Bytes;
        use edgezero_core::body::Body;
        use edgezero_core::http::StatusCode;
        use edgezero_core::outbound::{
            ResponseBodyPolicy as SdkBodyPolicy, ResponseDecoding, ResponseMode,
            apply_response_body_policy,
        };
        use futures::StreamExt as _;
        use futures::executor::block_on;
        use futures::stream;

        for encoding in ["br", "gzip", "deflate"] {
            let mut headers = HeaderMap::new();
            headers.insert("content-encoding", encoding.parse().unwrap());
            let original = serde_json::to_vec(&serde_json::json!({"id":"fake"})).unwrap();
            let stream = apply_response_body_policy(
                stream::iter(vec![Ok(Bytes::from(original.clone()))]).boxed_local(),
                &mut headers,
                SdkBodyPolicy {
                    max_brotli_window_bits: 22,
                    max_chunk_bytes: Some(MAX_CHUNK_BYTES),
                    max_decoded_response_bytes: Some(64),
                    max_decoder_bytes: 20_971_520,
                    max_encoded_response_bytes: Some(64),
                    response_decoding: ResponseDecoding::Decode,
                    response_mode: ResponseMode::Buffered { max_bytes: 64 },
                    status: StatusCode::PARTIAL_CONTENT,
                },
            )
            .unwrap();
            assert_eq!(
                block_on(Body::from_stream(stream).into_bytes_bounded(64)).unwrap(),
                original,
                "SDK partial responses preserve original bytes, even when they look like JSON",
            );
            assert_eq!(headers["content-encoding"], encoding);
            assert_eq!(
                cap_row_of(&headers),
                CapRow::RawPassthrough(UndecodableEncoding::Unclassified),
                "a coding retained by the SDK denotes original, not decoded, bytes: {encoding}",
            );
        }
    }
}
