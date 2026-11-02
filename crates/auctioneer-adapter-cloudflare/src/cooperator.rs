//! Cloudflare Workers `DecodeCooperator`.
//!
//! A Workers isolate freezes `Date.now()` for the duration of one
//! synchronous turn, so CPU time spent inside that turn is invisible to
//! the clock. Lifting a slot and settling the encoding it arrived in is
//! exactly such work, and the deadline recheck auctioneer owes runs right
//! after it. Without a yield the recheck compares the deadline against
//! the instant the turn began and can never fire, however long the lift
//! took.
//!
//! The yield therefore has to be a real host event. Awaiting an
//! already-resolved promise, or a bare `yield_now()`, re-polls inside
//! the same turn and refreshes nothing — the clock would still be
//! frozen on the other side. `setTimeout(0)` returns control to the JS
//! event loop, which is what makes the next `Date.now()` a new reading.

use crate::delay::DelaySource;
use async_trait::async_trait;
use auctioneer_runtime::DecodeCooperator;
use std::time::Duration;

/// `DecodeCooperator` that yields through a [`DelaySource`] with a zero
/// duration.
///
/// Zero is the point: the caller is asking for one turn of the event
/// loop, not for a wait. Anything longer would charge the slot's
/// deadline for time it did not need to spend.
pub struct CloudflareDecodeCooperator<D> {
    source: D,
}

impl<D> CloudflareDecodeCooperator<D> {
    #[must_use]
    pub const fn new(source: D) -> Self {
        Self { source }
    }
}

#[async_trait(?Send)]
impl<D> DecodeCooperator for CloudflareDecodeCooperator<D>
where
    D: DelaySource,
{
    async fn yield_to_host(&self) {
        self.source.delay(Duration::ZERO).await;
    }
}
