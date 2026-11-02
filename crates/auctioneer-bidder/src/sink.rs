//! The enforcing outbound-request sink and the auction-global call budget.
//!
//! A bidder does not return a `Vec` of requests it already built. It
//! pushes each request through [`OutboundRequestSink::push_request`],
//! which reserves a call slot BEFORE any buffer is allocated and rolls
//! that reservation back on every failure path. Partial success is
//! structural: accepted requests and typed build errors accumulate side
//! by side, so one over-cap body does not cost a bidder its siblings.
//!
//! The transaction inside `push_request` is fixed:
//!
//! 1. RESERVE — a slot from the shared [`CallBudget`]. Exhaustion
//!    returns before either closure runs, so nothing is allocated.
//! 2. METADATA — the meta byte pool is allocated and `build_metadata`
//!    fills it.
//! 3. VALIDATE — the finalized URL goes to the exchange-owned
//!    [`EndpointValidator`]. A rejection means `build_body` never runs.
//! 4. BODY — the [`CappedWriter`] is allocated and `build_body` fills it.
//! 5. COMMIT — the request and its STILL-UNCOMMITTED reservation are
//!    stored together. Commitment happens in [`OutboundRequestSink::into_outcome`]
//!    alone, so a later semantic failure refunds every slot by dropping
//!    the sink.

use crate::capped_writer::{BodyWriteError, CappedWriter};
use crate::diagnostics::{ConstructionDiagnostic, ConstructionDiagnostics};
use crate::endpoint_validator::{EndpointRejectReason, EndpointValidator};
use crate::meta::{META_BYTES_CAPACITY, OutboundRequestMetaBuilder, SinkAbort};
use crate::request_info::BidderRequestInfo;
use crate::types::{BidderHttpRequest, OutboundBuildError};
use std::cell::Cell;
use std::mem;

/// How many typed build errors one sink retains verbatim. Past this the
/// count moves into `suppressed_error_count`, so a bidder looping
/// `push_request` cannot grow memory through the error list.
pub const MAX_RETAINED_SINK_ERRORS: usize = 16;

/// The two values a sink enforces, copied out of the resolved isolate
/// envelope by whichever crate composes the auction.
///
/// `max_outbound_raw_bytes` is a `usize` because that is the width
/// [`CappedWriter`] and `Vec::with_capacity` take. The envelope carries
/// it as a `u64`; the checked conversion happens ONCE at boot, in the
/// crate that owns the envelope, so no per-call site can fail on a
/// 32-bit target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SinkLimits {
    /// Cap on one bidder-built raw body.
    pub max_outbound_raw_bytes: usize,
    /// Sizes the meta builder's imp-id offset table.
    pub max_impressions_per_auction: u16,
}

impl SinkLimits {
    /// Build the limits from two primitives.
    ///
    /// Taking primitives rather than an envelope type is what keeps
    /// `auctioneer-bidder` free of a dependency on the crate that resolves
    /// the isolate budget — the composition site sees both types and
    /// neither library crate learns about the other.
    #[must_use]
    pub const fn new(max_outbound_raw_bytes: usize, max_impressions_per_auction: u16) -> Self {
        Self {
            max_outbound_raw_bytes,
            max_impressions_per_auction,
        }
    }
}

/// The ONE auction-global cap on outbound calls.
///
/// `max_outbound_requests` is an AUCTION limit, so every per-bidder sink
/// shares a single budget — per-bidder budgets would have allowed
/// `bidders x max_calls`. The bidder loop is sequential in registry
/// order, so a `Cell` suffices and exhaustion is deterministic: slots go
/// in registry order, then in per-bidder request order.
#[derive(Debug)]
pub struct CallBudget {
    remaining: Cell<u32>,
    max_calls: u32,
}

impl CallBudget {
    #[must_use]
    pub const fn new(max_calls: u32) -> Self {
        Self {
            remaining: Cell::new(max_calls),
            max_calls,
        }
    }

    /// The cap this budget was created with, for the code-10999 warning.
    #[must_use]
    pub const fn max_calls(&self) -> u32 {
        self.max_calls
    }

    /// Slots still available. Diagnostic — a caller decides nothing from
    /// it, because only [`CallBudget::try_reserve`] is race-free.
    #[must_use]
    pub fn remaining(&self) -> u32 {
        self.remaining.get()
    }

    /// Take one slot, or `None` when the budget is spent.
    ///
    /// The returned [`CallReservation`] re-credits the slot when it is
    /// dropped without [`CallReservation::commit`], so there is no
    /// cross-crate rollback call to forget.
    #[must_use]
    pub fn try_reserve(&self) -> Option<CallReservation<'_>> {
        let left = self.remaining.get();
        let next = left.checked_sub(1)?;
        self.remaining.set(next);
        Some(CallReservation {
            budget: self,
            committed: false,
        })
    }

    fn refund(&self) {
        self.remaining.set(self.remaining.get().saturating_add(1));
    }
}

/// One held call slot. Dropping it uncommitted returns the slot.
#[derive(Debug)]
pub struct CallReservation<'budget> {
    budget: &'budget CallBudget,
    committed: bool,
}

impl CallReservation<'_> {
    /// Spend the slot permanently. Only the sink's `into_outcome` calls
    /// this, and only for requests that survived to dispatch.
    fn commit(mut self) {
        self.committed = true;
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for CallReservation<'_> {
    fn drop(&mut self) {
        if !self.committed {
            self.budget.refund();
        }
    }
}

/// An accepted request travelling with its still-uncommitted reservation.
///
/// Committing inside `push_request` would make a later semantic-error
/// discard unrefundable; dropping the reservation there would refund a
/// slot the request still occupies. Carrying the two together is what
/// makes both terminal paths correct without a rollback API.
#[derive(Debug)]
pub struct AcceptedRequest<'budget> {
    request: BidderHttpRequest,
    reservation: CallReservation<'budget>,
}

