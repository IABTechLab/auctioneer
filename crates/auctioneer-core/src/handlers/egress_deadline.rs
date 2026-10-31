//! The response-write deadline, its checkpoints, and the absolute
//! deadline auctioneer hands the host.
//!
//! # The leg this covers, and the leg it does not
//!
//! A response's write budget spans four points. Three of them are
//! auctioneer's own, and this module owns them:
//!
//! 1. **Before the egress-class permit is claimed** — the deadline is
//!    checked first, so a response that already spent its whole budget
//!    reaching this point is never serialized and never takes a slot a
//!    live response could use. There is no queue to wait in: the gate
//!    refuses immediately when it is full, so a saturated isolate sheds
//!    work rather than buffering it.
//! 2. **On permit grant, before serialization** — recheck. A permit
//!    that arrives after the budget is gone is returned unused rather
//!    than spent producing bytes nobody will read.
//! 3. **After serialization, before the first byte** — recheck, and
//!    DROP THE BUFFER on expiry. Serialization itself takes time, and a
//!    buffer built past the deadline is dead weight that would occupy
//!    a permit and a transmit buffer for a response the client has
//!    stopped waiting for.
//!
//! The fourth point — write-complete or client-disconnect — is the
//! HOST's, enforced from the [`ResponseEgressDeadline`] this module
//! attaches. auctioneer attaches the absolute deadline and does not
//! reimplement the transmission leg. Whether a given target actually
//! delivers a terminal report for that leg is a per-target capability
//! question and is certified elsewhere; attaching the deadline is
//! unconditional either way, because the attachment is what makes the
//! host able to enforce it at all.
//!
//! # Why the deadline is absolute, and where it is anchored
//!
//! Absolute, so the three checkpoints at three different times all
//! measure against ONE allowance rather than each restarting the clock.
//! Every checkpoint reads one clock in one domain — the host's own
//! per-request monotonic clock — so no projection sits between the
//! checkpoints and the deadline the host later enforces.
//!
//! The ANCHOR depends on what the response is, and the two cases are
//! deliberately different:
//!
//! - **A normal auction response** anchors at EGRESS ENTRY: the instant
//!   the auction resolved and the handler started producing bytes. This
//!   budget bounds the WRITE leg, and the auction's own elapsed time is
//!   already bounded by tmax — charging it to both means a request that
//!   spent most of its tmax arrives at egress with almost none of its
//!   write allowance left, and a slow enough one is handed a deadline
//!   already in the past. Then even the refusal that fires goes out
//!   carrying that same expired value.
//! - **A static response or a refusal** anchors at ITSELF, with its own
//!   fresh and much shorter [`REFUSAL_WRITE_BUDGET`]. A refusal born
//!   from an expired auction budget cannot inherit it — the 503 would
//!   abort before its first byte — and a refusal born during admission
//!   has no auction budget to inherit at all, so with no explicit value
//!   it falls to the host's own far wider default and a slow refusal
//!   reader holds static capacity far longer than the class is sized
//!   for.

use crate::host_primitives::gates::boot_limits;
use auctioneer_runtime::{EgressGate, EgressPermit, MonotonicInstant};
use edgezero_core::response_egress::{DEFAULT_RESPONSE_WRITE_BUDGET, ResponseEgressDeadline};
use edgezero_core::time::Deadline;
use std::sync::Arc;
use std::time::Duration;

/// The write budget used when the boot envelope declares none.
///
/// Reached only before the envelope is installed — which is also when
/// every gate accessor answers `None`, so there is no egress capacity
/// being contended for and nothing for a shed decision to protect.
///
/// It is therefore deliberately WIDE rather than short. A short
/// fallback would shed real responses for a condition that reflects
/// boot state rather than load: an in-process caller with no envelope
/// installed would see its answer discarded as overload while no gate
/// was bounding anything at all. Matching the host's own default
/// response-write budget keeps the checkpoints installed and honest
/// without inventing a bound nobody configured.
const FALLBACK_WRITE_BUDGET: Duration = DEFAULT_RESPONSE_WRITE_BUDGET;

