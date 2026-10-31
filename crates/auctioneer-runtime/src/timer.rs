//! `Timer` — the async sleep seam — and `DecodeCooperator`, the
//! host-yield seam auctioneer's own decode uses, plus the inert stub of
//! each that an adapter installs until it grows a real host primitive.

use async_trait::async_trait;
use std::time::Duration;

/// Host-provided sleep primitive.
///
/// `?Send` because the fanout futures are driven on single-threaded
/// edge runtimes; the trait object itself is shared across requests, so
/// the trait still requires `Send + Sync`.
#[async_trait(?Send)]
pub trait Timer: Send + Sync {
    /// Sleep for the given duration.
    ///
    /// Callers compute `duration = deadline_at.saturating_sub(clock
    /// .now_monotonic())` AT CALL TIME. A `sleep_until(deadline:
    /// Duration)` API would be ambiguous because the epoch of a
    /// `Duration` is impl-defined — the system clock's monotonic axis is
    /// a per-process anchor initialised on first call, while a runtime's
    /// own instant type has its own epoch. Taking a relative duration
    /// lifts the epoch question out of the trait and lets each impl
    /// convert to its runtime primitive directly.
    async fn sleep(&self, duration: Duration);
}

/// `Timer` that resolves immediately regardless of the requested
/// duration.
///
/// Installed by the adapters that have no host sleep primitive yet. It
/// is inert under a strategy that never awaits the timer; a strategy
/// that DOES race timers must not be paired with it.
pub struct ImmediateResolveTimer;

#[async_trait(?Send)]
impl Timer for ImmediateResolveTimer {
    async fn sleep(&self, _duration: Duration) {}
}

/// Host-provided cooperative yield point for auctioneer's OWN decode work.
///
/// Separate from [`Timer`] because it answers a different question. A
/// timer is asked to wait a stated duration; a cooperator is asked only
/// to return to the host once, so that a runtime whose clock advances
/// only at host boundaries reports a fresh instant afterwards. On such a
/// runtime — a Workers isolate, where `Date.now()` is frozen for the
/// duration of a synchronous turn — a long CPU-bound decode would
/// otherwise see the same instant before and after, and auctioneer's own
/// deadline recheck could never fire.
///
/// A bare `yield_now()` is NOT a valid implementation on those targets:
/// it re-polls inside the same turn and refreshes nothing. The impl has
/// to await a real host event.
#[async_trait(?Send)]
pub trait DecodeCooperator: Send + Sync {
    /// Return to the host once, then resume.
    async fn yield_to_host(&self);
}

/// `DecodeCooperator` that resumes immediately without touching the
/// host.
///
/// Correct on a target whose clock is not turn-frozen — a native host
/// reads a fresh instant on every call — and the default for an adapter
/// that has no host yield primitive yet. On a frozen-clock runtime it
/// makes the decode recheck inert, which is why those adapters install a
/// real one.
pub struct NoopDecodeCooperator;

#[async_trait(?Send)]
impl DecodeCooperator for NoopDecodeCooperator {
    async fn yield_to_host(&self) {}
}
