//! `DelaySource` — the seam between this adapter's timing primitives and
//! whatever the host can actually be asked to wait on.
//!
//! On Fastly the answer is NOTHING, and that is the point of the module
//! rather than an omission. Compute gives a guest no sleep primitive it
//! can await between dispatching a batch and harvesting it: there is no
//! `setTimeout` equivalent reachable from the guest, and the pending
//! request handles expose only a blocking `select`, which is not a
//! future a race can hold. So the PRODUCTION source below resolves on
//! its first poll.
//!
//! Why keep a seam at all, given one implementation ships: the seam is
//! for the surrounding loop, not for the sleep. auctioneer's wave scheduler
//! races gate acquisition against the earliest pending deadline, and the
//! behaviour worth pinning is what that loop does when the sleep arm is
//! always ready — that it re-reads the clock on every permit wake, emits
//! only the timeouts that are genuinely due, and gives up racing after a
//! bounded number of unproductive passes instead of spinning the
//! isolate. A test needs to observe the loop, not substitute a different
//! host.
//!
//! What a host test therefore MUST NOT do is install a delaying source
//! here. A source that actually waited would simulate deadline
//! behaviour Fastly cannot deliver, and every row built on it would
//! assert a guarantee that does not ship. The rows in
//! `tests/fanout.rs` drive [`ImmediateDelaySource`] — the production
//! implementation — and observe the loop around it.

use async_trait::async_trait;
use std::time::Duration;

/// A host primitive a timing seam may await.
///
/// `?Send` for the same reason [`auctioneer_runtime::Timer`] is: the fanout
/// futures are driven on single-threaded edge runtimes.
#[async_trait(?Send)]
pub trait DelaySource: Send + Sync {
    /// Yield to the host for `duration`, insofar as the host offers any
    /// way to do so.
    ///
    /// An implementation that cannot wait must still return, and the
    /// caller must not assume `duration` elapsed. Fastly's does exactly
    /// that; see the module docs for why no alternative exists.
    async fn delay(&self, duration: Duration);
}

/// The production source: it resolves immediately, because Compute has
/// no guest-awaitable sleep.
///
/// This is not a stub standing in for a real one that lands later. It is
/// what Fastly can do, and the `BestEffort` cells in
/// [`crate::limits::OUTBOUND_PROFILE`] are the declaration of the gap it
/// leaves.
pub struct ImmediateDelaySource;

#[async_trait(?Send)]
impl DelaySource for ImmediateDelaySource {
    async fn delay(&self, _duration: Duration) {}
}