/// The write budget every static response and every refusal gets, fresh,
/// anchored at the response itself.
///
/// Deliberately SHORT, and deliberately not the auction budget. The
/// static-response class is a small fixed pool of render buffers, and
/// the whole argument that a refusal can always be answered is that a
/// held permit drains within this budget. A refusal left on the host's
/// own default write budget drains an order of magnitude slower, so the
/// pool is sized against one number while the wire behaves like
/// another, and a handful of slow refusal readers hold the class
/// indefinitely.
///
/// Relative rather than absolute: the host resolves it at the true
/// transmission start, which is both the instant the class actually
/// starts paying and strictly later than anything auctioneer can sample.
/// Anchoring it here at construction would silently shorten it by
/// however long the response waits to be picked up.
///
/// A CONSTANT rather than an envelope row, which is a known gap and not
/// a preference: the value has to match a pool size, so an operator who
/// raises `terminal_response_permits` should be able to raise this with
/// it. `ResolvedIsolateLimits` carries no field for it yet; when it
/// gains one this constant becomes that field's default and the reader
/// below reads the envelope the way the auction budget already does.
pub const REFUSAL_WRITE_BUDGET: Duration = Duration::from_secs(2);

/// The write budget the boot envelope declares for a normal response.
fn envelope_write_budget() -> Duration {
    boot_limits().map_or(FALLBACK_WRITE_BUDGET, |limits| {
        Duration::from_millis(limits.response_write_deadline_ms)
    })
}

/// One response's write budget, as an absolute instant.
///
/// `Copy`, so a checkpoint can read it without consuming it, and
/// absolute, so three checkpoints at three different times all measure
/// against the same allowance.
#[derive(Clone, Copy, Debug)]
pub struct EgressDeadline {
    deadline: Deadline,
}

impl EgressDeadline {
    /// The envelope's write budget, anchored at the instant the handler
    /// ENTERS egress.
    ///
    /// Call this with the instant the response began to exist — after
    /// the auction resolved, not when the request was admitted. The
    /// budget bounds the write leg and nothing else: the auction's own
    /// time is bounded by tmax, and spending it from here as well would
    /// leave a slow-but-successful auction with a deadline in the past
    /// and shed its completed answer as overload.
    #[must_use]
    pub fn from_boot_envelope(egress_entry: MonotonicInstant) -> Self {
        Self::new(egress_entry, envelope_write_budget())
    }

    /// A refusal's own fresh budget, anchored at the refusal.
    ///
    /// Used by every static-class answer this handler produces, so a
    /// refusal born out of an auction whose budget is gone gets a live
    /// deadline rather than inheriting an expired one.
    #[must_use]
    pub fn for_refusal(refused_at: MonotonicInstant) -> Self {
        Self::new(refused_at, REFUSAL_WRITE_BUDGET)
    }

    /// The absolute deadline, for a caller that needs the raw instant.
    #[must_use]
    pub const fn deadline(self) -> Deadline {
        self.deadline
    }

    /// Whether the budget is already gone as of `now`.
    #[must_use]
    pub fn is_expired_at(self, now: MonotonicInstant) -> bool {
        self.deadline.is_expired_at(now)
    }

    /// An explicit budget from an explicit anchor.
    ///
    /// Overflow fails CLOSED — an unrepresentable instant yields a
    /// deadline at the anchor, so the response is treated as already
    /// expired rather than granted an unbounded write.
    #[must_use]
    pub fn new(anchor: MonotonicInstant, budget: Duration) -> Self {
        Self {
            deadline: Deadline::at_instant(anchor.checked_add(budget).unwrap_or(anchor)),
        }
    }

