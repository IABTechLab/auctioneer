//! Counted config work for unsheddable liveness probes.
//!
//! Saturation renders the last verdict without copying its banner. Before any verdict exists,
//! callers wait asynchronously for a verdict or a released work slot. Every
//! actual resolution owns a permit, including recovery after cancellation.
//! Response buffers have a separate, platform-population budget; config slots
//! are held through rendering, not through terminal egress.

use crate::config::runtime::ConfigStatusCode;
use crate::host_primitives::gates::boot_limits;
use auctioneer_runtime::{AuxGate, AuxPermit, PROBE_CONFIG_MIN_PERMITS};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::task::{Context, Poll, Waker};

static PROBE_CONFIG_GATE: OnceLock<Arc<AuxGate>> = OnceLock::new();
const PROBE_CONFIG_FLOOR: u32 = PROBE_CONFIG_MIN_PERMITS;

fn probe_config_capacity(aux_route_permits: Option<u32>) -> u32 {
    aux_route_permits.map_or(PROBE_CONFIG_FLOOR, |permits| {
        permits.max(PROBE_CONFIG_FLOOR)
    })
}

fn gate() -> &'static Arc<AuxGate> {
    PROBE_CONFIG_GATE.get_or_init(|| {
        AuxGate::new(probe_config_capacity(
            boot_limits().map(|row| row.aux_route_permits),
        ))
    })
}

#[derive(Default)]
struct ProbeState {
    verdict: Option<ProbeVerdict>,
    // Stable indices let a cancelled waiter remove its waker immediately.
    waiters: Vec<Option<Waker>>,
}

static STATE: Mutex<ProbeState> = Mutex::new(ProbeState {
    verdict: None,
    waiters: Vec::new(),
});

fn state() -> MutexGuard<'static, ProbeState> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn wake_waiters(waiters: Vec<Waker>) {
    for waiter in waiters {
        waiter.wake();
    }
}

/// The bounded banner and closed fault code, borrowed by cached renderers.
#[derive(Debug)]
pub(super) struct ProbeVerdict {
    pub banner: Box<str>,
    pub fault: Option<ConfigStatusCode>,
}

pub(super) enum ConfigClaim<Cached> {
    Work(ConfigWorkPermit),
    Cached(Cached),
}

/// A resolution owns a real slot; cancellation releases it and wakes waiters.
pub(super) struct ConfigWorkPermit(Option<AuxPermit>);

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for ConfigWorkPermit {
    fn drop(&mut self) {
        let waiters = {
            let state = state();
            drop(self.0.take());
            state.waiters.iter().flatten().cloned().collect()
        };
        wake_waiters(waiters);
    }
}

pub(super) fn claim_config_work<Cached>(
    render_cached: fn(&ProbeVerdict) -> Cached,
) -> impl Future<Output = ConfigClaim<Cached>> {
    ClaimConfigWork {
        waiter: None,
        render_cached,
    }
}

struct ClaimConfigWork<Cached> {
    waiter: Option<usize>,
    render_cached: fn(&ProbeVerdict) -> Cached,
}

impl<Cached> ClaimConfigWork<Cached> {
    fn unregister(&mut self, state: &mut ProbeState) {
        if let Some(index) = self.waiter.take() {
            if let Some(slot) = state.waiters.get_mut(index) {
                *slot = None;
            }
            while state.waiters.last().is_some_and(Option::is_none) {
                state.waiters.pop();
            }
        }
    }
}

