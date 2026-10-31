//! The static-egress class: one permit per terminal response.
//!
//! Every response auctioneer emits — an auction result, a refusal, an aux
//! route's answer, a 500 — occupies a transmit buffer and some
//! per-response state while it is being written. [`StaticEgressGate`]
//! bounds how many of those can exist at once, drawn from the boot
//! envelope's `terminal_response_permits`, with each permit standing
//! for one `refusal_body_bytes` render buffer.
//!
//! # Why this is a class of its own
//!
//! The admission, egress and aux gates each bound a ROUTE's work. This
//! one bounds the TRANSMISSION, which every route shares and which some
//! responses reach without ever passing a route gate: a pre-admission
//! 414 is rendered before application admission runs, and an admission
//! refusal is a response produced by the very decision that declined to
//! admit. Those responses still occupy a buffer, so they still need a
//! permit, and no route gate is in a position to give them one.
//!
//! # Why exhaustion means abort
//!
//! A response with no static permit has nowhere to render into. The
//! alternatives are to emit it anyway — which is exactly the unbounded
//! transmission this class exists to prevent — or to refuse the request
//! at the transport level. auctioneer refuses: a reset connection is a
//! bounded, recoverable answer to overload, and an ungated response is
//! not.
//!
//! The permit shape is deliberately the same as the other three gates:
//! one atomic counter, an immediate non-queueing try-acquire, and an
//! OWNED RAII permit. Owned rather than borrowing, because a permit has
//! to be movable into a response completion that outlives the frame
//! that acquired it.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// Bounded static-response class with immediate, non-blocking
/// acquisition.
///
/// A gate built with capacity `0` admits nothing.
#[derive(Debug)]
pub struct StaticEgressGate {
    in_use: AtomicU32,
    capacity: u32,
}

impl StaticEgressGate {
    /// Builds a gate admitting at most `capacity` concurrent terminal
    /// responses.
    #[must_use]
    pub fn new(capacity: u32) -> Arc<Self> {
        Arc::new(StaticEgressGate {
            in_use: AtomicU32::new(0),
            capacity,
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

    /// Claims a slot, or returns `None` when the class is full.
    ///
    /// Allocation-free on both paths, which matters here more than on
    /// the route gates: this is the path a saturated isolate takes for
    /// every single response, and an allocation on the shed path is an
    /// allocation exactly when there is no memory to spare.
    #[must_use]
    pub fn try_acquire(self: &Arc<Self>) -> Option<StaticEgressPermit> {
        self.in_use
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                // `current` never exceeds `capacity`, so the successor
                // cannot overflow a `u32` — but stay saturating rather
                // than trust that invariant with arithmetic.
                (current < self.capacity).then(|| current.saturating_add(1))
            })
            .ok()?;
        Some(StaticEgressPermit {
            gate: Arc::clone(self),
        })
    }
}

/// RAII lease on one [`StaticEgressGate`] slot.
///
/// Dropping it — normally or while a panic unwinds — releases exactly
/// one slot. Not `Clone`, so a slot cannot be released twice. Owned
/// rather than borrowing, so it can be moved into the response
/// completion that holds it until terminal egress.
#[derive(Debug)]
pub struct StaticEgressPermit {
    gate: Arc<StaticEgressGate>,
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for StaticEgressPermit {
    fn drop(&mut self) {
        // One permit == one successful increment, so this never
        // over-decrements.
        self.gate.in_use.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::StaticEgressGate;
    use std::panic::catch_unwind;
    use std::sync::Arc;

    /// The class bounds concurrent terminal responses and refuses the
    /// one past capacity, which is what makes "every response holds a
    /// permit" a real bound rather than a slogan.
    #[test]
    fn the_static_class_refuses_past_capacity() {
        const CAPACITY: u32 = 2;
        let gate = StaticEgressGate::new(CAPACITY);
        assert_eq!(gate.capacity(), CAPACITY);

        let first = gate.try_acquire().expect("slot one");
        let second = gate.try_acquire().expect("slot two");
        assert_eq!(gate.in_use(), CAPACITY);
        assert!(
            gate.try_acquire().is_none(),
            "an exhausted static class refuses rather than over-committing a buffer",
        );

        drop(first);
        assert_eq!(gate.in_use(), 1);
        assert!(
            gate.try_acquire().is_some(),
            "a released buffer is immediately reusable",
        );
        drop(second);
    }

    /// A zero-capacity gate admits nothing rather than admitting one.
    #[test]
    fn zero_capacity_admits_nothing() {
        let gate = StaticEgressGate::new(0);
        assert!(gate.try_acquire().is_none());
        assert_eq!(gate.in_use(), 0);
    }

    /// A permit released while a panic unwinds still returns its slot,
    /// so a panicking response path cannot leak transmit capacity.
    #[test]
    fn a_permit_released_during_unwind_returns_its_slot() {
        let gate = StaticEgressGate::new(1);
        let inner = Arc::clone(&gate);
        let unwound = catch_unwind(move || {
            let _permit = inner.try_acquire().expect("capacity 1 admits");
            panic!("the response path failed mid-write");
        });
        assert!(unwound.is_err(), "the panic was caught");
        assert_eq!(
            gate.in_use(),
            0,
            "unwinding released the permit: a panicking write leaks no capacity",
        );
    }
}