    /// Time left as of `now`, or `None` once the budget is gone.
    #[must_use]
    pub fn remaining_at(self, now: MonotonicInstant) -> Option<Duration> {
        self.deadline.remaining_at(now)
    }

    /// The value the host enforces the transmission leg from.
    ///
    /// Absolute rather than relative: a relative budget would restart
    /// at the instant egress begins, handing a response that already
    /// spent its allowance in the queue a fresh one.
    #[must_use]
    pub const fn to_response_extension(self) -> ResponseEgressDeadline {
        ResponseEgressDeadline::at(self.deadline)
    }
}

/// The one wire body an isolate-capacity refusal carries.
///
/// Appendix F gives this shape to BOTH overload surfaces — the
/// admission gate's pre-body `503` and this module's post-auction
/// egress shed — because they are the same fact from the client's side:
/// the isolate had no capacity for this request. It lives here, with
/// the egress checkpoints, and the admission policy imports it, so
/// there is exactly one definition to compare against the spec.
///
/// The trailing newline is part of the pinned contract.
pub const OVERLOADED_BODY: &str = "overloaded\n";

/// Why a response was abandoned before it reached the wire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EgressRefusal {
    /// The write budget was already gone before an egress permit was
    /// claimed. Named for the QUEUE position this checkpoint occupies
    /// in the write-budget contract, not for a queue the gate has —
    /// the gate is non-queueing, and the refusal is immediate.
    DeadlineInQueue,
    /// A permit was granted, but the budget was already gone. The
    /// permit is returned unused.
    DeadlineOnGrant,
    /// Serialization finished past the deadline. The serialized buffer
    /// is dropped un-transmitted.
    DeadlineAfterSerialization,
    /// The egress class is full and the budget does not allow waiting
    /// for it.
    GateFull,
}

impl EgressRefusal {
    /// The wire body every refusal in this module produces.
    ///
    /// All four are isolate-capacity overload from the client's side —
    /// the response was computed and could not be sent within its
    /// budget — so they share one body rather than leaking which
    /// internal checkpoint fired. The trailing newline is part of the
    /// pinned contract.
    ///
    /// It is [`OVERLOADED_BODY`], not a second spelling of it: the
    /// admission gate refuses with the same bytes, and Appendix F
    /// specifies ONE overload shape for both. Two literals would be two
    /// chances to disagree with the spec, and one of them did.
    #[must_use]
    pub const fn body(self) -> &'static str {
        OVERLOADED_BODY
    }

    /// The checkpoint name, for logs and metrics.
    #[must_use]
    pub const fn checkpoint(self) -> &'static str {
        match self {
            Self::DeadlineInQueue => "queue",
            Self::DeadlineOnGrant => "grant",
            Self::DeadlineAfterSerialization => "post_serialization",
            Self::GateFull => "gate_full",
        }
    }
}

/// Acquire an egress permit, racing the acquisition against the write
/// deadline.
///
/// This is checkpoints 1 and 2 as one operation, because they are one
/// decision: either a permit is in hand with budget left, or the
/// response is shed. Splitting them into two public calls would let a
/// caller do the grant without the recheck.
///
/// There is no queue and no retry: the gate is non-queueing, and this
/// makes exactly one attempt. A saturated gate is refused immediately
/// rather than parked on, which is the shed-fast behaviour an
/// overloaded isolate wants — a response that waits for a slot is a
/// response still holding an auction's worth of memory. Adding a
/// bounded retry would need a timer to sleep on, which is a different
/// unit's dependency, and a busy-wait would burn the very CPU the
/// saturation is competing for.
///
/// `now` is sampled by the caller's clock rather than read here, so the
/// whole request stays in one clock domain and a test can step time
/// deterministically.
///
/// # Errors
/// Returns the refusal that fired. [`EgressRefusal::DeadlineInQueue`]
/// when the budget was already gone on entry,
/// [`EgressRefusal::GateFull`] when the gate had no free slot, and
/// [`EgressRefusal::DeadlineOnGrant`] when the budget ran out between
/// the two clock reads.
pub fn acquire_within_deadline<Now>(
    gate: &Arc<EgressGate>,
    deadline: EgressDeadline,
    mut now: Now,
) -> Result<EgressPermit, EgressRefusal>
where
    Now: FnMut() -> MonotonicInstant,
{
    // CHECKPOINT 1 — the queue wait races the deadline. Checked BEFORE
    // the first try_acquire, so a response that arrives already expired
    // never takes a slot a live response could use.
    if deadline.is_expired_at(now()) {
        return Err(EgressRefusal::DeadlineInQueue);
    }
    let permit = gate.try_acquire().ok_or(EgressRefusal::GateFull)?;
    // CHECKPOINT 2 — recheck ON GRANT, before serialization. The
    // permit is returned by dropping it on the error path, so a
    // response that lost its budget between the two reads does not
    // strand a slot.
    if deadline.is_expired_at(now()) {
        drop(permit);
        return Err(EgressRefusal::DeadlineOnGrant);
    }
    Ok(permit)
}

