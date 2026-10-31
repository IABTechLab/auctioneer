//! Spin `Timer`.
//!
//! Replaces the immediate-resolve stub this adapter installed while its
//! fanout never awaited a timer. The wave scheduler does await one: it
//! races gate acquisition against the earliest pending deadline, and a
//! sleep that resolves on its first poll makes that race unproductive
//! every time — the scheduler then falls back to awaiting the
//! acquisition alone after a bounded number of passes, which is exactly
//! the overrun the profile declares as `BestEffort`.
//!
//! The wait itself is the WASI monotonic-clock pollable, reached through
//! `spin_sdk::time::sleep`. Unlike the Workers case there is no frozen
//! clock to refresh here: WASI's monotonic clock advances across guest
//! CPU work, so the point of awaiting a real sleep is only that the race
//! resolves when the deadline actually arrives rather than immediately.

use crate::delay::DelaySource;
use async_trait::async_trait;
use auctioneer_runtime::Timer;
use std::time::Duration;

/// `Timer` over a [`DelaySource`].
///
/// Generic rather than binding `spin_sdk::time::sleep` directly so the
/// sleep logic is reachable from a host test; see [`crate::delay`] for
/// what that does and does not prove.
pub struct SpinTimer<D> {
    source: D,
}

impl<D> SpinTimer<D> {
    #[must_use]
    pub const fn new(source: D) -> Self {
        Self { source }
    }
}

#[async_trait(?Send)]
impl<D> Timer for SpinTimer<D>
where
    D: DelaySource,
{
    async fn sleep(&self, duration: Duration) {
        self.source.delay(duration).await;
    }
}
