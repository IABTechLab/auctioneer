//! Egress-class concurrency gate and the body that carries a permit.
//!
//! [`EgressGate`] bounds how many responses auctioneer may have in flight
//! on the auction egress class. Its shape is deliberately identical to
//! [`AdmissionGate`](crate::admission::AdmissionGate): one atomic
//! counter, an immediate non-queueing
//! [`try_acquire`](EgressGate::try_acquire), and an owned RAII permit
//! that releases exactly one slot on drop. Capacity comes from the
//! baked boot envelope's `egress_permits`.
//!
//! # Two different permits
//!
//! The normative egress contract wants a permit that survives until the
//! host reports terminal completion or disconnect for *its own*
//! response. Body EOF is NOT that signal: a body can be fully read and
//! dropped before a single byte reaches the client. This module
//! therefore builds two distinct things and is careful about which
//! guarantee each one carries:
//!
//! - [`PermitBody`] is the ADMISSION-BODY permit carrier. It wraps a
//!   body stream and releases its permit on EOF or on drop, whichever
//!   comes first. That bounds CONCURRENT LIVE BODIES, which is exactly
//!   what EOF and drop measure, and it is the whole of its claim.
//! - The RESPONSE-EGRESS binding — holding the permit until the host's
//!   terminal report for this response — needs a per-response identity
//!   on that report, which the host does not expose at the revision
//!   this module is built against. [`EgressGate`] and [`EgressPermit`]
//!   are the durable half of it: the permit is an OWNED value with no
//!   lifetime parameter, so it can be moved into whatever per-response
//!   state the binding eventually lives in. Wiring the release to a
//!   terminal report is a separate change and is NOT done here.
//!
//! Releasing at handler-future completion would be actively wrong: the
//! host router drops its per-request grant BEFORE it constructs a
//! fallback response, so a permit released there strands capacity while
//! the response is still unwritten. Nothing in this module releases on
//! handler return.

use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::task::{Context, Poll};

use futures::stream::Stream;

/// Bounded egress-class gate with immediate, non-blocking acquisition.
///
/// Holds one in-use counter against a fixed capacity.
/// [`try_acquire`](EgressGate::try_acquire) claims a slot with a single
/// compare-and-swap loop and never waits; when every slot is taken it
/// returns `None`, so the caller decides the response rather than
/// blocking behind a queue. A gate built with capacity `0` admits
/// nothing.
#[derive(Debug)]
pub struct EgressGate {
    in_use: AtomicU32,
    capacity: u32,
}

impl EgressGate {
    /// Builds a gate admitting at most `capacity` concurrent holders.
    #[must_use]
    pub fn new(capacity: u32) -> Arc<Self> {
        Arc::new(EgressGate {
            in_use: AtomicU32::new(0),
            capacity,
        })
    }

    /// Claims a slot, or returns `None` when the gate is full.
    ///
    /// Allocation-free on both paths: a CAS loop that retries only when
    /// another thread changed the counter concurrently, and an
    /// `Arc::clone` that bumps a refcount rather than allocating.
    #[must_use]
    pub fn try_acquire(self: &Arc<Self>) -> Option<EgressPermit> {
        self.in_use
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                // `current` never exceeds `capacity`, so the successor
                // cannot overflow a `u32` — but stay saturating rather
                // than trust that invariant with arithmetic.
                (current < self.capacity).then(|| current.saturating_add(1))
            })
            .ok()?;
        Some(EgressPermit {
            gate: Arc::clone(self),
        })
    }

    /// The gate's fixed total capacity.
    #[must_use]
    pub const fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Slots currently leased out. Exposed for tests and diagnostics.
    #[must_use]
    pub fn in_use(&self) -> u32 {
        self.in_use.load(Ordering::Acquire)
    }
}