/// Checkpoint 3: recheck after serialization, before the first byte.
///
/// Takes the serialized buffer BY VALUE and gives it back only when the
/// budget survives, so an expired response cannot accidentally be
/// transmitted: the caller has no buffer left to send. That is the
/// "drops the buffer" half of the contract made structural rather than
/// left to a caller's discipline.
///
/// # Errors
/// Returns [`EgressRefusal::DeadlineAfterSerialization`] when the
/// budget expired while the response was being serialized. The buffer
/// is dropped inside this call.
pub fn transmit_within_deadline(
    serialized: Vec<u8>,
    deadline: EgressDeadline,
    now: MonotonicInstant,
) -> Result<Vec<u8>, EgressRefusal> {
    if deadline.is_expired_at(now) {
        // Explicit rather than relying on the move: the whole point of
        // this checkpoint is that the bytes do NOT survive it.
        drop(serialized);
        return Err(EgressRefusal::DeadlineAfterSerialization);
    }
    Ok(serialized)
}

#[cfg(test)]
mod tests {
    use super::{EgressDeadline, EgressRefusal, acquire_within_deadline, transmit_within_deadline};
    use auctioneer_runtime::{EgressGate, MonotonicInstant};
    use std::cell::Cell;
    use std::time::Duration;

    /// A hand-advanced monotonic source. The checkpoints read a clock
    /// through a closure precisely so a test can step time
    /// deterministically rather than sleeping.
    struct StepClock {
        now: Cell<MonotonicInstant>,
    }

    impl StepClock {
        fn advance(&self, by: Duration) {
            let next = self
                .now
                .get()
                .checked_add(by)
                .expect("test instants do not overflow");
            self.now.set(next);
        }

        /// Anchored at a real instant, then advanced only by hand.
        /// `MonotonicInstant` is an opaque platform instant with no
        /// constructor from a raw count, so the base has to come from
        /// the clock; every step after that is explicit, so the tests
        /// stay deterministic without sleeping.
        fn new() -> Self {
            Self {
                now: Cell::new(MonotonicInstant::now()),
            }
        }

        fn now(&self) -> MonotonicInstant {
            self.now.get()
        }
    }

    /// Checkpoint 1: a response whose budget is gone before a permit is
    /// even attempted is shed in the QUEUE, and the gate is untouched —
    /// so it never occupies a slot, and it is never serialized.
    #[test]
    fn egress_deadline_expires_in_queue_before_serialization() {
        let clock = StepClock::new();
        let gate = EgressGate::new(1);
        let deadline = EgressDeadline::new(clock.now(), Duration::from_millis(10));

        clock.advance(Duration::from_millis(11));
        let refused = acquire_within_deadline(&gate, deadline, || clock.now())
            .expect_err("an expired budget cannot acquire");

        assert_eq!(refused, EgressRefusal::DeadlineInQueue);
        assert_eq!(refused.checkpoint(), "queue");
        assert_eq!(
            gate.in_use(),
            0,
            "a queue-expired response must not take an egress slot",
        );
        assert_eq!(
            gate.capacity(),
            1,
            "and the slot is still there for a live response",
        );
        assert!(
            gate.try_acquire().is_some(),
            "the next response acquires the slot the shed one never took",
        );
    }

