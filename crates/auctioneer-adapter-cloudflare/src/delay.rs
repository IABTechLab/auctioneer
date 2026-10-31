//! `DelaySource` — the seam between this adapter's timing primitives
//! and the Workers `setTimeout` future that backs them.
//!
//! The trait exists because `worker::Delay` is a `wasm32`-only type: the
//! `worker` crate sits under `[target.'cfg(target_arch = "wasm32")'
//! .dependencies]`, so on the host it does not exist at all, and this
//! repository has no wasm runner. Without the seam neither
//! [`CloudflareTimer`](crate::timer::CloudflareTimer) nor
//! [`CloudflareDecodeCooperator`](crate::cooperator::CloudflareDecodeCooperator)
//! could be exercised anywhere: not on the host, for want of the type,
//! and not on wasm, for want of anything to run the test.
//!
//! What that buys and what it does not: a host test proves the LOGIC
//! built on top of a source — that a sleep awaits it for the duration
//! asked for, that a yield awaits it once, and that the wave race
//! observes both. It does NOT prove the `worker::Delay` binding itself.
//! That binding is compile-verified on `wasm32-unknown-unknown` and can
//! only be exercised by a deployed probe, which this repository cannot
//! run.

use async_trait::async_trait;
use std::time::Duration;

/// A host primitive that returns to the runtime and resumes no earlier
/// than `duration` later.
///
/// `?Send` for the same reason [`auctioneer_runtime::Timer`] is: the futures
/// are driven on single-threaded edge runtimes, and `worker::Delay`
/// holds an `Rc` internally, so it could not be `Send` even in
/// principle.
#[async_trait(?Send)]
pub trait DelaySource: Send + Sync {
    /// Yield to the host, resuming after at least `duration`.
    ///
    /// `Duration::ZERO` is meaningful and must still reach the host: it
    /// is how a caller asks for one turn of the event loop without
    /// asking to wait.
    async fn delay(&self, duration: Duration);
}

/// The production source: Workers' `setTimeout`-backed future.
///
/// Only compiled for the deployment target, because `worker::Delay`
/// only exists there. Its correctness is a compile-time fact here and a
/// deployed-probe fact in production; no test in this repository
/// executes it.
#[cfg(target_arch = "wasm32")]
pub struct WorkerDelaySource;

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl DelaySource for WorkerDelaySource {
    async fn delay(&self, duration: Duration) {
        worker::Delay::from(duration).await;
    }
}
