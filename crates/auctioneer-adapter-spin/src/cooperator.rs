//! Spin `DecodeCooperator`.
//!
//! # Why this is a bare yield and not a host sleep
//!
//! The Workers cooperator has to reach a real host event because a
//! Workers isolate freezes `Date.now()` for the duration of one
//! synchronous turn: CPU time spent inside that turn is invisible to the
//! clock, so auctioneer's deadline recheck would compare against the instant
//! the turn began and could never fire. Returning to the JS event loop
//! is what makes the next reading a new one.
//!
//! Spin has no such freeze. The clock behind
//! `MonotonicInstant` is WASI's `monotonic-clock.now`, a host call that
//! reads the current value every time it is made; nothing about a
//! guest's CPU work suspends it. So the recheck's freshness is already
//! guaranteed on this target, and awaiting a `sleep(0)` pollable to
//! obtain it would buy nothing while adding a host round trip — and a
//! host round trip inside every slot settle is a real cost on a
//! per-request-instantiated component.
//!
//! What the yield is still for: bounding poll monopolization. The wave's
//! observation loop polls the upstream batch and auctioneer's own slot
//! futures together, and a slot that never yields holds the task for
//! its whole duration, delaying the moment a sibling's completion is
//! OBSERVED. Returning `Pending` once hands control back to the executor
//! so the batch half gets a poll before the decode's recheck runs. That
//! is a latency property, not a correctness one — which is why it is
//! cheap enough to want and why the clock read after it is already
//! fresh.
//!
//! Yielding rather than installing [`auctioneer_runtime::NoopDecodeCooperator`]
//! is the deliberate choice: the no-op returns `Ready` on its first poll
//! and never gives the executor the chance, so it would leave that
//! bounding property unimplemented while looking, at the trait level,
//! identical.

use async_trait::async_trait;
use auctioneer_runtime::DecodeCooperator;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// `DecodeCooperator` that returns `Pending` exactly once, waking
/// immediately.
///
/// No host call, because Spin's clock needs no refreshing; see the
/// module docs.
pub struct SpinDecodeCooperator;

#[async_trait(?Send)]
impl DecodeCooperator for SpinDecodeCooperator {
    async fn yield_to_host(&self) {
        YieldOnce { yielded: false }.await;
    }
}

/// A future that is `Pending` on its first poll and `Ready` on its
/// second.
///
/// Written out rather than pulled from a runtime's `yield_now` because
/// this adapter has no async runtime: Spin drives the component's future
/// itself, and the executor that polls auctioneer's decode futures is the
/// wave scheduler's own `FuturesUnordered`. Waking the task before
/// returning `Pending` is what makes this a yield rather than a stall —
/// without it nothing would ever poll the future again.
struct YieldOnce {
    yielded: bool,
}

impl Future for YieldOnce {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.yielded {
            return Poll::Ready(());
        }
        self.yielded = true;
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::YieldOnce;
    use futures::executor::block_on;
    use futures::future::FutureExt as _;

    /// The future is `Pending` on its first poll and `Ready` on its
    /// second, so awaiting it hands control back to the executor exactly
    /// once.
    ///
    /// Asserted at the poll level rather than through `block_on` alone:
    /// a future that returned `Ready` on the first poll would also
    /// complete under `block_on`, and completing is not the property
    /// being claimed.
    #[test]
    fn yield_once_is_pending_then_ready() {
        block_on(async {
            let mut future = Box::pin(YieldOnce { yielded: false });
            assert!(
                futures::poll!(future.as_mut()).is_pending(),
                "the first poll returns to the executor",
            );
            assert!(
                futures::poll!(future.as_mut()).is_ready(),
                "and the second resumes; a yield is one round trip, not a stall",
            );
        });
    }

    /// The `Pending` poll wakes the task, so the future is re-polled
    /// without anything else driving it.
    ///
    /// A `Pending` that forgot to wake would be a deadlock, not a yield:
    /// nothing in the wave scheduler holds a waker for a decode future
    /// on its behalf.
    #[test]
    fn yield_once_completes_without_an_external_waker() {
        let _: () = block_on(YieldOnce { yielded: false });
    }

    /// The trait method is `Future::poll`-shaped for the same reason,
    /// once behind the async-trait boxing.
    #[test]
    fn cooperator_yield_returns_to_the_executor_once() {
        use auctioneer_runtime::DecodeCooperator as _;
        block_on(async {
            let mut yielding = super::SpinDecodeCooperator.yield_to_host().boxed_local();
            assert!(
                futures::poll!(yielding.as_mut()).is_pending(),
                "the cooperator's yield reaches the executor",
            );
            assert!(
                futures::poll!(yielding.as_mut()).is_ready(),
                "and resumes on the next poll",
            );
        });
    }
}