    /// Checkpoint 2: a permit granted after the budget ran out is
    /// RETURNED, not spent. The distinguishing assertion is the gate
    /// counter after the refusal: a permit leaked here would show as an
    /// occupied slot with no response behind it.
    #[test]
    fn egress_deadline_on_grant_returns_the_permit_unused() {
        let clock = StepClock::new();
        let gate = EgressGate::new(1);
        let deadline = EgressDeadline::new(clock.now(), Duration::from_millis(10));

        // Live on entry, expired by the second read: exactly the window
        // checkpoint 2 exists to close.
        let first = Cell::new(true);
        let refused = acquire_within_deadline(&gate, deadline, || {
            if first.replace(false) {
                clock.now()
            } else {
                clock.advance(Duration::from_millis(11));
                clock.now()
            }
        })
        .expect_err("a budget lost between the two reads refuses");

        assert_eq!(refused, EgressRefusal::DeadlineOnGrant);
        assert_eq!(refused.checkpoint(), "grant");
        assert_eq!(
            gate.in_use(),
            0,
            "the granted permit was returned rather than spent on serialization",
        );
    }

    /// The happy path: budget intact, permit acquired, slot held.
    #[test]
    fn a_live_budget_acquires_and_holds_its_slot() {
        let clock = StepClock::new();
        let gate = EgressGate::new(1);
        let deadline = EgressDeadline::new(clock.now(), Duration::from_millis(50));

        let permit = acquire_within_deadline(&gate, deadline, || clock.now())
            .expect("a live budget acquires");
        assert_eq!(gate.in_use(), 1, "the permit holds its slot");
        assert!(
            deadline.remaining_at(clock.now()).is_some(),
            "budget remains for the write leg",
        );
        drop(permit);
        assert_eq!(gate.in_use(), 0);
    }

    /// A full gate refuses, and it refuses as `GateFull` rather than as
    /// a deadline miss — the two are different operational facts and a
    /// shared body must not merge the diagnosis.
    #[test]
    fn egress_gate_full_refuses_distinctly_from_a_deadline_miss() {
        let clock = StepClock::new();
        let gate = EgressGate::new(1);
        let deadline = EgressDeadline::new(clock.now(), Duration::from_millis(50));

        let held = acquire_within_deadline(&gate, deadline, || clock.now())
            .expect("the one slot is available");
        let refused = acquire_within_deadline(&gate, deadline, || clock.now())
            .expect_err("the second response finds the class full");

        assert_eq!(refused, EgressRefusal::GateFull);
        assert_eq!(refused.checkpoint(), "gate_full");
        assert_eq!(
            refused.body(),
            "overloaded\n",
            "both shed paths share the pinned overload body",
        );
        assert_eq!(gate.in_use(), 1, "the refusal did not disturb the holder");
        drop(held);
    }

