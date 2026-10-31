//! `BidderFanout` trait + neutral types.

use async_trait::async_trait;
pub use auctioneer_bidder::Compression;
use auctioneer_bidder::types::{BidderHttpRequest, BidderHttpResponse};
use auctioneer_runtime::MonotonicInstant;
use std::fmt;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TmaxSource {
    Request,
    HostDefault,
    CappedByHost,
}

/// Absolute monotonic instant the whole auction must finish by, plus
/// source of the underlying budget.
#[derive(Clone, Copy, Debug)]
pub struct AuctionDeadline {
    pub deadline_at: MonotonicInstant,
    pub source: TmaxSource,
}

pub struct BidderCall {
    pub request: BidderHttpRequest,
    pub timeout: Duration,
    pub max_transport_bytes: u64,
    pub max_decoded_bytes: u64,
}

/// Construction and transport encoding have independent body ceilings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OversizeRequestStage {
    Raw,
    Encoded,
}

#[derive(Debug)]
pub struct BidderCallResult {
    pub result: Result<BidderHttpResponse, FanoutError>,
    pub elapsed: Duration,
}

/// Which budget ran out when a bidder call timed out.
///
/// Only a strategy that owns a real timer can produce this — a fanout
/// with no timer cannot synthesize a cause, so on that path a
/// transport-level timeout surfaces as [`FanoutError::Other`] instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeoutCause {
    /// The per-bidder [`BidderCall::timeout`] elapsed.
    Bidder,
    /// The absolute [`AuctionDeadline::deadline_at`] elapsed.
    AuctionDeadline,
}

/// Which size ceiling a refused bidder response breached.
///
/// auctioneer owns this enum rather than storing the platform's
/// `ResponseLimitReason` directly, for two reasons. The platform's enum is
/// `#[non_exhaustive]`, so auctioneer could never match it without a wildcard
/// arm, and a wildcard is exactly the silent fold this type exists to
/// prevent. And this crate is the neutral taxonomy: it carries no
/// platform dependency, so a platform type in its public API would make
/// every consumer of a `FanoutError` depend on the platform too.
///
/// The mapping is therefore one explicit arm per recognised upstream
/// reason, plus [`Self::Unclassified`] for the reason upstream itself
/// calls unspecified and for any variant a future platform release adds.
/// An unrecognised reason is never folded into a named class: it lands in
/// `Unclassified`, which says "a size ceiling, origin unknown" and not
/// "a header ceiling".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OversizeOrigin {
    /// The response body exceeded its cap on the wire, before decoding.
    EncodedBody,
    /// Decoding expanded the body past its post-decode cap.
    DecodedBody,
    /// Buffering the whole body exceeded the platform's buffer ceiling.
    BufferedBody,
    /// The decoder's own working memory exceeded its ceiling.
    DecoderMemory,
    /// The response advertised a Brotli window wider than allowed.
    BrotliWindow,
    /// The response header block exceeded its byte cap.
    HeaderBytes,
    /// The response carried more header entries than allowed.
    HeaderCount,
    /// The REQUEST's header block was refused as too large, so no
    /// response was ever read.
    ///
    /// Kept in this enum rather than given its own `FanoutError` because
    /// the wire contract is identical — error code 4, `seatnonbid`
    /// statuscode 100 — and splitting it would change that contract.
    /// Kept as its OWN variant rather than shared with a response class
    /// because it is a different fact: auctioneer sent a request the platform
    /// would not carry, which is auctioneer's composition to fix, whereas
    /// every other variant is the bidder's answer being too big.
    RequestHeaders,
    /// auctioneer's own `BidderCall::max_decoded_bytes` check rejected a
    /// body the platform had already accepted.
    ///
    /// Distinct from every upstream reason because the ceiling is
    /// auctioneer's, applied after a successful read.
    CallResponseCap,
    /// A size ceiling whose origin auctioneer cannot name: the platform
    /// reported its own "unspecified", or reported a reason this build
    /// does not know about.
    Unclassified,
}

/// Fixed transport categories; never carries an upstream diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportErrorKind {
    Initial,
    Unreachable,
}

impl TransportErrorKind {
    #[must_use]
    pub const fn public_message(self) -> &'static str {
        match self {
            Self::Initial => "outbound send failed",
            Self::Unreachable => "bidder host unreachable",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct TransportError {
    pub kind: TransportErrorKind,
}

impl fmt::Debug for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TransportError")
            .field(&self.kind.public_message())
            .finish()
    }
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.kind.public_message())
    }
}

/// Why a bidder response arrived in an encoding nothing could decode.
///
/// auctioneer owns this enum rather than storing the platform's
/// `PassthroughReason` directly, for the two reasons
/// [`OversizeOrigin`] gives: the platform's enum is
/// `#[non_exhaustive]`, so matching it would need the wildcard this type
/// exists to contain, and this crate is the neutral taxonomy — a
/// platform type in its public API would make every consumer of a
/// [`FanoutError`] depend on the platform too.
///
/// The mapping is one explicit arm per recognised upstream reason, plus
/// [`Self::Unclassified`] for a variant a future platform release adds.
/// An unrecognised reason is never folded into a named cause: it lands
/// in `Unclassified`, which says "undecodable, cause unknown" and not
/// "a stacked chain".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UndecodableEncoding {
    /// The `Content-Encoding` value was not a well-formed coding list,
    /// so no coding could be named, let alone decoded.
    Malformed,
    /// The response stacked more than one coding. The platform decodes a
    /// single bare coding only, and auctioneer has no layered decoder.
    Stacked,
    /// One well-formed coding that no decoder in this workspace
    /// implements — `zstd`, `compress`, and the like.
    ///
    /// Carries the coding token so an operator can tell which bidder is
    /// sending what. The token is the platform's PARSED coding: already
    /// lowercased and already validated against the HTTP token grammar,
    /// so it is a bounded identifier and not free upstream text. It is
    /// still never rendered to the wire — see
    /// [`FanoutError::public_message`].
    Unsupported(String),
    /// A cause auctioneer cannot name, because the platform reported a
    /// reason this build does not know about.
    Unclassified,
}