/// RAII lease on one [`EgressGate`] slot.
///
/// Dropping the permit — normally or while a panic unwinds — releases
/// exactly one slot. The permit keeps its gate alive through an OWNED
/// handle rather than a borrow, so it can be moved into per-response
/// state that outlives the frame that acquired it; that is what lets a
/// future change bind release to a terminal egress report instead of to
/// a stack frame. It is not `Clone`, so a slot cannot be released
/// twice.
#[derive(Debug)]
pub struct EgressPermit {
    gate: Arc<EgressGate>,
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for EgressPermit {
    fn drop(&mut self) {
        // One permit == one successful increment, so this never
        // over-decrements.
        self.gate.in_use.fetch_sub(1, Ordering::AcqRel);
    }
}

/// A body stream that carries a permit and releases it exactly once, at
/// stream EOF or on drop — whichever happens first.
///
/// This is the ADMISSION-BODY carrier. It bounds concurrent live
/// bodies, and that is the only claim it makes: EOF is not a terminal
/// egress signal, so a `PermitBody` does NOT establish that the
/// response reached the client. See the module docs.
///
/// `P` is the permit type so the same carrier serves the egress and aux
/// classes without either gate knowing about the other.
#[derive(Debug)]
pub struct PermitBody<B, P> {
    body: B,
    /// `None` once released. Held in an `Option` rather than tracked
    /// with a separate `bool` so "released" and "still owns a permit"
    /// cannot disagree: releasing IS taking the permit out and dropping
    /// it, and a second release finds nothing to take.
    permit: Option<P>,
}

impl<B, P> PermitBody<B, P> {
    /// Wraps `body`, holding `permit` until EOF or drop.
    pub const fn new(body: B, permit: P) -> Self {
        PermitBody {
            body,
            permit: Some(permit),
        }
    }

    /// Whether this body still holds its permit.
    ///
    /// `false` after EOF released it. Exposed so a caller can assert
    /// the release happened rather than infer it from gate counters.
    pub const fn holds_permit(&self) -> bool {
        self.permit.is_some()
    }