    /// Checkpoint 3: serialization overran the budget, so the BUFFER IS
    /// DROPPED. The structural assertion is that the error arm hands
    /// back no bytes at all — the caller cannot transmit what it does
    /// not have.
    #[test]
    fn egress_deadline_rechecked_after_serialization_drops_buffer() {
        let clock = StepClock::new();
        let deadline = EgressDeadline::new(clock.now(), Duration::from_millis(10));
        let serialized = vec![b'x'; 4096];

        clock.advance(Duration::from_millis(11));
        let outcome = transmit_within_deadline(serialized, deadline, clock.now());

        let refused = outcome.expect_err("a buffer finished past the deadline is not transmitted");
        assert_eq!(refused, EgressRefusal::DeadlineAfterSerialization);
        assert_eq!(refused.checkpoint(), "post_serialization");

        // And a buffer finished IN time survives, so the checkpoint is
        // a deadline test and not an unconditional drop.
        let in_time = EgressDeadline::new(clock.now(), Duration::from_millis(10));
        let kept = transmit_within_deadline(vec![1_u8, 2, 3], in_time, clock.now())
            .expect("a live budget keeps its bytes");
        assert_eq!(kept, vec![1_u8, 2, 3], "the bytes pass through unchanged");
    }

    /// The deadline auctioneer attaches for the host is ABSOLUTE and
    /// anchored at request start, so the auction's own elapsed time is
    /// already spent from the write budget. A relative deadline would
    /// restart at egress and hand a slow response a second full
    /// allowance.
    #[test]
    fn egress_deadline_attached_to_response_for_upstream() {
        let clock = StepClock::new();
        let request_start = clock.now();
        let deadline = EgressDeadline::new(request_start, Duration::from_millis(100));

        // The auction spends 80ms of the request's own budget.
        clock.advance(Duration::from_millis(80));
        let remaining = deadline
            .remaining_at(clock.now())
            .expect("20ms of the budget is left");
        assert_eq!(
            remaining,
            Duration::from_millis(20),
            "the auction's elapsed time is spent from the write budget, not forgiven",
        );

        // The attached value resolves to the same absolute instant
        // whenever the host reads it, which is what makes the above
        // true on the host's side of the handoff.
        let attached = deadline.to_response_extension();
        assert_eq!(
            format!("{attached:?}"),
            format!("{:?}", deadline.to_response_extension()),
            "the attachment is a stable absolute value",
        );
        assert_eq!(
            deadline.deadline().instant(),
            request_start
                .checked_add(Duration::from_millis(100))
                .expect("no overflow"),
            "the absolute instant is request_start + the whole budget",
        );
    }

    /// With no envelope installed the fallback budget must be WIDE
    /// enough that a normal auction is not shed for a boot-state
    /// condition.
    ///
    /// This is a regression pin, not a preference. A short fallback
    /// looked safer and was wrong: it shed a real, completed auction
    /// as overload while no gate was bounding anything, because before
    /// the envelope is installed there is no egress capacity being
    /// contended for. A second-scale auction has to survive it.
    #[test]
    fn the_no_envelope_fallback_budget_does_not_shed_a_normal_auction() {
        let clock = StepClock::new();
        let deadline = EgressDeadline::from_boot_envelope(clock.now());
        // A full second of auction, an order of magnitude past any
        // shipped tmax row, and the budget must still have room.
        clock.advance(Duration::from_secs(1));
        assert!(
            !deadline.is_expired_at(clock.now()),
            "a one-second auction must not be shed when no envelope declared a write budget",
        );
        assert!(
            deadline.remaining_at(clock.now()).is_some(),
            "and the remaining budget must be readable rather than exhausted",
        );
    }

    /// Overflow fails closed: a budget that cannot be represented as
    /// `anchor + budget` yields a deadline AT the anchor, so the
    /// response is already expired rather than granted an unbounded
    /// write. The alternative — letting the addition wrap or saturate
    /// to the far future — would turn a bad budget into an unbounded
    /// one, which is the failure mode this whole module exists to
    /// prevent.
    #[test]
    fn an_unrepresentable_budget_fails_closed() {
        let anchor = MonotonicInstant::now();
        let deadline = EgressDeadline::new(anchor, Duration::MAX);
        assert_eq!(
            deadline.deadline().instant(),
            anchor,
            "overflow collapses the deadline onto the anchor",
        );
        assert!(
            deadline.is_expired_at(anchor),
            "a deadline at the anchor is expired at the anchor: fail closed",
        );
    }
}
