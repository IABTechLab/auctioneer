//! The aux-route admission class.
//!
//! Aux routes — status, bidder info, the GVL health probe — are cheap
//! but unauthenticated and easy to flood. [`AuxGate`] gives them their
//! OWN capacity, drawn from the boot envelope's `aux_route_permits`, so
//! saturating the aux class cannot take a single slot away from auction
//! admission. That separation is the whole reason the type exists: one
//! shared gate with a large capacity would let an aux flood starve
//! auctions, and one shared gate with a small capacity would let
//! auctions starve the health probe an operator needs precisely when
//! the isolate is saturated.
//!
//! The acquisition shape is deliberately the same as
//! [`AdmissionGate`](crate::admission::AdmissionGate) and
//! [`EgressGate`](crate::egress::EgressGate): one atomic counter, an
//! immediate non-queueing try-acquire, and an owned RAII permit. Three
//! gates with one shape means a reader learns it once.
//!
//! [`AuxPermit`] is OWNED — it holds a handle on its gate rather than
//! borrowing one — so it can be moved into per-response state that
//! outlives the frame that acquired it. A permit must be acquired at
//! most ONCE per request: a route that is already admitted and then
//! tries to acquire a second permit from a capacity-1 gate would block
//! on a slot it is itself holding, which is a request rejecting itself.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// Bounded aux-route gate with immediate, non-blocking admission.
///
/// A gate built with capacity `0` admits nothing.
#[derive(Debug)]
pub struct AuxGate {
    in_use: AtomicU32,
    capacity: u32,
}

impl AuxGate {
    /// Builds a gate admitting at most `capacity` concurrent aux
    /// routes.
    #[must_use]
    pub fn new(capacity: u32) -> Arc<Self> {
        Arc::new(AuxGate {
            in_use: AtomicU32::new(0),
            capacity,
        })
    }

    /// Claims a slot, or returns `None` when the aux class is full.
    ///
    /// Allocation-free on both paths.
    #[must_use]
    pub fn try_acquire(self: &Arc<Self>) -> Option<AuxPermit> {
        self.in_use
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                // `current` never exceeds `capacity`, so the successor
                // cannot overflow a `u32` — but stay saturating rather
                // than trust that invariant with arithmetic.
                (current < self.capacity).then(|| current.saturating_add(1))
            })
            .ok()?;
        Some(AuxPermit {
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

/// RAII lease on one [`AuxGate`] slot.
///
/// Dropping it — normally or while a panic unwinds — releases exactly
/// one slot. Not `Clone`, so a slot cannot be released twice.
#[derive(Debug)]
pub struct AuxPermit {
    gate: Arc<AuxGate>,
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for AuxPermit {
    fn drop(&mut self) {
        // One permit == one successful increment, so this never
        // over-decrements.
        self.gate.in_use.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::AuxGate;
    use crate::admission::AdmissionGate;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    /// The separation that justifies a second gate: exhausting the aux
    /// class leaves auction admission untouched, and exhausting auction
    /// admission leaves the aux class untouched. A single shared gate
    /// would fail this in both directions.
    #[test]
    fn aux_flood_cannot_consume_auction_admission() {
        const AUX_CAPACITY: u32 = 2;
        const AUCTION_CAPACITY: u32 = 3;

        let aux = AuxGate::new(AUX_CAPACITY);
        let auction = AdmissionGate::new(AUCTION_CAPACITY);

        // Flood the aux class past its capacity.
        let flood: Vec<_> = (0..AUX_CAPACITY)
            .map(|slot| {
                aux.try_acquire()
                    .unwrap_or_else(|| panic!("aux capacity {AUX_CAPACITY} admits slot {slot}"))
            })
            .collect();
        assert_eq!(aux.in_use(), AUX_CAPACITY, "the aux class is saturated");
        assert!(
            aux.try_acquire().is_none(),
            "a saturated aux class refuses further aux routes",
        );

        // The flood consumed NO auction capacity.
        assert_eq!(
            auction.in_use(),
            0,
            "an aux flood must not touch the auction counter",
        );
        let auctions: Vec<_> = (0..AUCTION_CAPACITY)
            .map(|slot| {
                auction.try_acquire().unwrap_or_else(|| {
                    panic!("auction slot {slot} must be free during an aux flood")
                })
            })
            .collect();
        assert_eq!(
            auction.in_use(),
            AUCTION_CAPACITY,
            "every auction slot was still available",
        );

        // And the isolation holds in the other direction: with auction
        // admission now saturated, draining the aux flood frees aux
        // slots without needing an auction slot.
        assert!(
            auction.try_acquire().is_none(),
            "auction admission is saturated in its own right",
        );
        drop(flood);
        assert_eq!(aux.in_use(), 0, "the aux class drained");
        assert!(
            aux.try_acquire().is_some(),
            "an aux route is admitted while auction admission is full",
        );
        assert_eq!(
            auction.in_use(),
            AUCTION_CAPACITY,
            "admitting an aux route consumed no auction capacity",
        );
        drop(auctions);
    }

    /// One permit per request, and the slot comes back on drop —
    /// including while a panic unwinds, so a panicking aux handler does
    /// not strand its class.
    #[test]
    fn aux_permit_releases_on_drop_including_panic() {
        let gate = AuxGate::new(1);
        assert_eq!(gate.capacity(), 1);

        let permit = gate.try_acquire().expect("capacity 1 admits the first");
        assert_eq!(gate.in_use(), 1);
        drop(permit);
        assert_eq!(gate.in_use(), 0, "normal drop releases the slot");

        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let _permit = gate.try_acquire().expect("gate is empty again");
            assert_eq!(gate.in_use(), 1);
            panic!("unwind with a live aux permit");
        }));
        assert!(outcome.is_err(), "the closure is expected to panic");
        assert_eq!(gate.in_use(), 0, "unwinding drop releases the slot");

        // Release happened exactly once — a double release would have
        // wrapped the counter and this acquisition would have failed.
        let reacquired = gate.try_acquire().expect("the slot is reusable");
        assert!(gate.try_acquire().is_none(), "capacity 1 is now full");
        drop(reacquired);
        assert_eq!(gate.in_use(), 0);
    }

    /// A zero-capacity aux gate admits nothing rather than admitting
    /// one.
    #[test]
    fn zero_capacity_aux_gate_admits_nothing() {
        let gate = AuxGate::new(0);
        assert!(gate.try_acquire().is_none());
        assert_eq!(gate.in_use(), 0);
    }
}
