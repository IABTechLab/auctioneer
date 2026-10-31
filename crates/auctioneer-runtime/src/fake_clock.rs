use crate::{Clock, MonotonicInstant, Timer};
use async_trait::async_trait;
use std::collections::BTreeMap;
use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll, Waker};
use std::time::Duration;
use time::{OffsetDateTime, PrimitiveDateTime};

/// Ceiling the wall-clock projection saturates to. `OffsetDateTime` has
/// no `MAX` constant of its own; the widest representable value is the
/// maximum primitive datetime read at UTC.
const MAX_WALL_CLOCK: OffsetDateTime = PrimitiveDateTime::MAX.assume_utc();

/// A registered sleeper. Keyed by a monotonically minted id so wake
/// order is registration order.
struct Waiter {
    wake_at: MonotonicInstant,
    waker: Waker,
}

struct FakeClockInner {
    /// Fixed axis origin, captured once in `new()`. Every instant this
    /// clock hands out is `origin + offset`, so the whole fake timeline
    /// sits in the real `MonotonicInstant` domain and its instants
    /// compare against `EdgeZero`'s without any projection.
    origin: MonotonicInstant,
    /// Distance from `origin` — only ever advanced.
    offset: Duration,
    next_waiter_id: u64,
    waiters: BTreeMap<u64, Waiter>,
}

impl FakeClockInner {
    /// The current instant on this clock's axis.
    ///
    /// Saturates at `origin` on overflow rather than panicking: the
    /// offset is test-supplied and `advance(Duration::MAX)` is a
    /// legitimate way to ask for "as far forward as possible".
    fn now_instant(&self) -> MonotonicInstant {
        self.origin.checked_add(self.offset).unwrap_or(self.origin)
    }

    /// Remove every waiter whose deadline has arrived and hand its waker
    /// back to the caller. The caller MUST wake outside the lock.
    ///
    /// `BTreeMap` iteration is key order, which is waiter-id order,
    /// which is registration order — so wake order is deterministic
    /// across runs.
    fn take_ready_wakers(&mut self) -> Vec<Waker> {
        let now = self.now_instant();
        let mut ready = Vec::new();
        let mut still_waiting = BTreeMap::new();
        for (id, waiter) in mem::take(&mut self.waiters) {
            if waiter.wake_at <= now {
                ready.push(waiter.waker);
            } else {
                still_waiting.insert(id, waiter);
            }
        }
        self.waiters = still_waiting;
        ready
    }
}

/// Deterministic test clock. Both wall and monotonic advance via
/// `advance()`; the wall axis is derived from the monotonic one so the
/// two can never drift apart.
///
/// Also the deterministic [`Timer`]: `sleep` registers a waiter that
/// `advance` / `advance_to` resolves, with no real time involved.
pub struct FakeClock {
    inner: Arc<Mutex<FakeClockInner>>,
    wall: OffsetDateTime,
}

impl FakeClock {
    #[must_use]
    #[inline]
    pub fn new(start: OffsetDateTime) -> Self {
        Self {
            inner: Arc::new(Mutex::new(FakeClockInner {
                origin: MonotonicInstant::now(),
                offset: Duration::ZERO,
                next_waiter_id: 0,
                waiters: BTreeMap::new(),
            })),
            wall: start,
        }
    }

    /// The axis origin — the instant `new()` captured, which is also
    /// what `now_monotonic()` returns before any `advance`.
    #[must_use]
    #[inline]
    pub fn origin(&self) -> MonotonicInstant {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .origin
    }

    /// Move the monotonic axis forward by `delta` and wake every sleeper
    /// whose deadline that reaches. Saturates rather than panicking on
    /// `Duration` overflow.
    #[inline]
    pub fn advance(&self, delta: Duration) {
        let ready = {
            let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
            inner.offset = inner.offset.saturating_add(delta);
            inner.take_ready_wakers()
        };
        Self::wake_all(ready);
    }

    /// Move the monotonic axis to `target` and wake every sleeper that
    /// reaches. A `target` at or behind the current instant is a no-op
    /// on the axis: a monotonic clock never moves backwards.
    #[inline]
    pub fn advance_to(&self, target: MonotonicInstant) {
        let ready = {
            let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(offset) = target.checked_duration_since(inner.origin)
                && offset > inner.offset
            {
                inner.offset = offset;
            }
            inner.take_ready_wakers()
        };
        Self::wake_all(ready);
    }