/// Layout pins for the isolate memory charge. A field added to either
/// type fails this assertion rather than silently outgrowing the budget.
const _: () = {
    // Pointer-width-independent, and the fact the offset-table charge
    // depends on.
    assert!(
        size_of::<(u16, u16, u16, u16)>() == 8,
        "a header offset entry must stay 8 bytes; the offset-table charge assumes it"
    );
    // Keep the upstream per-call upper bound. The sealed request holds
    // only an Arc metadata handle and a movable Vec body; the metadata
    // allocation and body bytes have separate charges. Include the two
    // Arc counters here, not in correlation's separate metadata delta.
    // Smaller wasm handles must not fail this upper-bound pin.
    assert!(
        size_of::<AcceptedRequest<'_>>() + 2 * size_of::<usize>() <= 120,
        "AcceptedRequest outgrew the per-call struct overhead the isolate budget charges"
    );
};

/// Everything one bidder's construction pass produced.
#[derive(Debug)]
pub struct SinkOutcome {
    /// Requests cleared for finalization and dispatch, in push order.
    pub accepted: Vec<BidderHttpRequest>,
    /// The first [`MAX_RETAINED_SINK_ERRORS`] typed build errors.
    pub errors: Vec<OutboundBuildError>,
    /// Independently bounded nonfatal diagnostics, including on fatal discard.
    pub diagnostics: ConstructionDiagnostics,
    /// Typed errors past the retained window.
    pub suppressed_error_count: u32,
    /// Every attempt the call budget refused. Drives the code-10999 `N`.
    pub rejected_call_count: u32,
}

/// The only constructor of a dispatchable [`BidderHttpRequest`].
pub struct OutboundRequestSink<'budget> {
    limits: SinkLimits,
    budget: &'budget CallBudget,
    validator: &'budget dyn EndpointValidator,
    accepted: Vec<AcceptedRequest<'budget>>,
    errors: Vec<OutboundBuildError>,
    diagnostics: ConstructionDiagnostics,
    suppressed_error_count: u32,
    rejected_call_count: u32,
    /// Whether the error list already carries a budget-exhaustion entry.
    /// The exchange needs the reason once; the COUNT lives in
    /// `rejected_call_count`.
    budget_error_recorded: bool,
}

impl<'budget> OutboundRequestSink<'budget> {
    /// Read the installed raw-body ceiling; every write remains sink-enforced.
    #[must_use]
    pub const fn max_outbound_raw_bytes(&self) -> usize {
        self.limits.max_outbound_raw_bytes
    }

    /// Open a sink over the shared budget and the exchange's validator.
    #[must_use]
    pub fn new(
        limits: SinkLimits,
        budget: &'budget CallBudget,
        validator: &'budget dyn EndpointValidator,
    ) -> Self {
        Self {
            limits,
            budget,
            validator,
            accepted: Vec::new(),
            errors: Vec::with_capacity(MAX_RETAINED_SINK_ERRORS),
            diagnostics: ConstructionDiagnostics::default(),
            suppressed_error_count: 0,
            rejected_call_count: 0,
            budget_error_recorded: false,
        }
    }

