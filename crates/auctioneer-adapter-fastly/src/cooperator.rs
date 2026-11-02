//! Fastly Compute `DecodeCooperator`.
//!
//! A plain `yield_now()`, which is correct here for a reason Cloudflare's
//! is not: the Compute clock is NOT frozen across guest CPU work.
//! `std::time::Instant` on `wasm32-wasip1` reads a host clock that keeps
//! advancing while the guest computes, so the deadline recheck auctioneer
//! runs after its own slot settle already sees a fresh instant without
//! any host round trip.
//!
//! That makes the yield's job a different one. It is not there to
//! refresh a frozen clock — nothing here is frozen — it is there so a
//! slot returns to the executor once and lets the batch driver observe
//! siblings that became terminal meanwhile, rather than holding the
//! single-threaded runtime for the whole settle.
//!
//! Which is why this is NOT
//! [`auctioneer_runtime::NoopDecodeCooperator`], despite the clock argument
//! sounding like the case for it. `NoopDecodeCooperator` returns without
//! reaching the executor at all, so a slot running under it never yields
//! a poll to the sibling slots waiting behind it. `yield_now()` costs one
//! wake and buys that interleaving.

use auctioneer_runtime::DecodeCooperator;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;

/// A future that returns `Pending` exactly once, waking itself, then
/// resolves.
///
/// Written out rather than pulled from a dependency because
/// `futures::pending!`/`yield_now` would put `futures` in this crate's
/// non-test graph for one nine-line future, and the semantics worth
/// being explicit about are exactly these: wake before returning
/// `Pending`, so the executor re-polls promptly instead of parking the
/// task.
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
        // Wake before yielding: the task is ready to continue and is
        // giving up the poll slot, not waiting for anything.
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

/// `DecodeCooperator` that returns one poll to the executor.
pub struct FastlyDecodeCooperator;

#[async_trait(?Send)]
impl DecodeCooperator for FastlyDecodeCooperator {
    async fn yield_to_host(&self) {
        YieldOnce { yielded: false }.await;
    }
}