    /// Wake EVERY registered sleeper regardless of its wake instant,
    /// without moving the axis to an unrepresentable instant.
    ///
    /// This is what a test wants when it needs the registry emptied —
    /// "let every pending sleep resolve" — and it says so directly.
    /// The axis-based idiom it replaces asked for an instant so far in
    /// the future that it could not be represented at all once the
    /// axis became a real `MonotonicInstant`.
    #[inline]
    pub fn drain(&self) {
        let ready = {
            let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
            mem::take(&mut inner.waiters)
                .into_values()
                .map(|waiter| waiter.waker)
                .collect()
        };
        Self::wake_all(ready);
    }

    /// The furthest instant reachable from `now`, used as the wake
    /// instant for a sleep whose duration overflows the axis.
    ///
    /// Halving the span until the addition succeeds terminates because
    /// `Duration::ZERO` always does, and `checked_div` keeps the
    /// division itself total — a `None` there would mean dividing by
    /// zero, which cannot happen with the literal 2, so `now` is the
    /// correct fallback either way.
    fn unreachable_instant(now: MonotonicInstant) -> MonotonicInstant {
        let mut span = Duration::MAX;
        loop {
            if let Some(instant) = now.checked_add(span) {
                return instant;
            }
            let Some(halved) = span.checked_div(2) else {
                return now;
            };
            span = halved;
        }
    }

    /// Wake outside the lock. A woken future may poll immediately on
    /// this thread and re-enter the clock; waking under the lock would
    /// deadlock on the non-reentrant mutex.
    fn wake_all(ready: Vec<Waker>) {
        for waker in ready {
            waker.wake();
        }
    }
}

impl Clock for FakeClock {
    #[inline]
    fn now(&self) -> OffsetDateTime {
        // A poisoned lock means some other test panicked while holding
        // it; the value itself is intact, only the guard was
        // contaminated, so recover and continue.
        let inner_now = match self.inner.lock() {
            Ok(guard) => guard.offset,
            Err(poisoned) => poisoned.into_inner().offset,
        };
        // Three failure modes each saturate to the maximum representable
        // datetime: the monotonic axis exceeding `time::Duration`'s range
        // (its max is `i64::MAX` seconds while `std`'s is `u64::MAX`),
        // the addition leaving the supported year range, and an
        // already-maximal starting wall clock.
        let Ok(delta) = time::Duration::try_from(inner_now) else {
            return MAX_WALL_CLOCK;
        };
        self.wall.checked_add(delta).unwrap_or(MAX_WALL_CLOCK)
    }

    #[inline]
    fn now_monotonic(&self) -> MonotonicInstant {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .now_instant()
    }
}

#[async_trait(?Send)]
impl Timer for FakeClock {
    async fn sleep(&self, duration: Duration) {
        // Fast path: a zero sleep never enters the registry, so it costs
        // no lock contention and leaves nothing to cancel.
        if duration.is_zero() {
            return;
        }
        // A wake instant the axis cannot represent means "never on its
        // own" — no `advance` can reach it. Pinning it to the current
        // instant would instead make it fire immediately, so the
        // unrepresentable case parks at the far end of the axis and is
        // released only by `drain()`.
        let wake_at = {
            let inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
            let now = inner.now_instant();
            now.checked_add(duration)
                .unwrap_or_else(|| Self::unreachable_instant(now))
        };
        SleepFuture {
            inner: Arc::clone(&self.inner),
            wake_at,
            waiter_id: None,
        }
        .await;
    }
}

/// Registry-backed sleep handle. Registers on first poll, replaces its
/// stored waker on repoll, and deregisters on drop.
struct SleepFuture {
    inner: Arc<Mutex<FakeClockInner>>,
    wake_at: MonotonicInstant,
    waiter_id: Option<u64>,
}

