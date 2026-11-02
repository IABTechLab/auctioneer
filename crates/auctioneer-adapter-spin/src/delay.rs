//! `DelaySource` — the seam between this adapter's sleep primitive and
//! the WASI monotonic-clock future that backs it.
//!
//! The trait exists because `spin_sdk` is a `wasm32`-only dependency:
//! the crate sits under `[target.'cfg(target_arch = "wasm32")'
//! .dependencies]`, so on the host `spin_sdk::time::sleep` does not
//! exist at all, and this repository has no wasm runner. Without the
//! seam [`SpinTimer`](crate::timer::SpinTimer) could not be exercised
//! anywhere: not on the host, for want of the function, and not on
//! wasm, for want of anything to run the test.
//!
//! What that buys and what it does not: a host test proves the LOGIC
//! built on top of a source — that a sleep awaits it once for the
//! duration asked for, and that the wave's acquisition race observes
//! it. It does NOT prove the `spin_sdk::time::sleep` binding itself.
//! That binding is compile-verified on `wasm32-wasip2` and could only
//! be exercised by a live component test, which this repository cannot
//! run.
//!
//! Only the timer takes a source. The decode cooperator does not: Spin
//! runs on WASI, whose monotonic clock keeps advancing across guest CPU
//! work, so the cooperator owes no clock refresh and needs no host
//! event to produce one. See [`crate::cooperator`].

use async_trait::async_trait;
use std::time::Duration;

/// A host primitive that returns to the runtime and resumes no earlier
/// than `duration` later.
///
/// `?Send` for the same reason [`auctioneer_runtime::Timer`] is: the futures
/// are driven on single-threaded edge runtimes. The trait object itself
/// is shared across requests, so the trait still requires `Send + Sync`.
#[async_trait(?Send)]
pub trait DelaySource: Send + Sync {
    /// Yield to the host, resuming after at least `duration`.
    async fn delay(&self, duration: Duration);
}

/// The production source: Spin's WASI monotonic-clock sleep.
///
/// Only compiled for the deployment target, because `spin_sdk` only
/// exists there. `spin_sdk::time::sleep` is the same primitive
/// `EdgeZero`'s own Spin adapter awaits for its deadline timers, so this
/// adapter's timer and the transport beneath it wait on one mechanism
/// rather than two.
#[cfg(target_arch = "wasm32")]
pub struct SpinDelaySource;

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl DelaySource for SpinDelaySource {
    async fn delay(&self, duration: Duration) {
        use spin_sdk::time::sleep;

        sleep(duration).await;
    }
}