    /// Build and accept one outbound request.
    ///
    /// `build_metadata` sets the method, the URL, the headers and the
    /// covered imp ids; `build_body` writes the raw body. Both speak the
    /// builder error language, and the sink maps it onto
    /// [`OutboundBuildError`].
    ///
    /// The returned error is ADVISORY — every rejection is recorded in
    /// the sink's error list before it is returned, so a bidder that
    /// ignores the `Result` cannot hide a failure from the exchange. It
    /// exists so a bidder can stop early.
    ///
    /// # Errors
    /// - [`OutboundBuildError::CallBudgetExhausted`] when no slot is
    ///   left. Neither closure runs.
    /// - [`OutboundBuildError::MissingMethod`] / [`OutboundBuildError::MissingUrl`]
    ///   when `build_metadata` returned `Ok` without setting one, checked
    ///   in that order and before validation, so the body closure never
    ///   runs.
    /// - [`OutboundBuildError::EndpointRejected`] when the validator
    ///   refuses the finalized URL, and
    ///   [`OutboundBuildError::MalformedUrl`] when it does not parse at
    ///   all — in both cases the body closure never runs.
    /// - The [`SinkAbort`] `build_metadata` returned, mapped variant for
    ///   variant, or [`OutboundBuildError::OversizeRaw`] /
    ///   [`OutboundBuildError::Body`] for a body failure.
    ///
    /// Every one of these rolls the reservation back.
    pub fn push_request<M, B>(
        &mut self,
        info: &BidderRequestInfo<'_>,
        build_metadata: M,
        build_body: B,
    ) -> Result<(), OutboundBuildError>
    where
        M: FnOnce(&mut OutboundRequestMetaBuilder<'_>) -> Result<(), SinkAbort>,
        B: FnOnce(&mut CappedWriter<'_>) -> Result<(), BodyWriteError>,
    {
        // RESERVE. Before this point nothing is allocated, and an
        // exhausted budget returns without allocating anything.
        let Some(reservation) = self.budget.try_reserve() else {
            self.rejected_call_count = self.rejected_call_count.saturating_add(1);
            let error = OutboundBuildError::CallBudgetExhausted {
                max_calls: self.budget.max_calls(),
            };
            if !self.budget_error_recorded {
                self.budget_error_recorded = true;
                // Count rejections have their own exhaustive counter and warning.
                // A full build-error window must not turn one into a code-5 error.
                if self.errors.len() < MAX_RETAINED_SINK_ERRORS {
                    self.errors.push(error.clone());
                }
            }
            return Err(error);
        };

        match self.build_accepted(info, build_metadata, build_body) {
            Ok(request) => {
                self.accepted.push(AcceptedRequest {
                    request,
                    reservation,
                });
                Ok(())
            }
            Err(error) => {
                // The reservation drops here, uncommitted, so the slot
                // is back before the caller sees the error.
                drop(reservation);
                self.record(error.clone());
                Err(error)
            }
        }
    }

    /// The METADATA → VALIDATE → BODY stages. Split out so every exit
    /// path in `push_request` rolls the reservation back exactly once.
    fn build_accepted<M, B>(
        &self,
        info: &BidderRequestInfo<'_>,
        build_metadata: M,
        build_body: B,
    ) -> Result<BidderHttpRequest, OutboundBuildError>
    where
        M: FnOnce(&mut OutboundRequestMetaBuilder<'_>) -> Result<(), SinkAbort>,
        B: FnOnce(&mut CappedWriter<'_>) -> Result<(), BodyWriteError>,
    {
        // METADATA.
        let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut meta_builder =
            OutboundRequestMetaBuilder::new(&mut pool, self.limits.max_impressions_per_auction);
        build_metadata(&mut meta_builder).map_err(OutboundBuildError::from)?;
        let meta = meta_builder.finish();

        // An incomplete meta is typed, and detected before validation so
        // the body closure never runs for one.
        if meta.method().is_empty() {
            return Err(OutboundBuildError::MissingMethod);
        }
        if meta.url().is_empty() {
            return Err(OutboundBuildError::MissingUrl);
        }

        // Parsing percent-encodes literal path braces, losing the distinction
        // from macro data that the builder already encoded. Check first.
        if meta.url().contains("{{.") {
            return Err(OutboundBuildError::EndpointRejected {
                reason: EndpointRejectReason::TemplateUnresolved,
            });
        }
        // VALIDATE. Endpoint policy remains exchange-owned.
        let url =
            url::Url::parse(meta.url()).map_err(|_unparsed| OutboundBuildError::MalformedUrl)?;
        self.validator
            .validate(&url, info)
            .map_err(|reason| OutboundBuildError::EndpointRejected { reason })?;

        // BODY. Only now is the body buffer allocated.
        let cap = self.limits.max_outbound_raw_bytes;
        let mut body = Vec::with_capacity(cap);
        let mut writer = CappedWriter::new(&mut body, cap).map_err(OutboundBuildError::from)?;
        build_body(&mut writer).map_err(OutboundBuildError::from)?;

        Ok(BidderHttpRequest::from_sink(meta, body))
    }

    /// Serialize JSON only after call reservation and endpoint validation.
    ///
    /// # Errors
    /// Returns and records the same construction failures as `push_request`.
    pub fn write_json<M, T>(
        &mut self,
        info: &BidderRequestInfo<'_>,
        build_metadata: M,
        value: &T,
    ) -> Result<(), OutboundBuildError>
    where
        M: FnOnce(&mut OutboundRequestMetaBuilder<'_>) -> Result<(), SinkAbort>,
        T: serde::Serialize + ?Sized,
    {
        self.push_request(info, build_metadata, |writer| writer.write_json(value))
    }

    /// Record a nonfatal diagnostic without spending a call or build-error slot.
    /// Accepted sibling requests remain eligible for dispatch.
    pub fn diagnostic(&mut self, diagnostic: ConstructionDiagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Requests accepted so far, for a bidder that adapts its next
    /// push to how many landed.
    #[must_use]
    pub fn accepted_count(&self) -> usize {
        self.accepted.len()
    }

    /// Finish on the bidder's success path: every held reservation
    /// commits and the requests go to dispatch.
    #[must_use]
    pub fn into_outcome(mut self) -> SinkOutcome {
        let accepted = mem::take(&mut self.accepted);
        let requests = accepted
            .into_iter()
            .map(|entry| {
                entry.reservation.commit();
                entry.request
            })
            .collect();
        SinkOutcome {
            accepted: requests,
            errors: mem::take(&mut self.errors),
            diagnostics: self.diagnostics,
            suppressed_error_count: self.suppressed_error_count,
            rejected_call_count: self.rejected_call_count,
        }
    }

    /// Finish on the bidder's semantic-failure path: the requests are
    /// dropped and their reservations refund themselves, while diagnostics,
    /// errors and both counts still report for the bidder.
    #[must_use]
    pub fn into_outcome_discarding_accepted(mut self) -> SinkOutcome {
        // Dropping the entries drops uncommitted reservations, which
        // re-credit the budget.
        self.accepted.clear();
        SinkOutcome {
            accepted: Vec::new(),
            errors: mem::take(&mut self.errors),
            diagnostics: self.diagnostics,
            suppressed_error_count: self.suppressed_error_count,
            rejected_call_count: self.rejected_call_count,
        }
    }

    /// Retain an error verbatim while the window has room, and count it
    /// otherwise. The list's capacity never grows.
    fn record(&mut self, error: OutboundBuildError) {
        if self.errors.len() < MAX_RETAINED_SINK_ERRORS {
            self.errors.push(error);
        } else {
            self.suppressed_error_count = self.suppressed_error_count.saturating_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AcceptedRequest, CallBudget, MAX_RETAINED_SINK_ERRORS, OutboundRequestSink, SinkLimits,
    };
    use crate::bidder::EntryPoint;
    use crate::capped_writer::{BodyWriteError, CappedWriter};
    use crate::diagnostics::{ConstructionDiagnostic, MAX_CONSTRUCTION_DIAGNOSTICS};
    use crate::endpoint_validator::{EndpointRejectReason, EndpointValidator};
    use crate::meta::{META_BIDDER_REGION_BYTES, OutboundRequestMetaBuilder, SinkAbort};
    use crate::request_info::BidderRequestInfo;
    use crate::types::{BidderName, OutboundBuildError};
    use std::cell::Cell;
    use std::ptr;

    const URL: &str = "https://example.test/openrtb";

    /// Accepts everything — the sink behaviour under test is the
    /// transaction, not the policy.
    struct AcceptAll;

    impl EndpointValidator for AcceptAll {
        fn validate(
            &self,
            _url: &url::Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            Ok(())
        }
    }

    /// Rejects with one fixed reason, and counts how often it was asked.
    struct Rejecting {
        reason: EndpointRejectReason,
        calls: Cell<u32>,
    }

    impl Rejecting {
        const fn new(reason: EndpointRejectReason) -> Self {
            Self {
                reason,
                calls: Cell::new(0),
            }
        }
    }

    impl EndpointValidator for Rejecting {
        fn validate(
            &self,
            _url: &url::Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(self.reason)
        }
    }

    fn info<'a>(macros: &'a [&'a str], currency: &'a [String]) -> BidderRequestInfo<'a> {
        BidderRequestInfo::new(
            BidderName::new("fake_generic"),
            BidderName::new("fake_generic"),
            "acct-1",
            None,
            URL,
            macros,
            None,
            None,
            EntryPoint::Auction,
            currency,
        )
    }

    fn limits() -> SinkLimits {
        SinkLimits::new(4_096, 100)
    }

    /// The ordinary metadata closure: a complete, valid request.
    fn good_meta(meta: &mut OutboundRequestMetaBuilder<'_>) -> Result<(), SinkAbort> {
        meta.set_method("POST")?;
        meta.set_url(URL)?;
        meta.imp_id("imp-1")
    }

    fn small_body(body: &mut CappedWriter<'_>) -> Result<(), BodyWriteError> {
        body.write_all(b"{}")
    }

    #[test]
    fn raw_body_ceiling_observation_preserves_each_independent_installed_cap() {
        for cap in [0, 1, 127, 4096] {
            let budget = CallBudget::new(7);
            let sink = OutboundRequestSink::new(SinkLimits::new(cap, 13), &budget, &AcceptAll);
            assert_eq!(sink.max_outbound_raw_bytes(), cap);
            assert_eq!(budget.remaining(), 7);
        }
    }

    #[test]
    fn both_terminal_paths_start_with_an_empty_diagnostic_buffer() {
        for discard in [false, true] {
            let budget = CallBudget::new(0);
            let sink = OutboundRequestSink::new(limits(), &budget, &AcceptAll);
            let outcome = if discard {
                sink.into_outcome_discarding_accepted()
            } else {
                sink.into_outcome()
            };
            assert!(outcome.diagnostics.is_empty());
            assert!(outcome.errors.is_empty());
            assert_eq!(outcome.suppressed_error_count, 0);
            assert_eq!(outcome.rejected_call_count, 0);
        }
    }

    #[test]
    fn nonfatal_diagnostics_preserve_accepted_siblings_and_commit_their_slots() {
        let budget = CallBudget::new(2);
        let request_info = info(&[], &[]);
        let mut sink = OutboundRequestSink::new(limits(), &budget, &AcceptAll);
        sink.diagnostic(ConstructionDiagnostic::InvalidImpressionParameters);
        assert_eq!(budget.remaining(), 2);
        sink.push_request(&request_info, good_meta, small_body)
            .unwrap();
        sink.diagnostic(ConstructionDiagnostic::UnsupportedMediaType);
        sink.push_request(&request_info, good_meta, small_body)
            .unwrap();
        sink.diagnostic(ConstructionDiagnostic::InvalidImpression);

        let outcome = sink.into_outcome();
        assert_eq!(outcome.accepted.len(), 2);
        assert!(outcome.errors.is_empty());
        assert_eq!(outcome.suppressed_error_count, 0);
        assert_eq!(outcome.rejected_call_count, 0);
        assert_eq!(budget.remaining(), 0);
        assert!(
            outcome.diagnostics.iter().eq([
                ConstructionDiagnostic::InvalidImpressionParameters,
                ConstructionDiagnostic::UnsupportedMediaType,
                ConstructionDiagnostic::InvalidImpression,
            ]
            .iter())
        );
    }

    #[test]
    fn diagnostics_and_sink_error_windows_are_independent_in_both_orders() {
        for diagnostics_first in [false, true] {
            let budget = CallBudget::new(1);
            let request_info = info(&[], &[]);
            let mut sink = OutboundRequestSink::new(limits(), &budget, &AcceptAll);
            for _ in 0..=MAX_CONSTRUCTION_DIAGNOSTICS {
                if diagnostics_first {
                    sink.diagnostic(ConstructionDiagnostic::InvalidImpressionParameters);
                }
            }
            for _ in 0..=MAX_RETAINED_SINK_ERRORS {
                assert_eq!(
                    sink.push_request(&request_info, |meta| meta.set_url(URL), small_body),
                    Err(OutboundBuildError::MissingMethod)
                );
            }
            for _ in 0..=MAX_CONSTRUCTION_DIAGNOSTICS {
                if !diagnostics_first {
                    sink.diagnostic(ConstructionDiagnostic::InvalidImpressionParameters);
                }
            }
            assert_eq!(budget.remaining(), 1);
            sink.push_request(&request_info, good_meta, small_body)
                .unwrap();
            assert_eq!(
                sink.push_request(&request_info, good_meta, small_body),
                Err(OutboundBuildError::CallBudgetExhausted { max_calls: 1 })
            );

            let outcome = sink.into_outcome();
            assert_eq!(outcome.accepted.len(), 1);
            assert_eq!(outcome.errors.len(), 16);
            assert_eq!(outcome.errors.capacity(), MAX_RETAINED_SINK_ERRORS);
            assert!(
                outcome
                    .errors
                    .iter()
                    .all(|error| *error == OutboundBuildError::MissingMethod)
            );
            assert_eq!(outcome.suppressed_error_count, 1);
            assert_eq!(outcome.rejected_call_count, 1);
            assert_eq!(outcome.diagnostics.len(), 32);
            let mut expected = [ConstructionDiagnostic::InvalidImpressionParameters; 32];
            expected[31] = ConstructionDiagnostic::AdditionalDiagnostics;
            assert!(outcome.diagnostics.iter().eq(expected.iter()));
            assert_eq!(budget.remaining(), 0);
        }
    }

    #[test]
    fn fatal_discard_moves_diagnostics_and_errors_and_refunds_all_accepted_slots() {
        let budget = CallBudget::new(2);
        let request_info = info(&[], &[]);
        let mut sink = OutboundRequestSink::new(limits(), &budget, &AcceptAll);
        sink.diagnostic(ConstructionDiagnostic::InvalidImpressionParameters);
        assert_eq!(
            sink.push_request(&request_info, |meta| meta.set_url(URL), small_body),
            Err(OutboundBuildError::MissingMethod)
        );
        for _ in 0_usize..2 {
            sink.push_request(&request_info, good_meta, small_body)
                .unwrap();
        }
        assert_eq!(budget.remaining(), 0);
        assert_eq!(
            sink.push_request(&request_info, good_meta, small_body),
            Err(OutboundBuildError::CallBudgetExhausted { max_calls: 2 })
        );
        sink.diagnostic(ConstructionDiagnostic::UnsupportedMediaType);

        let outcome = sink.into_outcome_discarding_accepted();
        assert!(outcome.accepted.is_empty());
        assert_eq!(
            outcome.errors,
            [
                OutboundBuildError::MissingMethod,
                OutboundBuildError::CallBudgetExhausted { max_calls: 2 },
            ]
        );
        assert_eq!(outcome.suppressed_error_count, 0);
        assert_eq!(outcome.rejected_call_count, 1);
        assert!(
            outcome.diagnostics.iter().eq([
                ConstructionDiagnostic::InvalidImpressionParameters,
                ConstructionDiagnostic::UnsupportedMediaType,
            ]
            .iter())
        );
        assert_eq!(budget.remaining(), 2);

        let mut sibling = OutboundRequestSink::new(limits(), &budget, &AcceptAll);
        for _ in 0_usize..2 {
            sibling
                .push_request(&request_info, good_meta, small_body)
                .unwrap();
        }
        assert_eq!(sibling.into_outcome().accepted.len(), 2);
        assert_eq!(budget.remaining(), 0);
    }

    /// A body closure that records whether it ran, so a test can prove
    /// the body stage was never reached instead of asserting it was not.
    /// Stage ordering is a fact about what executed, and a witness that
    /// the closure itself flips is the only thing that observes it.
    #[derive(Default)]
    struct BodyWitness {
        ran: Cell<bool>,
    }

    impl BodyWitness {
        fn closure(&self) -> impl FnOnce(&mut CappedWriter<'_>) -> Result<(), BodyWriteError> {
            move |body| {
                self.ran.set(true);
                body.write_all(b"{}")
            }
        }

        fn assert_never_ran(&self) {
            assert!(
                !self.ran.get(),
                "build_body ran for a request that should have been rejected first"
            );
        }
    }

    #[test]
    fn accepted_request_stays_within_its_charged_size() {
        // The compile-time pin lives beside the type; this is the
        // runtime witness of the same fact, and of the width the
        // offset-table charge assumes.
        assert_eq!(size_of::<(u16, u16, u16, u16)>(), 8);
        assert!(
            size_of::<AcceptedRequest<'_>>() + 2 * size_of::<usize>() <= 120,
            "AcceptedRequest outgrew the per-call struct overhead the budget charges"
        );
        // The bound is the 64-bit worst case, so a narrower target must
        // land strictly under it rather than merely at it.
        if size_of::<usize>() < 8 {
            assert!(
                size_of::<AcceptedRequest<'_>>() + 2 * size_of::<usize>() < 120,
                "a 32-bit target's handles are narrower, so it cannot reach the 64-bit bound"
            );
        }
    }

    #[test]
    fn call_budget_exhausted_returns_slot_on_build_failure() {
        let budget = CallBudget::new(1);
        let validator = AcceptAll;
        let macros: &[&str] = &[];
        let currency: &[String] = &[];
        let request_info = info(macros, currency);

        // A first sink burns its one slot on a body that cannot fit.
        let mut first = OutboundRequestSink::new(SinkLimits::new(4, 100), &budget, &validator);
        let err = first
            .push_request(&request_info, good_meta, |body| {
                body.write_all(b"far too many bytes for a four-byte cap")
            })
            .unwrap_err();
        assert_eq!(err, OutboundBuildError::OversizeRaw { limit: 4 });
        assert_eq!(
            budget.remaining(),
            1,
            "a failed build must re-credit its reservation immediately"
        );

        // The refunded slot is available to the next bidder's sink.
        let mut second = OutboundRequestSink::new(limits(), &budget, &validator);
        second
            .push_request(&request_info, good_meta, small_body)
            .unwrap();
        assert_eq!(second.accepted_count(), 1);
        assert_eq!(
            budget.remaining(),
            0,
            "an accepted request holds its slot until the outcome is taken"
        );

        let outcome = second.into_outcome();
        assert_eq!(outcome.accepted.len(), 1);
        assert_eq!(outcome.rejected_call_count, 0);
        assert_eq!(
            budget.remaining(),
            0,
            "into_outcome commits, so the slot must not come back"
        );
    }

    #[test]
    fn invalid_correlation_refunds_the_call_before_body_and_preserves_siblings() {
        use crate::correlation::{CorrelationError, CorrelationMedia};

        for (case, expected) in [
            (0_u8, CorrelationError::HeaderOverCap),
            (1, CorrelationError::DuplicateHeader),
            (2, CorrelationError::UnknownOrdinal),
            (3, CorrelationError::DuplicateOrdinal),
            (4, CorrelationError::DuplicateCoverage),
            (5, CorrelationError::InvalidMedia),
        ] {
            let budget = CallBudget::new(2);
            let witness = BodyWitness::default();
            let request_info = info(&[], &[]);
            let mut sink = OutboundRequestSink::new(limits(), &budget, &AcceptAll);
            sink.push_request(
                &request_info,
                |meta| {
                    good_meta(meta)?;
                    meta.set_correlation_header(b"incumbent")?;
                    meta.correlate_imp(0, CorrelationMedia::Banner, 10)
                },
                small_body,
            )
            .unwrap();
            assert_eq!(budget.remaining(), 1);
            let error = sink
                .push_request(
                    &request_info,
                    |meta| {
                        good_meta(meta)?;
                        match case {
                            0 => meta.set_correlation_header(&[1; 33]),
                            1 => {
                                meta.set_correlation_header(b"first")?;
                                meta.set_correlation_header(b"second")
                            }
                            2 => meta.correlate_imp(1, CorrelationMedia::Video, 20),
                            3 => {
                                meta.correlate_imp(0, CorrelationMedia::Video, 20)?;
                                meta.correlate_imp(0, CorrelationMedia::Banner, 30)
                            }
                            4 => meta.imp_id("imp-1"),
                            _ => {
                                let media = CorrelationMedia::try_from(5)?;
                                meta.correlate_imp(0, media, 20)
                            }
                        }
                    },
                    witness.closure(),
                )
                .unwrap_err();
            assert_eq!(
                error,
                OutboundBuildError::Correlation(expected),
                "case {case}"
            );
            assert_eq!(budget.remaining(), 1, "case {case} must refund immediately");
            witness.assert_never_ran();
            assert_eq!(sink.accepted_count(), 1);
            sink.push_request(
                &request_info,
                |meta| {
                    good_meta(meta)?;
                    meta.correlate_imp(0, CorrelationMedia::Video, 40)
                },
                small_body,
            )
            .unwrap();
            let outcome = sink.into_outcome();
            assert_eq!(budget.remaining(), 0);
            assert_eq!(outcome.errors, [OutboundBuildError::Correlation(expected)]);
            assert_eq!(outcome.accepted.len(), 2);
            let mut calls = outcome.accepted.into_iter();
            let (first, _) = calls.next().unwrap().into_parts();
            let (second, _) = calls.next().unwrap().into_parts();
            assert_eq!(first.correlation_header(), Some(b"incumbent".as_slice()));
            assert_eq!(first.imp_correlation(0).unwrap().token(), 10);
            assert_eq!(second.imp_correlation(0).unwrap().token(), 40);
            assert_eq!(
                second.imp_correlation(0).unwrap().media(),
                CorrelationMedia::Video
            );
        }
    }

    #[test]
    fn sink_error_list_bounded_with_suppressed_count() {
        let witness = BodyWitness::default();
        let budget = CallBudget::new(64);
        let validator = AcceptAll;
        let macros: &[&str] = &[];
        let currency: &[String] = &[];
        let request_info = info(macros, currency);
        let mut sink = OutboundRequestSink::new(limits(), &budget, &validator);

        // Sixteen distinct typed errors fill the retained window.
        for _ in 0..MAX_RETAINED_SINK_ERRORS {
            let err = sink
                .push_request(&request_info, |meta| meta.set_url(URL), witness.closure())
                .unwrap_err();
            assert_eq!(err, OutboundBuildError::MissingMethod);
        }

        let mut probe = OutboundRequestSink::new(limits(), &budget, &validator);
        probe
            .push_request(&request_info, good_meta, small_body)
            .unwrap();
        drop(probe);

        // The seventeenth is suppressed rather than retained.
        let err = sink
            .push_request(&request_info, |meta| meta.set_url(URL), witness.closure())
            .unwrap_err();
        assert_eq!(err, OutboundBuildError::MissingMethod);

        let outcome = sink.into_outcome();
        assert_eq!(outcome.errors.len(), MAX_RETAINED_SINK_ERRORS);
        assert_eq!(outcome.suppressed_error_count, 1);
        assert_eq!(
            outcome.errors.capacity(),
            MAX_RETAINED_SINK_ERRORS,
            "the error list must never grow past its fixed window"
        );
        assert_eq!(
            budget.remaining(),
            64,
            "every failed push refunded, so the whole budget is intact"
        );
        witness.assert_never_ran();
    }

    #[test]
    fn budget_rejection_count_drives_10999_n() {
        let witness = BodyWitness::default();
        let budget = CallBudget::new(1);
        let validator = AcceptAll;
        let macros: &[&str] = &[];
        let currency: &[String] = &[];
        let request_info = info(macros, currency);
        let mut sink = OutboundRequestSink::new(limits(), &budget, &validator);

        sink.push_request(&request_info, good_meta, small_body)
            .unwrap();

        // A bidder that keeps pushing: every refusal counts, while the
        // reason is recorded once.
        for _ in 0..3_u8 {
            let err = sink
                .push_request(&request_info, good_meta, witness.closure())
                .unwrap_err();
            assert_eq!(
                err,
                OutboundBuildError::CallBudgetExhausted { max_calls: 1 }
            );
        }

        let outcome = sink.into_outcome();
        assert_eq!(outcome.accepted.len(), 1);
        assert_eq!(outcome.rejected_call_count, 3);
        assert_eq!(
            outcome.errors,
            vec![OutboundBuildError::CallBudgetExhausted { max_calls: 1 }],
            "the exhaustion reason belongs in the list once; the count carries N"
        );
        assert_eq!(outcome.suppressed_error_count, 0);

        // A bidder that stops at its first exhaustion yields N = 1.
        let stop_budget = CallBudget::new(1);
        let mut stopper = OutboundRequestSink::new(limits(), &stop_budget, &validator);
        stopper
            .push_request(&request_info, good_meta, small_body)
            .unwrap();
        stopper
            .push_request(&request_info, good_meta, witness.closure())
            .unwrap_err();
        assert_eq!(stopper.into_outcome().rejected_call_count, 1);
        witness.assert_never_ran();
    }

    #[test]
    fn endpoint_validator_rejects_before_any_body_is_built() {
        let witness = BodyWitness::default();
        let all = [
            EndpointRejectReason::SchemeNotAllowed,
            EndpointRejectReason::HostNotAllowed,
            EndpointRejectReason::IpLiteralRejected,
            EndpointRejectReason::TemplateUnresolved,
            EndpointRejectReason::PortNotAllowed,
            EndpointRejectReason::MalformedUrl,
            EndpointRejectReason::UserinfoRejected,
            EndpointRejectReason::AuthorityMismatch,
        ];
        let macros: &[&str] = &[];
        let currency: &[String] = &[];
        let request_info = info(macros, currency);

        for reason in all {
            let budget = CallBudget::new(1);
            let validator = Rejecting::new(reason);
            let mut sink = OutboundRequestSink::new(limits(), &budget, &validator);

            // `never_body` panics if the body stage is ever reached.
            let err = sink
                .push_request(&request_info, good_meta, witness.closure())
                .unwrap_err();
            assert_eq!(err, OutboundBuildError::EndpointRejected { reason });
            assert_eq!(validator.calls.get(), 1);
            assert_eq!(
                budget.remaining(),
                1,
                "a rejected endpoint must not consume a call slot"
            );

            // The refunded slot still dispatches a sibling request — one
            // bad endpoint does not cost the bidder its other calls.
            let accepting = AcceptAll;
            let mut survivor = OutboundRequestSink::new(limits(), &budget, &accepting);
            survivor
                .push_request(&request_info, good_meta, small_body)
                .unwrap();
            let outcome = survivor.into_outcome();
            assert_eq!(outcome.accepted.len(), 1);
            assert_eq!(
                outcome.rejected_call_count, 0,
                "an endpoint rejection is not a budget rejection"
            );
            witness.assert_never_ran();
        }
    }

    #[test]
    fn missing_method_refunds_reservation_and_never_invokes_body() {
        let witness = BodyWitness::default();
        let budget = CallBudget::new(1);
        let validator = AcceptAll;
        let macros: &[&str] = &[];
        let currency: &[String] = &[];
        let request_info = info(macros, currency);
        let mut sink = OutboundRequestSink::new(limits(), &budget, &validator);

        // Metadata returns Ok, but never set the method.
        let err = sink
            .push_request(&request_info, |meta| meta.set_url(URL), witness.closure())
            .unwrap_err();
        assert_eq!(err, OutboundBuildError::MissingMethod);
        assert_eq!(budget.remaining(), 1);

        // At capacity one, the next push succeeds — the slot came back.
        sink.push_request(&request_info, good_meta, small_body)
            .unwrap();
        let outcome = sink.into_outcome();
        assert_eq!(outcome.accepted.len(), 1);
        assert_eq!(outcome.errors, vec![OutboundBuildError::MissingMethod]);
        assert_eq!(outcome.rejected_call_count, 0);
        witness.assert_never_ran();
    }

    #[test]
    fn count_rejection_after_full_error_window_is_not_a_suppressed_build_failure() {
        let budget = CallBudget::new(1);
        let request_info = info(&[], &[]);
        let mut sink = OutboundRequestSink::new(limits(), &budget, &AcceptAll);
        for _ in 0..MAX_RETAINED_SINK_ERRORS {
            sink.push_request(&request_info, |meta| meta.set_url(URL), small_body)
                .unwrap_err();
        }
        sink.push_request(&request_info, good_meta, small_body)
            .unwrap();
        sink.push_request(&request_info, good_meta, small_body)
            .unwrap_err();
        let outcome = sink.into_outcome();
        assert_eq!(outcome.rejected_call_count, 1);
        assert_eq!(outcome.suppressed_error_count, 0);
        assert_eq!(outcome.errors.len(), MAX_RETAINED_SINK_ERRORS);
    }

    #[test]
    fn missing_url_refunds_reservation_and_never_invokes_body() {
        let witness = BodyWitness::default();
        let budget = CallBudget::new(1);
        let validator = AcceptAll;
        let macros: &[&str] = &[];
        let currency: &[String] = &[];
        let request_info = info(macros, currency);
        let mut sink = OutboundRequestSink::new(limits(), &budget, &validator);

        let err = sink
            .push_request(
                &request_info,
                |meta| meta.set_method("POST"),
                witness.closure(),
            )
            .unwrap_err();
        assert_eq!(err, OutboundBuildError::MissingUrl);
        assert_eq!(budget.remaining(), 1);

        sink.push_request(&request_info, good_meta, small_body)
            .unwrap();
        let outcome = sink.into_outcome();
        assert_eq!(outcome.accepted.len(), 1);
        assert_eq!(outcome.errors, vec![OutboundBuildError::MissingUrl]);
        witness.assert_never_ran();
    }

    #[test]
    fn semantic_err_discard_refunds_every_retained_reservation() {
        let budget = CallBudget::new(2);
        let validator = AcceptAll;
        let macros: &[&str] = &[];
        let currency: &[String] = &[];
        let request_info = info(macros, currency);
        let mut sink = OutboundRequestSink::new(limits(), &budget, &validator);

        sink.push_request(&request_info, good_meta, small_body)
            .unwrap();
        sink.push_request(&request_info, good_meta, small_body)
            .unwrap();
        assert_eq!(budget.remaining(), 0);

        // The bidder then fails semantically.
        let outcome = sink.into_outcome_discarding_accepted();
        assert!(outcome.accepted.is_empty());
        assert!(outcome.errors.is_empty());
        assert_eq!(outcome.rejected_call_count, 0);

        // Nothing was committed during push_request, so both slots are
        // reservable again.
        let first = budget.try_reserve();
        let second = budget.try_reserve();
        assert!(first.is_some() && second.is_some());
        drop(first);
        drop(second);
        assert_eq!(budget.remaining(), 2);
    }

    #[test]
    fn accepted_request_carries_method_url_headers_and_imp_ids() {
        let budget = CallBudget::new(1);
        let validator = AcceptAll;
        let macros: &[&str] = &[];
        let currency: &[String] = &[];
        let request_info = info(macros, currency);
        let mut sink = OutboundRequestSink::new(limits(), &budget, &validator);

        sink.push_request(
            &request_info,
            |meta| {
                meta.set_method("POST")?;
                meta.set_url(URL)?;
                meta.header("X-Openrtb-Version", "2.6")?;
                meta.imp_id("imp-1")?;
                meta.imp_id("imp-2")
            },
            // Byte-string fixture: `write_all` takes `&[u8]`, and the
            // assertion below compares the accepted body against these
            // exact bytes, so the literal IS the expected value.
            |body| body.write_all(br#"{"id":"req-1"}"#),
        )
        .unwrap();

        let outcome = sink.into_outcome();
        let accepted = outcome.accepted.first().unwrap();
        assert_eq!(accepted.method(), "POST");
        assert_eq!(accepted.url(), URL);
        assert_eq!(
            accepted.headers().collect::<Vec<(&str, &str)>>(),
            vec![("X-Openrtb-Version", "2.6")]
        );
        assert_eq!(accepted.body(), br#"{"id":"req-1"}"#);
        assert_eq!(
            accepted.covered_imp_ids().collect::<Vec<&str>>(),
            ["imp-1", "imp-2"]
        );

        // The dispatchable request MOVES the body: the sink's one
        // allocation is what goes on the wire, and the pointer proves
        // it rather than the length doing so.
        let outbound = outcome.accepted.into_iter().next().unwrap();
        let body_ptr = outbound.body().as_ptr();
        let (meta, body) = outbound.into_parts();
        assert!(
            ptr::eq(body.as_ptr(), body_ptr),
            "the dispatched body is the sink's buffer, not a copy of it",
        );
        assert_eq!(
            meta.covered_imp_ids().collect::<Vec<&str>>(),
            ["imp-1", "imp-2"],
            "and the metadata is still here for response processing",
        );
    }

    #[test]
    fn meta_abort_maps_onto_its_own_build_error_variant() {
        let witness = BodyWitness::default();
        let budget = CallBudget::new(8);
        let validator = AcceptAll;
        let macros: &[&str] = &[];
        let currency: &[String] = &[];
        let request_info = info(macros, currency);
        let mut sink = OutboundRequestSink::new(limits(), &budget, &validator);

        let reserved = sink
            .push_request(
                &request_info,
                |meta| {
                    meta.set_method("POST")?;
                    meta.set_url(URL)?;
                    meta.header("Content-Length", "12")
                },
                witness.closure(),
            )
            .unwrap_err();
        assert_eq!(
            reserved,
            OutboundBuildError::ReservedHeader {
                name: "Content-Length".to_owned()
            }
        );

        let duplicate = sink
            .push_request(
                &request_info,
                |meta| {
                    meta.set_method("POST")?;
                    meta.set_url(URL)?;
                    meta.set_url(URL)
                },
                witness.closure(),
            )
            .unwrap_err();
        assert!(matches!(
            duplicate,
            OutboundBuildError::DuplicateMeta { .. }
        ));

        // A one-imp sink refuses the second id, and the limit travels
        // from `SinkLimits` into the meta builder's offset table.
        let mut narrow = OutboundRequestSink::new(SinkLimits::new(4_096, 1), &budget, &validator);
        let too_many_imps = narrow
            .push_request(
                &request_info,
                |meta| {
                    meta.set_method("POST")?;
                    meta.set_url(URL)?;
                    meta.imp_id("imp-1")?;
                    meta.imp_id("imp-2")
                },
                witness.closure(),
            )
            .unwrap_err();
        assert_eq!(
            too_many_imps,
            OutboundBuildError::TooManyImpIds { limit: 1 }
        );

        let over_cap = sink
            .push_request(
                &request_info,
                |meta| {
                    meta.set_method("POST")?;
                    meta.set_url(&"x".repeat(META_BIDDER_REGION_BYTES + 1))
                },
                witness.closure(),
            )
            .unwrap_err();
        assert!(matches!(over_cap, OutboundBuildError::MetaOverCap { .. }));
        witness.assert_never_ran();
    }

    #[test]
    fn unparsable_url_is_rejected_before_validation_and_before_the_body() {
        let witness = BodyWitness::default();
        let budget = CallBudget::new(1);
        let validator = Rejecting::new(EndpointRejectReason::HostNotAllowed);
        let macros: &[&str] = &[];
        let currency: &[String] = &[];
        let request_info = info(macros, currency);
        let mut sink = OutboundRequestSink::new(limits(), &budget, &validator);

        let err = sink
            .push_request(
                &request_info,
                |meta| {
                    meta.set_method("POST")?;
                    meta.set_url("not-a-url")
                },
                witness.closure(),
            )
            .unwrap_err();
        assert_eq!(err, OutboundBuildError::MalformedUrl);
        assert_eq!(
            validator.calls.get(),
            0,
            "an unparsable URL never reaches the deployment's policy"
        );
        assert_eq!(budget.remaining(), 1);
        witness.assert_never_ran();
    }
}