impl Future for SleepFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = self.get_mut();
        let mut inner = this.inner.lock().unwrap_or_else(PoisonError::into_inner);
        match this.waiter_id {
            None => {
                if inner.now_instant() >= this.wake_at {
                    return Poll::Ready(());
                }
                let waiter_id = inner.next_waiter_id;
                #[expect(
                    clippy::expect_used,
                    reason = "u64 waiter id overflow requires 2^64 registrations in one FakeClock lifetime — treating it as a workspace-invariant panic is safer than silently wrapping and colliding waiter identities"
                )]
                let next_waiter_id = waiter_id.checked_add(1).expect("waiter id overflow");
                inner.next_waiter_id = next_waiter_id;
                inner.waiters.insert(
                    waiter_id,
                    Waiter {
                        wake_at: this.wake_at,
                        waker: cx.waker().clone(),
                    },
                );
                this.waiter_id = Some(waiter_id);
                Poll::Pending
            }
            Some(waiter_id) => {
                let Some(waiter) = inner.waiters.get_mut(&waiter_id) else {
                    // The entry is gone, so the wake step already fired.
                    // Clearing the id also makes `Drop` a no-op.
                    this.waiter_id = None;
                    return Poll::Ready(());
                };
                // Replace only when the executor handed us a different
                // waker; `will_wake` avoids a pointless
                // drop-clone-store on the same one.
                if !waiter.waker.will_wake(cx.waker()) {
                    waiter.waker.clone_from(cx.waker());
                }
                Poll::Pending
            }
        }
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for SleepFuture {
    fn drop(&mut self) {
        // Cancellation. `Drop` acquires and releases synchronously, and
        // ownership rules mean a poll of the same future cannot be in
        // flight, so this cannot race the poll path.
        if let Some(waiter_id) = self.waiter_id {
            let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
            inner.waiters.remove(&waiter_id);
        }
    }
}

#[cfg(test)]
impl FakeClock {
    /// Registry depth, for the cancellation / registration assertions.
    ///
    /// Deliberately `cfg(test)` and crate-private: `cfg(test)` in a
    /// library does NOT extend to an integration-test binary, so any
    /// future integration coverage would need this behind a
    /// `test-inspection` Cargo feature instead.
    fn waiters_len(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .waiters
            .len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_clock_advances_both_axes() {
        let start = OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("valid unix ts");
        let clock = FakeClock::new(start);
        let origin = clock.origin();
        clock.advance(Duration::from_millis(500));
        assert_eq!(
            clock.now_monotonic(),
            origin
                .checked_add(Duration::from_millis(500))
                .expect("no overflow")
        );
        assert_eq!(clock.now(), start + Duration::from_millis(500));
    }

    #[test]
    fn fake_clock_starts_at_provided_time() {
        let start = OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("valid unix ts");
        let clock = FakeClock::new(start);
        assert_eq!(clock.now(), start);
        assert_eq!(
            clock.now_monotonic(),
            clock.origin(),
            "a fresh clock sits exactly on its axis origin"
        );
    }

    #[test]
    fn now_saturates_to_max_offset_datetime_after_unbounded_advance() {
        let clock = FakeClock::new(OffsetDateTime::UNIX_EPOCH);
        clock.advance(Duration::MAX);
        assert_eq!(
            clock.now(),
            MAX_WALL_CLOCK,
            "an unbounded monotonic axis must saturate the wall projection, not panic"
        );
    }

    #[test]
    fn advance_to_never_moves_the_axis_backwards() {
        let clock = FakeClock::new(OffsetDateTime::UNIX_EPOCH);
        clock.advance(Duration::from_secs(10));
        let reached = clock.now_monotonic();
        clock.advance_to(
            clock
                .origin()
                .checked_add(Duration::from_secs(1))
                .expect("no overflow"),
        );
        assert_eq!(
            clock.now_monotonic(),
            reached,
            "a target behind now leaves the axis where it was"
        );
        clock.advance_to(
            clock
                .origin()
                .checked_add(Duration::from_secs(25))
                .expect("no overflow"),
        );
        assert_eq!(
            clock.now_monotonic(),
            clock
                .origin()
                .checked_add(Duration::from_secs(25))
                .expect("no overflow"),
            "a target ahead of now moves the axis exactly onto it"
        );
    }

    #[test]
    fn now_monotonic_sits_in_the_real_instant_domain() {
        let before = MonotonicInstant::now();
        let clock = FakeClock::new(OffsetDateTime::UNIX_EPOCH);
        let after = MonotonicInstant::now();
        let origin = clock.origin();
        assert!(
            origin >= before && origin <= after,
            "the axis origin is a real MonotonicInstant, not a synthetic zero"
        );
    }

    #[test]
    fn now_saturates_when_inner_exceeds_time_duration_range() {
        let clock = FakeClock::new(OffsetDateTime::UNIX_EPOCH);
        // `time::Duration` tops out at `i64::MAX` seconds while `std`'s
        // goes to `u64::MAX`, so this value fails the conversion before
        // the datetime addition is ever attempted.
        clock.inner.lock().expect("fresh mutex").offset = Duration::from_secs(u64::MAX);
        assert_eq!(
            clock.now(),
            MAX_WALL_CLOCK,
            "a monotonic axis outside time::Duration's range must saturate"
        );
    }
}

#[cfg(test)]
mod timer_tests {
    use super::{Clock as _, Duration, FakeClock, MonotonicInstant, OffsetDateTime, Timer as _};
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll, Wake, Waker};