    /// Drops the permit if it is still held. Idempotent.
    fn release(&mut self) {
        drop(self.permit.take());
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "retain the conservative default size_hint without changing stream behavior"
)]
impl<B, P, T, E> Stream for PermitBody<B, P>
where
    B: Stream<Item = Result<T, E>> + Unpin,
    P: Unpin,
{
    type Item = Result<T, E>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // `Unpin` on both fields is what makes this projection sound
        // without `unsafe`: `get_mut` hands back a plain `&mut Self`.
        let this = self.get_mut();
        let polled = Pin::new(&mut this.body).poll_next(cx);
        if matches!(polled, Poll::Ready(None)) {
            // EOF. Release here rather than waiting for the drop so a
            // body whose consumer keeps the (now exhausted) stream
            // alive does not pin a slot.
            this.release();
        }
        polled
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl<B, P> Drop for PermitBody<B, P> {
    fn drop(&mut self) {
        // A body abandoned mid-stream never reaches the EOF branch, so
        // drop is the backstop. `release` is idempotent, so a body that
        // already hit EOF releases nothing a second time.
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::{EgressGate, PermitBody};
    use futures::stream::{self, Stream};
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::task::{Context, Poll, Waker};

    use crate::alloc_track;

    /// Drives a stream to the next ready item on the current thread.
    /// The streams under test never return `Pending`, so a bare poll is
    /// a complete drive step and no executor is needed.
    fn poll_next<S>(stream: &mut S) -> Poll<Option<S::Item>>
    where
        S: Stream + Unpin,
    {
        let mut cx = Context::from_waker(Waker::noop());
        Pin::new(stream).poll_next(&mut cx)
    }

    /// An owned permit whose drop is observable without a gate, so the
    /// release-count assertions measure drops directly rather than
    /// inferring them from a counter that a double release could wrap
    /// back into place.
    #[derive(Debug)]
    struct CountingPermit {
        drops: Arc<AtomicU32>,
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for CountingPermit {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::AcqRel);
        }
    }

    /// The admission-body permit's whole contract: it releases at
    /// stream EOF, and it releases on drop when the body is abandoned
    /// before EOF. Named for the ADMISSION-BODY permit, not the egress
    /// permit, because EOF is not a terminal-egress signal and this
    /// test must not be read as evidence for that stronger claim.
    #[test]
    fn admission_body_permit_releases_on_eof_and_on_drop() {
        // EOF path: reading the body to completion releases exactly
        // once, and the subsequent drop does not release again.
        let drops = Arc::new(AtomicU32::new(0));
        let permit = CountingPermit {
            drops: Arc::clone(&drops),
        };
        let mut body = PermitBody::new(stream::iter(vec![Ok::<u8, ()>(1), Ok(2)]), permit);

        assert!(body.holds_permit(), "a fresh body holds its permit");
        assert!(matches!(poll_next(&mut body), Poll::Ready(Some(Ok(1)))));
        assert_eq!(
            drops.load(Ordering::Acquire),
            0,
            "a mid-stream chunk must not release the permit",
        );
        assert!(matches!(poll_next(&mut body), Poll::Ready(Some(Ok(2)))));
        assert_eq!(drops.load(Ordering::Acquire), 0, "still mid-stream");

        assert!(matches!(poll_next(&mut body), Poll::Ready(None)), "EOF");
        assert_eq!(
            drops.load(Ordering::Acquire),
            1,
            "EOF releases the permit exactly once",
        );
        assert!(!body.holds_permit(), "the permit is gone after EOF");

        drop(body);
        assert_eq!(
            drops.load(Ordering::Acquire),
            1,
            "dropping an already-released body must not release twice",
        );

        // Drop path: a body abandoned mid-stream still releases.
        let abandoned_drops = Arc::new(AtomicU32::new(0));
        let abandoned_permit = CountingPermit {
            drops: Arc::clone(&abandoned_drops),
        };
        let mut abandoned =
            PermitBody::new(stream::iter(vec![Ok::<u8, ()>(1), Ok(2)]), abandoned_permit);
        assert!(matches!(
            poll_next(&mut abandoned),
            Poll::Ready(Some(Ok(1)))
        ));
        assert_eq!(
            abandoned_drops.load(Ordering::Acquire),
            0,
            "one chunk read is not EOF",
        );
        drop(abandoned);
        assert_eq!(
            abandoned_drops.load(Ordering::Acquire),
            1,
            "abandoning a body mid-stream releases its permit",
        );
    }

    /// Against a REAL gate, a body returns its slot so the next
    /// response can acquire one. This is the counter-level counterpart
    /// of the drop-count test above.
    #[test]
    fn admission_body_returns_its_slot_to_the_gate() {
        let gate = EgressGate::new(1);
        let permit = gate.try_acquire().expect("capacity 1 admits the first");
        assert_eq!(gate.in_use(), 1);
        assert!(
            gate.try_acquire().is_none(),
            "the one slot is taken while the body is live",
        );

        let mut body = PermitBody::new(stream::iter(vec![Ok::<u8, ()>(7)]), permit);
        assert!(matches!(poll_next(&mut body), Poll::Ready(Some(Ok(7)))));
        assert_eq!(gate.in_use(), 1, "the slot is held until EOF");
        assert!(matches!(poll_next(&mut body), Poll::Ready(None)));
        assert_eq!(gate.in_use(), 0, "EOF returned the slot");

        let reacquired = gate.try_acquire().expect("the slot is reusable");
        assert_eq!(gate.in_use(), 1);
        drop(reacquired);
        assert_eq!(gate.in_use(), 0);
    }

    /// Pure gate construction: the (capacity + 1)-th `try_acquire`
    /// returns `None` and allocates nothing. This asserts the GATE's
    /// decision, not the response that decision produces — the
    /// response-integration half needs a terminal-completion API this
    /// revision does not have.
    #[test]
    fn egress_gate_full_refuses_without_a_permit() {
        const CAPACITY: u32 = 3;
        let gate = EgressGate::new(CAPACITY);
        assert_eq!(gate.capacity(), CAPACITY);

        let held: Vec<_> = (0..CAPACITY)
            .map(|slot| {
                gate.try_acquire()
                    .unwrap_or_else(|| panic!("capacity {CAPACITY} admits slot {slot}"))
            })
            .collect();
        assert_eq!(gate.in_use(), CAPACITY, "every slot is leased");

        // The allocator is process-global, so measure per-thread and
        // across only the rejected call.
        let before = alloc_track::allocated_on_this_thread();
        let rejected = gate.try_acquire();
        let allocated = alloc_track::allocated_on_this_thread().saturating_sub(before);

        assert!(
            rejected.is_none(),
            "the capacity+1-th acquisition must be refused",
        );
        assert_eq!(
            allocated, 0,
            "refusal must allocate nothing: it is the shed path under load",
        );
        assert_eq!(
            gate.in_use(),
            CAPACITY,
            "a refused acquisition must not disturb the counter",
        );

        drop(held);
        assert_eq!(gate.in_use(), 0, "releasing every permit empties the gate");
        assert!(
            gate.try_acquire().is_some(),
            "the gate admits again once drained",
        );
    }

    /// A zero-capacity gate admits nothing rather than admitting one.
    #[test]
    fn zero_capacity_egress_gate_admits_nothing() {
        let gate = EgressGate::new(0);
        assert!(gate.try_acquire().is_none());
        assert_eq!(gate.in_use(), 0);
    }
}