impl<Cached> Future for ClaimConfigWork<Cached> {
    type Output = ConfigClaim<Cached>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = state();
        // A cold waiter shares the verdict that woke it, even if the resolver
        // also freed a slot. Otherwise each waiter would start another read.
        if self.waiter.is_some()
            && let Some(verdict) = state.verdict.as_ref()
        {
            let cached = (self.render_cached)(verdict);
            self.unregister(&mut state);
            return Poll::Ready(ConfigClaim::Cached(cached));
        }
        if let Some(permit) = gate().try_acquire() {
            self.unregister(&mut state);
            return Poll::Ready(ConfigClaim::Work(ConfigWorkPermit(Some(permit))));
        }
        if let Some(verdict) = state.verdict.as_ref() {
            let cached = (self.render_cached)(verdict);
            self.unregister(&mut state);
            return Poll::Ready(ConfigClaim::Cached(cached));
        }
        // Checking the cache, acquiring and registering all share the lock
        // with publication and permit release, so no wakeup can be missed.
        let index = *self.waiter.get_or_insert_with(|| {
            state
                .waiters
                .iter()
                .position(Option::is_none)
                .unwrap_or_else(|| {
                    let next = state.waiters.len();
                    state.waiters.push(None);
                    next
                })
        });
        if let Some(slot) = state.waiters.get_mut(index) {
            *slot = Some(cx.waker().clone());
        }
        Poll::Pending
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl<Cached> Drop for ClaimConfigWork<Cached> {
    fn drop(&mut self) {
        self.unregister(&mut state());
    }
}

pub(super) fn record_verdict(verdict: ProbeVerdict) {
    let waiters = {
        let mut state = state();
        state.verdict = Some(verdict);
        state.waiters.iter().flatten().cloned().collect()
    };
    wake_waiters(waiters);
}

/// Slots held by actual probe resolutions, including response rendering.
#[must_use]
pub fn config_work_in_flight() -> u32 {
    PROBE_CONFIG_GATE.get().map_or(0, |gate| gate.in_use())
}

#[must_use]
pub fn config_work_capacity() -> Option<u32> {
    PROBE_CONFIG_GATE.get().map(|gate| gate.capacity())
}

static CONFIG_RESOLUTIONS: AtomicU32 = AtomicU32::new(0);

pub(super) fn note_config_resolution() {
    CONFIG_RESOLUTIONS.fetch_add(1, Ordering::Relaxed);
}

#[must_use]
pub fn config_resolutions() -> u32 {
    CONFIG_RESOLUTIONS.load(Ordering::Relaxed)
}

pub(super) static PROBE_ISOLATION: Mutex<()> = Mutex::new(());

/// Serializes and clears process-global state for real-handler tests.
#[doc(hidden)]
#[must_use]
pub fn __probe_isolation_for_tests() -> ProbeIsolation {
    let guard = PROBE_ISOLATION
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    state().verdict = None;
    ProbeIsolation { _guard: guard }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct ProbeIsolation {
    _guard: MutexGuard<'static, ()>,
}

#[cfg(test)]
mod tests {
    use super::{
        ConfigClaim, PROBE_CONFIG_FLOOR, PROBE_ISOLATION, claim_config_work, gate,
        probe_config_capacity, state,
    };
    use futures::executor::block_on;
    use futures::task::noop_waker;
    use std::future::Future as _;
    use std::task::Context;

    #[test]
    fn the_envelope_can_raise_the_probe_capacity_but_not_lower_it() {
        assert_eq!(probe_config_capacity(Some(1)), PROBE_CONFIG_FLOOR);
        assert_eq!(probe_config_capacity(Some(0)), PROBE_CONFIG_FLOOR);
        assert_eq!(probe_config_capacity(Some(16)), 16);
        assert_eq!(probe_config_capacity(None), PROBE_CONFIG_FLOOR);
    }

    #[test]
    fn cancelled_cold_waiters_remove_their_wakers_and_repolls_do_not_duplicate_them() {
        let _isolation = PROBE_ISOLATION.lock().unwrap();
        state().verdict = None;
        let mut permits = Vec::new();
        for _ in 0..gate().capacity() {
            let ConfigClaim::Work(permit) = block_on(claim_config_work(|_| ())) else {
                panic!("an idle config slot must be available");
            };
            permits.push(permit);
        }
        for _ in 0_u32..128 {
            let mut claim = Box::pin(claim_config_work(|_| ()));
            for _ in 0_u32..2 {
                assert!(
                    claim
                        .as_mut()
                        .poll(&mut Context::from_waker(&noop_waker()))
                        .is_pending()
                );
                assert_eq!(state().waiters.len(), 1);
            }
            drop(claim);
            assert!(state().waiters.is_empty());
        }
        let mut first = Box::pin(claim_config_work(|_| ()));
        let mut middle = Box::pin(claim_config_work(|_| ()));
        let mut last = Box::pin(claim_config_work(|_| ()));
        for claim in [&mut first, &mut middle, &mut last] {
            assert!(
                claim
                    .as_mut()
                    .poll(&mut Context::from_waker(&noop_waker()))
                    .is_pending()
            );
        }
        assert_eq!(state().waiters.len(), 3);
        drop(first);
        let mut replacement = Box::pin(claim_config_work(|_| ()));
        assert!(
            replacement
                .as_mut()
                .poll(&mut Context::from_waker(&noop_waker()))
                .is_pending()
        );
        assert_eq!(state().waiters.len(), 3, "reuse a cancelled non-tail slot");
        drop(middle);
        drop(last);
        drop(replacement);
        assert!(state().waiters.is_empty());
        drop(permits);
        assert_eq!(gate().in_use(), 0);
    }
}