    /// Waker that records its own id when invoked, so a test can prove
    /// WHICH waker the registry held.
    struct RecordingWaker {
        id: u8,
        log: Arc<Mutex<Vec<u8>>>,
    }

    impl Wake for RecordingWaker {
        fn wake(self: Arc<Self>) {
            self.wake_by_ref();
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.log.lock().expect("uncontended").push(self.id);
        }
    }

    fn recording_waker(id: u8, log: &Arc<Mutex<Vec<u8>>>) -> Waker {
        Waker::from(Arc::new(RecordingWaker {
            id,
            log: Arc::clone(log),
        }))
    }

    fn clock() -> FakeClock {
        FakeClock::new(OffsetDateTime::UNIX_EPOCH)
    }

    #[test]
    fn zero_duration_sleep_returns_ready_without_registering() {
        let clock = clock();
        let log = Arc::new(Mutex::new(Vec::new()));
        let waker = recording_waker(1, &log);
        let mut sleep = clock.sleep(Duration::ZERO);
        assert_eq!(
            sleep.as_mut().poll(&mut Context::from_waker(&waker)),
            Poll::Ready(()),
            "a zero sleep resolves on the first poll"
        );
        assert_eq!(
            clock.waiters_len(),
            0,
            "the zero-duration fast path must never enter the registry"
        );
    }

    #[test]
    fn elapsed_sleep_wakes_in_registration_order() {
        let clock = clock();
        let log = Arc::new(Mutex::new(Vec::new()));
        let waker_a = recording_waker(1, &log);
        let waker_b = recording_waker(2, &log);
        let mut sleep_a = clock.sleep(Duration::from_millis(10));
        let mut sleep_b = clock.sleep(Duration::from_millis(20));
        assert!(
            sleep_a
                .as_mut()
                .poll(&mut Context::from_waker(&waker_a))
                .is_pending(),
            "a 10ms sleep is pending at t=0"
        );
        assert!(
            sleep_b
                .as_mut()
                .poll(&mut Context::from_waker(&waker_b))
                .is_pending(),
            "a 20ms sleep is pending at t=0"
        );
        assert_eq!(clock.waiters_len(), 2, "both sleeps registered");

        clock.advance(Duration::from_millis(15));
        assert_eq!(
            *log.lock().expect("uncontended"),
            vec![1_u8],
            "only the 10ms waiter is due at t=15ms"
        );
        assert_eq!(
            sleep_a.as_mut().poll(&mut Context::from_waker(&waker_a)),
            Poll::Ready(()),
            "the elapsed sleep resolves on its next poll"
        );
        assert!(
            sleep_b
                .as_mut()
                .poll(&mut Context::from_waker(&waker_b))
                .is_pending(),
            "the 20ms sleep is still pending at t=15ms"
        );
        assert_eq!(clock.waiters_len(), 1, "only the 20ms waiter remains");

        clock.advance(Duration::from_millis(10));
        assert_eq!(
            *log.lock().expect("uncontended"),
            vec![1_u8, 2_u8],
            "wake order follows registration order"
        );
    }

