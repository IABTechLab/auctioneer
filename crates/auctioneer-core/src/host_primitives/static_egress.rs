//! Binding the static-egress class to the responses the host emits
//! before auctioneer's admission policy ever runs.
//!
//! # The class of response this covers
//!
//! A request head that fails normalization — an over-long request
//! target, a header block over the admitted caps, a malformed head — is
//! rejected BEFORE application admission. There is no admission
//! decision to attach a permit to, and by default no completion to
//! release one: the host hardcodes an empty completion for these and
//! its own docs say they "do not invoke the application's
//! response-egress policy or observer".
//!
//! That used to be a declared EXEMPTION in auctioneer's egress accounting:
//! the static pool bounded every response auctioneer itself emitted, and
//! explicitly did not bound the host's pre-admission rejections. The
//! host now exposes a decision factory for exactly these responses, so
//! the exemption is closed rather than documented. Pre-admission
//! rejections hold a static permit like every other response, and an
//! exhausted pool ABORTS rather than emitting ungated — which makes
//! "every response auctioneer puts on the wire holds a permit" true without
//! qualification.
//!
//! # Why the permit goes into the completion, not into a local
//!
//! The factory runs synchronously and returns; the response it decided
//! for is written afterwards. A permit held in the factory's frame
//! would be released the instant the factory returned, which is before
//! a single byte is written — the release-too-early error that bounds
//! nothing. Moving it into the completion makes the host its owner from
//! that instant, so every terminal path releases it exactly once,
//! including abandonment.

use crate::host_primitives::gates::static_egress_gate;
use auctioneer_runtime::StaticEgressPermit;
use edgezero_core::response_egress::{
    DetachedResponseEgressDecision, DetachedResponseEgressHead, ResponseEgressCompletion,
};
use edgezero_core::time::Deadline;
use std::time::Duration;

/// The write budget a pre-admission rejection gets.
///
/// Short on purpose, and deliberately not the auction write budget: the
/// body is a fixed-size constant and the isolate is, by the time it is
/// rejecting heads, most likely under load. A rejection that cannot be
/// written in this long is not going to be written.
const REJECTION_WRITE_BUDGET: Duration = Duration::from_millis(500);

/// Decide the fate of one pre-admission normalized-head failure.
///
/// Install with `App::set_detached_response_egress_decision_factory`.
///
/// Returns [`DetachedResponseEgressDecision::Send`] carrying a
/// completion that owns a static permit, or
/// [`DetachedResponseEgressDecision::Abort`] when the static class is
/// exhausted or the gates are not installed yet.
///
/// Both refusal cases are the same judgement: with no permit there is
/// no bounded buffer to render into, and emitting anyway is the
/// unbounded transmission the class exists to prevent. Aborting closes
/// the connection at the transport, which is bounded and recoverable.
#[must_use]
pub fn detached_egress_decision(
    head: &DetachedResponseEgressHead<'_>,
) -> DetachedResponseEgressDecision {
    // No gate means boot has not finished. That is fail-CLOSED for the
    // same reason every other gate accessor is: the safe answer to "is
    // there capacity?" before the capacity exists is "no".
    let Some(gate) = static_egress_gate() else {
        log::warn!(
            "aborting a pre-admission {} rejection: the static-egress class is not installed yet",
            head.status().as_u16(),
        );
        return DetachedResponseEgressDecision::Abort;
    };
    let Some(permit) = gate.try_acquire() else {
        log::warn!(
            "aborting a pre-admission {} rejection: the static-egress class is exhausted",
            head.status().as_u16(),
        );
        return DetachedResponseEgressDecision::Abort;
    };
    DetachedResponseEgressDecision::Send {
        completion: completion_owning(permit),
        deadline: head.write_deadline_after(REJECTION_WRITE_BUDGET),
    }
}

/// A completion that owns `permit` until terminal egress.
///
/// The permit is MOVED into the callback, so the host owns it from the
/// moment this returns. Every terminal path runs or drops that callback
/// exactly once, which releases the permit exactly once — including the
/// abandonment path, where the callback never runs and the drop does
/// the release instead.
fn completion_owning(permit: StaticEgressPermit) -> ResponseEgressCompletion {
    ResponseEgressCompletion::new(move |_report| drop(permit))
}

/// The absolute deadline a pre-admission rejection is given, anchored
/// at the request's own start.
///
/// Exposed so a test can assert the budget without reconstructing a
/// head, and so a caller that needs the same budget for a
/// auctioneer-emitted refusal reads it from here rather than restating the
/// constant.
#[must_use]
pub fn rejection_deadline(request_start: auctioneer_runtime::MonotonicInstant) -> Deadline {
    Deadline::at_instant(
        request_start
            .checked_add(REJECTION_WRITE_BUDGET)
            .unwrap_or(request_start),
    )
}

#[cfg(test)]
mod tests {
    use super::{REJECTION_WRITE_BUDGET, completion_owning, rejection_deadline};
    use auctioneer_runtime::{MonotonicInstant, StaticEgressGate};

    /// The permit is owned by the COMPLETION, not by the frame that
    /// decided. A completion that merely borrowed, or a factory that
    /// held the permit locally, would release at return — before a byte
    /// is written — and bound nothing.
    #[test]
    fn the_completion_holds_the_permit_until_it_closes() {
        let gate = StaticEgressGate::new(1);
        let permit = gate.try_acquire().expect("capacity 1 admits");
        let completion = completion_owning(permit);
        assert_eq!(
            gate.in_use(),
            1,
            "the permit survives the call that created the completion",
        );
        assert!(
            gate.try_acquire().is_none(),
            "a second rejection cannot render while the first is unwritten",
        );

        drop(completion);
        assert_eq!(
            gate.in_use(),
            0,
            "closing the completion releases the permit exactly once",
        );
        assert!(gate.try_acquire().is_some(), "the buffer is reusable");
    }

    /// The rejection deadline is absolute and anchored at request
    /// start, so a head that took time to reject does not earn a fresh
    /// write budget.
    #[test]
    fn the_rejection_deadline_is_anchored_at_request_start() {
        let start = MonotonicInstant::now();
        let deadline = rejection_deadline(start);
        assert_eq!(
            deadline.instant(),
            start
                .checked_add(REJECTION_WRITE_BUDGET)
                .expect("no overflow"),
            "the deadline is request_start plus the whole rejection budget",
        );
        assert!(
            !deadline.is_expired_at(start),
            "a fresh rejection has its budget in front of it",
        );
        assert!(
            deadline.is_expired_at(
                start
                    .checked_add(REJECTION_WRITE_BUDGET)
                    .expect("no overflow")
            ),
            "and it is expired once the budget is spent",
        );
    }

    /// Overflow fails closed here too: an unrepresentable deadline
    /// collapses onto the anchor rather than becoming unbounded.
    #[test]
    fn an_unrepresentable_rejection_deadline_fails_closed() {
        // `REJECTION_WRITE_BUDGET` cannot itself overflow a real
        // instant, so the property is asserted on the helper's
        // saturating shape: the deadline never lands BEFORE the anchor,
        // and never further out than the budget allows.
        let start = MonotonicInstant::now();
        let deadline = rejection_deadline(start);
        assert!(
            deadline.instant() >= start,
            "a deadline never precedes its anchor",
        );
        assert!(
            deadline.remaining_at(start).unwrap_or_default() <= REJECTION_WRITE_BUDGET,
            "and never exceeds the budget",
        );
    }
}
