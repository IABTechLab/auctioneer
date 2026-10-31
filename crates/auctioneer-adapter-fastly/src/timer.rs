//! Fastly Compute `Timer`.
//!
//! `sleep` resolves immediately, whatever duration it is handed. That is
//! a statement about the platform: a Compute guest has no sleep it can
//! await between dispatching outbound requests and harvesting them, so
//! there is no host event for this timer to wait on. See
//! [`crate::delay`] for the full reasoning.
//!
//! # What this means for the wave gate
//!
//! auctioneer's scheduler races gate acquisition against the earliest
//! pending deadline. With this timer the deadline arm is ready on its
//! first poll, so the race is never PRODUCTIVE — it wins no permits and,
//! on its own, finds nothing due. The scheduler handles that by bounding
//! the re-race: after a fixed number of unproductive passes it awaits
//! the acquisition alone, which cannot be woken early by any timer
//! because none exists.
//!
//! The consequence, stated plainly rather than papered over: the
//! wave-gate wait on Fastly CAN overrun a slot deadline. A bare
//! acquisition await does not re-read the clock until a permit wakes it.
//! What auctioneer does guarantee is the half it owns — it re-checks the
//! clock on every permit wake and emits the positional timeouts that are
//! then due, so nothing late is ever published as on-time. It does NOT
//! claim the wait is deadline-bounded. `outbound-deadlines` is
//! `BestEffort` on this target for exactly this reason.
//!
//! What it is NOT is a busy spin. The unproductive-race bound is what
//! separates "degrades to waiting on permits" from "burns the isolate",
//! and an uncontended wave never reaches the bound at all: it wins its
//! permits on the first pass and dispatches normally.

use crate::delay::DelaySource;
use async_trait::async_trait;
use auctioneer_runtime::Timer;
use std::time::Duration;

/// `Timer` over a [`DelaySource`].
///
/// Generic over the source so the wave loop built on top of it is
/// reachable from a host test. The only source that ships is
/// [`crate::delay::ImmediateDelaySource`]; the parameter is not a place
/// to inject a real sleep Fastly does not have.
pub struct FastlyTimer<D> {
    source: D,
}

impl<D> FastlyTimer<D> {
    #[must_use]
    pub const fn new(source: D) -> Self {
        Self { source }
    }
}

#[async_trait(?Send)]
impl<D> Timer for FastlyTimer<D>
where
    D: DelaySource,
{
    async fn sleep(&self, duration: Duration) {
        self.source.delay(duration).await;
    }
}