    #[test]
    fn dropped_sleep_removes_waiter() {
        let clock = clock();
        let log = Arc::new(Mutex::new(Vec::new()));
        let waker = recording_waker(1, &log);
        let mut sleep = clock.sleep(Duration::from_millis(50));
        assert!(
            sleep
                .as_mut()
                .poll(&mut Context::from_waker(&waker))
                .is_pending(),
            "the sleep registers on its first poll"
        );
        assert_eq!(clock.waiters_len(), 1, "one registered waiter");
        drop(sleep);
        assert_eq!(clock.waiters_len(), 0, "dropping the future deregisters it");

        clock.advance(Duration::from_millis(60));
        assert!(
            log.lock().expect("uncontended").is_empty(),
            "a cancelled sleep must not be woken"
        );
    }

    #[test]
    fn waker_replacement_on_repoll() {
        let clock = clock();
        let log = Arc::new(Mutex::new(Vec::new()));
        let waker_a = recording_waker(1, &log);
        let waker_b = recording_waker(2, &log);
        let mut sleep = clock.sleep(Duration::from_millis(10));
        assert!(
            sleep
                .as_mut()
                .poll(&mut Context::from_waker(&waker_a))
                .is_pending(),
            "first poll registers waker A"
        );
        assert!(
            sleep
                .as_mut()
                .poll(&mut Context::from_waker(&waker_b))
                .is_pending(),
            "second poll replaces the stored waker with B"
        );

        clock.advance(Duration::from_millis(10));
        assert_eq!(
            *log.lock().expect("uncontended"),
            vec![2_u8],
            "only the most recently supplied waker is invoked"
        );
    }

    /// Waker whose `wake` re-enters the clock. If `advance` woke under
    /// its own lock this would deadlock on the non-reentrant mutex.
    struct LockReacquiringWaker {
        clock: Arc<FakeClock>,
        observed: Arc<Mutex<Vec<MonotonicInstant>>>,
    }

    impl Wake for LockReacquiringWaker {
        fn wake(self: Arc<Self>) {
            self.wake_by_ref();
        }

        fn wake_by_ref(self: &Arc<Self>) {
            let seen = self.clock.now_monotonic();
            self.observed.lock().expect("uncontended").push(seen);
        }
    }

    #[test]
    fn wake_does_not_deadlock_when_waker_reacquires_lock() {
        let clock = Arc::new(clock());
        let observed = Arc::new(Mutex::new(Vec::new()));
        let waker = Waker::from(Arc::new(LockReacquiringWaker {
            clock: Arc::clone(&clock),
            observed: Arc::clone(&observed),
        }));
        let mut sleep = clock.sleep(Duration::from_millis(10));
        assert!(
            sleep
                .as_mut()
                .poll(&mut Context::from_waker(&waker))
                .is_pending(),
            "the sleep registers the re-entrant waker"
        );

        clock.advance(Duration::from_millis(10));
        assert_eq!(
            *observed.lock().expect("uncontended"),
            vec![
                clock
                    .origin()
                    .checked_add(Duration::from_millis(10))
                    .expect("no overflow")
            ],
            "the waker observed the advanced clock without deadlocking"
        );
    }

    #[test]
    fn fake_clock_drain_wakes_every_sleeper() {
        let clock = clock();
        let log = Arc::new(Mutex::new(Vec::new()));
        let waker_near = recording_waker(1, &log);
        let waker_far = recording_waker(2, &log);
        let mut near = clock.sleep(Duration::from_millis(10));
        // A sleep no `advance` can reach: its wake instant overflows
        // the axis, so only `drain` releases it.
        let mut far = clock.sleep(Duration::MAX);
        assert!(
            near.as_mut()
                .poll(&mut Context::from_waker(&waker_near))
                .is_pending(),
            "the near sleep registers"
        );
        assert!(
            far.as_mut()
                .poll(&mut Context::from_waker(&waker_far))
                .is_pending(),
            "the unreachable sleep registers"
        );

        clock.drain();
        assert_eq!(
            *log.lock().expect("uncontended"),
            vec![1_u8, 2_u8],
            "drain wakes every registered sleeper in registration order"
        );
        assert_eq!(
            clock.waiters_len(),
            0,
            "drain empties the registry outright"
        );
        assert_eq!(
            near.as_mut().poll(&mut Context::from_waker(&waker_near)),
            Poll::Ready(()),
            "a drained sleep resolves on its next poll"
        );
        assert_eq!(
            far.as_mut().poll(&mut Context::from_waker(&waker_far)),
            Poll::Ready(()),
            "even the unreachable sleep resolves after a drain"
        );
    }
}
