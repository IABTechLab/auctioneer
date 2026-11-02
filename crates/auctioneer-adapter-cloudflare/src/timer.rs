//! Cloudflare Workers `Timer`.
//!
//! Replaces the immediate-resolve stub this adapter installed while its
//! fanout never awaited a timer. The wave scheduler does await one: it
//! races gate acquisition against the earliest pending deadline, and a
//! sleep that resolves on its first poll makes that race unproductive
//! every time — the scheduler then falls back to awaiting the
//! acquisition alone after a bounded number of passes, which is exactly
//! the overrun the profile declares as `BestEffort`.
//!
//! Awaiting a real `setTimeout` also makes the boundary a
//! CLOCK-REFRESHING one. A Workers isolate freezes `Date.now()` for the
//! duration of a synchronous turn, so the instant read after this sleep
//! is a new one rather than the instant the turn began — which is what
//! the scheduler's post-acquisition recheck needs in order to see that a
//! deadline passed while it waited.

use crate::delay::DelaySource;
use async_trait::async_trait;
use auctioneer_runtime::Timer;
use std::time::Duration;

/// `Timer` over a [`DelaySource`].
///
/// Generic rather than binding `worker::Delay` directly so the sleep
/// logic is reachable from a host test; see [`crate::delay`] for what
/// that does and does not prove.
pub struct CloudflareTimer<D> {
    source: D,
}

impl<D> CloudflareTimer<D> {
    #[must_use]
    pub const fn new(source: D) -> Self {
        Self { source }
    }
}

#[async_trait(?Send)]
impl<D> Timer for CloudflareTimer<D>
where
    D: DelaySource,
{
    async fn sleep(&self, duration: Duration) {
        self.source.delay(duration).await;
    }
}