#[derive(Clone, Debug)]
pub enum FanoutError {
    Timeout(TimeoutCause),
    Transport(TransportError),
    Unreachable,
    BadServerResponse,
    OversizeResponse(OversizeOrigin),
    /// An outbound request auctioneer built and could not carry: it crossed
    /// an outbound size ceiling. Code 5 — the bidder never reached the
    /// fan-out, exactly like a `make_requests` failure.
    ///
    /// Separate from [`Self::OversizeResponse`] because the direction
    /// decides the code: a response too large is the bidder answering
    /// badly (4), a request too large is auctioneer failing to request bids
    /// (5). Both used to arrive as [`Self::Other`] and render 999.
    OversizeRequest {
        /// Which ceiling, and therefore where the entry lands.
        stage: OversizeRequestStage,
    },
    /// auctioneer's own transport conversion refused the request at the
    /// `http` layer — method, URI, or a header it cannot represent.
    /// Nothing was sent.
    ///
    /// Code 4 per the spec's wire table, which decides this case
    /// explicitly rather than by the general "unparseable response"
    /// gloss.
    Preflight,
    /// The bidder answered, within every cap, in an encoding neither the
    /// platform nor auctioneer can decode.
    ///
    /// Distinct from [`Self::BadServerResponse`] even though both are
    /// code 4: that one means the bidder's answer was broken, while this
    /// one means it was well-formed but unreadable here. The two share a
    /// wire pair and must stay separable in a log, because the operator
    /// actions differ — one is the bidder's bug, the other is a codec
    /// auctioneer does not implement.
    UndecodableResponse(UndecodableEncoding),
    Other(String),
}

impl FanoutError {
    /// Fixed category text for the response `ext.errors` entry.
    ///
    /// Allowlisted by construction: the returned string is a compile-time
    /// constant per variant and no arm reads a payload, so an untrusted
    /// transport detail can never reach the wire through this path. The
    /// [`OversizeOrigin`] an oversize error carries is auctioneer's own
    /// classification, not upstream text, and it is still deliberately
    /// NOT rendered here — the wire category stays one string for every
    /// size ceiling. The [`UndecodableEncoding`] cause is withheld the
    /// same way, including its coding token: that token is bounded, but
    /// the wire has no field for it and inventing one would publish a
    /// shape no consumer expects.
    #[must_use]
    pub fn public_message(&self) -> &'static str {
        match self {
            Self::Timeout(_) => "bidder timeout",
            Self::Transport(error) => error.kind.public_message(),
            Self::Unreachable => "bidder unreachable",
            Self::BadServerResponse => "bidder server error",
            Self::OversizeResponse(_) => "bidder response too large",
            // The stage is withheld for the same reason an oversize
            // response withholds its origin: it is auctioneer's internal
            // diagnostic fidelity and the wire has no field for it.
            Self::OversizeRequest { .. } => "bidder request too large",
            Self::Preflight => "bidder request could not be dispatched",
            Self::UndecodableResponse(_) => "bidder response encoding unsupported",
            Self::Other(_) => "internal fanout error",
        }
    }
}

#[derive(Debug)]
pub enum FanoutInternalError {
    NoOutboundCapability,
    Internal(String),
    /// The outbound batch reported every dispatched slot resolved while
    /// one or more positions were still unfilled.
    ///
    /// Distinct from [`Self::Internal`] because it names a specific
    /// broken promise — positional completeness — and carries the two
    /// numbers that prove it, so an operator reading a 500 can tell a
    /// positional bug from any other orchestration fault. A cutoff is
    /// NOT this: an unresolved slot at cutoff is the expected shape and
    /// becomes a positional timeout.
    BatchContractViolation {
        /// Slots dispatched upstream.
        expected: usize,
        /// Slots that came back terminal.
        got: usize,
    },
}

/// Async fan-out seam. `&dyn BidderFanout` is held by the exchange — so
/// the trait carries `async-trait(?Send)`; native `async fn
/// in trait` is not object-safe.
#[async_trait(?Send)]
pub trait BidderFanout: Send + Sync {
    /// Dispatch every call concurrently and return them in order.
    ///
    /// Contract:
    /// - `Ok(results)` ⇒ `results.len() == calls.len()` and `results[i]`
    ///   corresponds positionally to `calls[i]` (no reordering).
    /// - `Err(_)` ⇒ no per-call results are produced; the exchange maps
    ///   to `AuctionError::FanoutUnavailable`.
    ///
    /// # Errors
    /// Returns `FanoutInternalError` when the fanout itself cannot
    /// run (e.g. `NoOutboundCapability` on a target where the proxy
    /// hook isn't wired).
    async fn send_all(
        &self,
        calls: Vec<BidderCall>,
        deadline: AuctionDeadline,
    ) -> Result<Vec<BidderCallResult>, FanoutInternalError>;
}

#[cfg(test)]
mod tests {
    use super::BidderFanout;

    #[test]
    fn trait_is_object_safe() {
        fn _take(_b: &dyn BidderFanout) {}
    }
}
