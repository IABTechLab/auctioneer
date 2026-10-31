//! Portable admission gates: shed immediately, or queue and wait.
//!
//! [`AdmissionGate`] bounds the number of concurrent in-flight requests
//! with a single [`AtomicU32`]. Acquisition is immediate and never
//! queues: [`AdmissionGate::try_acquire`] either hands back an
//! [`AdmissionPermit`] lease or returns `None` when the gate is full, so
//! the caller can shed load without waiting or allocating. Capacity
//! comes from the baked boot envelope.
//!
//! The permit is an RAII lease: dropping it — including while a panic
//! unwinds — returns exactly one slot to the gate. It owns a handle on
//! its gate rather than borrowing one, so it can be moved into
//! per-request state that outlives the frame that acquired it.
//!
//! [`StreamGate`] is the queued counterpart: callers that must not be
//! shed wait their turn instead. It leases weight in all-or-nothing
//! waves ([`StreamGate::acquire_wave`]) in strict arrival order, and
//! its acquisition future is cancellation-safe at both stages — a
//! waiter that walks away holds nothing to leak, and one that walks
//! away after winning hands its whole wave to the next waiter.

use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::task::{Context, Poll, Waker};

/// Bounded concurrency gate with immediate, non-blocking admission.
///
/// The gate holds a single in-use counter against a fixed capacity.
/// [`try_acquire`](AdmissionGate::try_acquire) claims a slot with one
/// compare-and-swap loop and never waits; when every slot is taken it
/// returns `None` so the caller can shed the request. A gate built with
/// capacity `0` admits nothing.
#[derive(Debug)]
pub struct AdmissionGate {
    in_use: AtomicU32,
    capacity: u32,
}

impl AdmissionGate {
    /// Builds a gate admitting at most `capacity` concurrent holders.
    ///
    /// The gate is installed once per isolate or process and shared
    /// through the returned handle.
    #[must_use]
    pub fn new(capacity: u32) -> Arc<Self> {
        Arc::new(AdmissionGate {
            in_use: AtomicU32::new(0),
            capacity,
        })
    }

    /// Claims a slot, or returns `None` when the gate is full.
    ///
    /// Allocation-free on both paths: a CAS loop that retries only when
    /// another thread changed the counter concurrently, and an
    /// `Arc::clone` that bumps a refcount rather than allocating.
    #[must_use]
    pub fn try_acquire(self: &Arc<Self>) -> Option<AdmissionPermit> {
        self.in_use
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                // `current` never exceeds `capacity`, so the successor
                // cannot overflow a `u32` — but stay saturating rather
                // than trust that invariant with arithmetic.
                (current < self.capacity).then(|| current.saturating_add(1))
            })
            .ok()?;
        Some(AdmissionPermit {
            gate: Arc::clone(self),
        })
    }

    /// Slots currently leased out. Exposed for tests and diagnostics.
    #[must_use]
    pub fn in_use(&self) -> u32 {
        self.in_use.load(Ordering::Acquire)
    }
}

/// RAII lease on one [`AdmissionGate`] slot.
///
/// Dropping the permit — normally or while a panic unwinds — releases
/// exactly one slot. The permit keeps its gate alive through an owned
/// handle, so it may be moved into request-scoped state, and it is not
/// `Clone`, so a slot cannot be released twice.
#[derive(Debug)]
pub struct AdmissionPermit {
    gate: Arc<AdmissionGate>,
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for AdmissionPermit {
    fn drop(&mut self) {
        // One permit == one successful increment, so this never
        // over-decrements.
        self.gate.in_use.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Why a wave acquisition can never be satisfied.
///
/// Both variants are caller bugs rather than flow control: fanout
/// clamps a wave to the gate capacity before acquiring, so a gate that
/// sees either of these has been handed a width it could not have
/// produced. They are reported on the FIRST poll, before any waiter
/// node exists, so an erroring acquisition never touches gate state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveError {
    /// A wave of zero calls was requested. There is nothing to lease.
    ZeroWave,
    /// The wave is wider than the gate, so it could never grant.
    WaveExceedsCapacity {
        /// The width the caller asked for.
        requested: u32,
        /// The gate's fixed total weight.
        capacity: u32,
    },
}

impl Display for WaveError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match *self {
            WaveError::ZeroWave => f.write_str("a wave must carry at least one call"),
            WaveError::WaveExceedsCapacity {
                requested,
                capacity,
            } => write!(
                f,
                "a wave of {requested} exceeds the gate capacity of {capacity}"
            ),
        }
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "leaf error retains default source and legacy hooks; type_id and provide are unstable"
)]
impl Error for WaveError {}

/// What a waiter node is doing right now.
#[derive(Debug)]
enum WaiterState {
    /// Queued, holding no weight, with the waker of its latest poller.
    Waiting(Waker),
    /// Won its whole wave; `requested` weight is parked on the node
    /// until the future repolls into a guard, or cancels and hands it
    /// back.
    Granted,
}

/// One entry in the gate's waiter table.
#[derive(Debug)]
struct Waiter {
    requested: u32,
    state: WaiterState,
}

/// The gate's mutable state, guarded by one mutex.
///
/// The three weight counters are mutated independently and never
/// derived from one another, so a lost release or a double grant trips
/// the invariant instead of cancelling out.
#[derive(Debug)]
struct GateInner {
    /// Weight assigned to nobody.
    free: u32,
    /// Weight inside live [`WavePermit`] guards.
    held: u32,
    /// Weight parked on `Granted` nodes awaiting their repoll.
    granted: u32,
    /// Ids of WAITING nodes in arrival order. A granted id is popped;
    /// a cancelled id is removed here as well as from `nodes`.
    order: VecDeque<u64>,
    /// Every live node, waiting or granted, by id.
    nodes: HashMap<u64, Waiter>,
    next_id: u64,
}

impl GateInner {
    /// Debug-asserts that no weight has been lost or duplicated.
    fn assert_invariant(&self, capacity: u32) {
        debug_assert_eq!(
            self.free
                .saturating_add(self.held)
                .saturating_add(self.granted),
            capacity,
            "gate weight leaked: free={} held={} granted={} capacity={capacity}",
            self.free,
            self.held,
            self.granted
        );
        debug_assert_eq!(
            self.granted,
            self.nodes
                .values()
                .filter(|waiter| matches!(waiter.state, WaiterState::Granted))
                .map(|waiter| waiter.requested)
                .fold(0_u32, u32::saturating_add),
            "parked weight disagrees with the granted nodes holding it"
        );
        debug_assert!(
            self.order.len() <= self.nodes.len(),
            "the waiting order retains ids with no live node: order={} nodes={}",
            self.order.len(),
            self.nodes.len()
        );
    }

    /// Grants whole waves off the head of the queue while the head is
    /// satisfiable, returning the wakers to fire once the lock is
    /// released.
    ///
    /// The scan continues past each grant, so one release can satisfy
    /// several small waves; it stops at a head too wide for the free
    /// weight, which is the deliberate head-of-line block.
    fn scan_grants(&mut self) -> Vec<Waker> {
        let mut wakers = Vec::new();
        while let Some(&head_id) = self.order.front() {
            let Some(head) = self.nodes.get_mut(&head_id) else {
                // A cancelled leftover; cancellation normally removes
                // its own id, so this is belt and braces.
                self.order.pop_front();
                continue;
            };
            if !matches!(head.state, WaiterState::Waiting(_)) || self.free < head.requested {
                break;
            }
            let requested = head.requested;
            let previous = mem::replace(&mut head.state, WaiterState::Granted);
            self.free = self.free.saturating_sub(requested);
            self.granted = self.granted.saturating_add(requested);
            self.order.pop_front();
            if let WaiterState::Waiting(waker) = previous {
                wakers.push(waker);
            }
        }
        wakers
    }

    /// Drops a waiting id from the arrival order immediately.
    ///
    /// Reclaiming here rather than letting the scan skip the id later
    /// is what bounds retained storage: the scan pops stale ids only
    /// off the FRONT and stops at an unsatisfiable head, so any id
    /// behind a blocked head would never be reached.
    fn forget_waiting(&mut self, id: u64) {
        self.order.retain(|&queued| queued != id);
    }
}

/// Weighted FIFO admission gate with queued, all-or-nothing waves.
///
/// Where [`AdmissionGate`] sheds load, `StreamGate` makes callers wait
/// their turn. [`acquire_wave`](StreamGate::acquire_wave) leases `k`
/// units of a fixed capacity at once: the waiter holds ZERO weight
/// until all `k` grant in a single transition, so a partial
/// reservation never exists and an abandoned wait releases nothing.
///
/// Order is strictly first-come, first-served by arrival and NOT by
/// width. A wide wave at the head blocks narrower waves behind it until
/// it can grant — deliberate, because letting narrow waves overtake
/// would starve full-width waves forever at saturation.
#[derive(Debug)]
pub struct StreamGate {
    capacity: u32,
    inner: Mutex<GateInner>,
}

impl StreamGate {
    /// Builds a gate leasing at most `capacity` units of weight.
    #[must_use]
    pub fn new(capacity: u32) -> Arc<Self> {
        Arc::new(StreamGate {
            capacity,
            inner: Mutex::new(GateInner {
                free: capacity,
                held: 0,
                granted: 0,
                order: VecDeque::new(),
                nodes: HashMap::new(),
                next_id: 0,
            }),
        })
    }

    /// Leases `k` units of weight as one indivisible wave.
    ///
    /// The returned future yields a [`WavePermit`] carrying the whole
    /// wave, or a [`WaveError`] on its first poll when `k` is zero or
    /// wider than the gate. Dropping the future before it resolves
    /// cancels the acquisition cleanly at either stage: a waiting
    /// acquisition held nothing to give back, and one already granted
    /// hands its full weight to the next waiter that can use it.
    pub fn acquire_wave(self: &Arc<Self>, k: u32) -> StreamAcquire<'_> {
        StreamAcquire {
            gate: self,
            requested: k,
            id: None,
        }
    }

    /// The gate's fixed total weight.
    #[must_use]
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Weight assigned to nobody. Exposed for tests and diagnostics.
    #[must_use]
    pub fn free(&self) -> u32 {
        self.lock().free
    }

    /// Weight inside live permits. Exposed for tests and diagnostics.
    #[must_use]
    pub fn in_flight(&self) -> u32 {
        self.lock().held
    }

    /// Weight parked on grants awaiting their repoll. Exposed for tests
    /// and diagnostics.
    #[must_use]
    pub fn granted(&self) -> u32 {
        self.lock().granted
    }

    /// Ids retained in the waiting order. Exposed so tests can assert
    /// on RETAINED STORAGE rather than only on weight, which a
    /// cancellation leak would leave balanced.
    #[must_use]
    pub fn waiting_len(&self) -> usize {
        self.lock().order.len()
    }

    /// Live waiter nodes, waiting and granted. Exposed for the same
    /// retained-storage reason as [`waiting_len`](StreamGate::waiting_len).
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.lock().nodes.len()
    }

    /// Takes the state lock, recovering the state if a holder panicked.
    ///
    /// Gate state is a handful of plain counters and maps; a panic
    /// between two of its mutations is possible in principle but leaves
    /// nothing a later caller can misread beyond what the invariant
    /// already checks, so poisoning is recovered rather than
    /// propagated. Refusing every future acquisition would turn one
    /// unlucky panic into a permanently wedged isolate.
    fn lock(&self) -> MutexGuard<'_, GateInner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Releases `weight` from a dropped permit and hands it onward.
    fn release_held(&self, weight: u32) {
        let wakers = {
            let mut inner = self.lock();
            inner.held = inner.held.saturating_sub(weight);
            inner.free = inner.free.saturating_add(weight);
            let wakers = inner.scan_grants();
            inner.assert_invariant(self.capacity);
            wakers
        };
        // OUTSIDE the lock: a waker is arbitrary caller code and may
        // re-enter the gate.
        wake_all(wakers);
    }
}

/// Fires every collected waker. Called only with the lock released.
fn wake_all(wakers: Vec<Waker>) {
    for waker in wakers {
        waker.wake();
    }
}

/// RAII lease on a whole wave of [`StreamGate`] weight.
///
/// Dropping the permit — normally or while a panic unwinds — returns
/// all `weight` units in one transition and runs the grant scan, which
/// matches how a wave completes: the batch finishes as a unit, so its
/// weight comes back as a unit. The permit owns a handle on its gate
/// rather than borrowing one, so it can be moved into wave-scoped
/// state, and it is not `Clone`, so a wave cannot be released twice.
#[derive(Debug)]
pub struct WavePermit {
    gate: Arc<StreamGate>,
    weight: u32,
}

impl WavePermit {
    /// The number of calls this lease covers.
    #[must_use]
    pub fn weight(&self) -> u32 {
        self.weight
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for WavePermit {
    fn drop(&mut self) {
        self.gate.release_held(self.weight);
    }
}

/// The future returned by [`StreamGate::acquire_wave`].
///
/// It is cancellation-safe at both stages. Dropped while waiting, it
/// removes its node AND its queue id, then rescans — a cancelled
/// too-wide head may have been the only thing blocking waiters behind
/// it, and since no weight moved, nothing else would ever fire that
/// scan. Dropped after it was granted but before it repolled, it
/// returns the parked weight to `free` and rescans, so a winner that
/// walks away cannot strand its wave.
#[derive(Debug)]
pub struct StreamAcquire<'gate> {
    gate: &'gate Arc<StreamGate>,
    requested: u32,
    /// The waiter node's id while one is enqueued. `None` both before
    /// the acquisition queues and after it resolves, which is exactly
    /// when there is no gate state for `Drop` to reclaim.
    id: Option<u64>,
}

impl Future for StreamAcquire<'_> {
    type Output = Result<WavePermit, WaveError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let capacity = this.gate.capacity;

        if this.requested == 0 {
            return Poll::Ready(Err(WaveError::ZeroWave));
        }
        if this.requested > capacity {
            return Poll::Ready(Err(WaveError::WaveExceedsCapacity {
                requested: this.requested,
                capacity,
            }));
        }

        let mut inner = this.gate.lock();

        if let Some(id) = this.id {
            // An enqueued node: either it has won, or its waker needs
            // replacing with this task's.
            let Some(node) = inner.nodes.get_mut(&id) else {
                // Unreachable through the public API: only this future
                // removes its own node, and it clears `id` when it
                // does, so a polled `Some(id)` always resolves.
                debug_assert!(false, "a live acquisition lost its waiter node");
                return Poll::Pending;
            };
            return match node.state {
                WaiterState::Granted => {
                    let requested = node.requested;
                    inner.nodes.remove(&id);
                    inner.granted = inner.granted.saturating_sub(requested);
                    inner.held = inner.held.saturating_add(requested);
                    inner.assert_invariant(capacity);
                    drop(inner);
                    this.id = None;
                    Poll::Ready(Ok(WavePermit {
                        gate: Arc::clone(this.gate),
                        weight: requested,
                    }))
                }
                WaiterState::Waiting(ref mut stored) => {
                    // Replace in place: same node, same queue position.
                    if !stored.will_wake(cx.waker()) {
                        stored.clone_from(cx.waker());
                    }
                    Poll::Pending
                }
            };
        }

        // A fresh acquisition. It may take weight only when nobody is
        // ahead of it — a new caller never steals from a queue.
        if inner.order.is_empty() && inner.free >= this.requested {
            inner.free = inner.free.saturating_sub(this.requested);
            inner.held = inner.held.saturating_add(this.requested);
            inner.assert_invariant(capacity);
            drop(inner);
            return Poll::Ready(Ok(WavePermit {
                gate: Arc::clone(this.gate),
                weight: this.requested,
            }));
        }

        let id = inner.next_id;
        inner.next_id = inner.next_id.saturating_add(1);
        inner.nodes.insert(
            id,
            Waiter {
                requested: this.requested,
                state: WaiterState::Waiting(cx.waker().clone()),
            },
        );
        inner.order.push_back(id);
        inner.assert_invariant(capacity);
        drop(inner);
        this.id = Some(id);
        Poll::Pending
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for StreamAcquire<'_> {
    fn drop(&mut self) {
        let Some(id) = self.id else {
            return;
        };
        let capacity = self.gate.capacity;
        let wakers = {
            let mut inner = self.gate.lock();
            let Some(node) = inner.nodes.remove(&id) else {
                return;
            };
            match node.state {
                WaiterState::Waiting(_) => {
                    // Reclaim the queue entry NOW. Deferring it to the
                    // scan would retain every id behind a blocked head
                    // forever, which the weight invariant cannot see.
                    inner.forget_waiting(id);
                }
                WaiterState::Granted => {
                    inner.granted = inner.granted.saturating_sub(node.requested);
                    inner.free = inner.free.saturating_add(node.requested);
                }
            }
            // BOTH arms rescan. The waiting arm moved no weight, so this
            // is the only chance a satisfiable successor gets.
            let wakers = inner.scan_grants();
            inner.assert_invariant(capacity);
            wakers
        };
        wake_all(wakers);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use crate::alloc_track;

    #[test]
    fn admission_permit_releases_on_drop_including_panic() {
        let gate = AdmissionGate::new(2);

        let permit = gate.try_acquire().expect("capacity 2 admits the first");
        assert_eq!(gate.in_use(), 1);
        drop(permit);
        assert_eq!(gate.in_use(), 0, "normal drop releases the slot");

        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let _permit = gate.try_acquire().expect("gate is empty again");
            assert_eq!(gate.in_use(), 1);
            panic!("unwind with a live permit");
        }));
        assert!(outcome.is_err(), "the closure is expected to panic");
        assert_eq!(gate.in_use(), 0, "unwinding drop releases the slot");

        // The slot is genuinely reusable, and release happened exactly
        // once — a double release would have wrapped the counter.
        let first = gate.try_acquire().expect("slot 1 free after unwind");
        let second = gate.try_acquire().expect("slot 2 free");
        assert_eq!(gate.in_use(), 2);
        assert!(gate.try_acquire().is_none(), "gate is now full");
        drop((first, second));
        assert_eq!(gate.in_use(), 0);
    }

    #[test]
    fn admission_full_returns_none_without_allocating() {
        let gate = AdmissionGate::new(1);
        let held = gate.try_acquire().expect("capacity 1 admits the first");

        // The allocator is process-global, so measure per-thread and
        // across only the rejected call.
        let before = alloc_track::allocated_on_this_thread();
        let rejected = gate.try_acquire();
        let allocated = alloc_track::allocated_on_this_thread().saturating_sub(before);

        assert!(rejected.is_none(), "a full gate rejects rather than queues");
        assert_eq!(
            allocated, 0,
            "rejection allocated {allocated} bytes; the full-gate path must not allocate"
        );

        drop(held);
        assert_eq!(gate.in_use(), 0);
    }
}

#[cfg(test)]
mod stream_gate_tests {
    use super::*;
    use std::iter;
    use std::pin::Pin;
    use std::sync::Mutex as StdMutex;
    use std::sync::atomic::AtomicUsize;
    use std::task::{Context, Poll, Wake, Waker};

    /// Waker that counts how often it was woken.
    #[derive(Debug, Default)]
    struct CountingWaker {
        woken: AtomicUsize,
    }

    impl CountingWaker {
        fn new() -> Arc<Self> {
            Arc::new(CountingWaker::default())
        }

        fn count(&self) -> usize {
            self.woken.load(Ordering::Acquire)
        }
    }

    impl Wake for CountingWaker {
        fn wake(self: Arc<Self>) {
            self.wake_by_ref();
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.woken.fetch_add(1, Ordering::AcqRel);
        }
    }

    /// An in-progress acquisition, owned so a test can hold it, poll it
    /// by hand and drop it at a chosen point without shadowing bindings.
    struct Acquisition<'gate> {
        future: Pin<Box<StreamAcquire<'gate>>>,
    }

    impl<'gate> Acquisition<'gate> {
        fn start(gate: &'gate Arc<StreamGate>, k: u32) -> Self {
            Acquisition {
                future: Box::pin(gate.acquire_wave(k)),
            }
        }

        /// Polls once with a waker built from `counter`.
        fn poll(&mut self, counter: &Arc<CountingWaker>) -> Poll<Result<WavePermit, WaveError>> {
            let waker = Waker::from(Arc::clone(counter));
            let mut cx = Context::from_waker(&waker);
            self.future.as_mut().poll(&mut cx)
        }

        /// Polls once and asserts the acquisition is still queued.
        fn poll_pending(&mut self, counter: &Arc<CountingWaker>, why: &str) {
            assert!(self.poll(counter).is_pending(), "{why}");
        }

        /// Polls once and unwraps the granted permit.
        fn poll_granted(&mut self, counter: &Arc<CountingWaker>) -> WavePermit {
            match self.poll(counter) {
                Poll::Ready(Ok(permit)) => permit,
                Poll::Ready(Err(error)) => panic!("expected a grant, got {error:?}"),
                Poll::Pending => panic!("expected a grant, got Pending"),
            }
        }

        /// Polls once and unwraps the rejection. A permit is an RAII
        /// lease and deliberately not comparable, so the error cases
        /// unwrap before asserting.
        fn poll_rejected(&mut self, counter: &Arc<CountingWaker>) -> WaveError {
            match self.poll(counter) {
                Poll::Ready(Err(error)) => error,
                Poll::Ready(Ok(_)) => panic!("expected a rejection, got a grant"),
                Poll::Pending => panic!("expected a rejection, got Pending"),
            }
        }
    }

    /// Takes a wave immediately, for tests that need the gate occupied.
    fn hold(gate: &Arc<StreamGate>, k: u32, counter: &Arc<CountingWaker>) -> WavePermit {
        Acquisition::start(gate, k).poll_granted(counter)
    }

    #[test]
    fn stream_gate_zero_wave_and_over_capacity_are_typed_errors() {
        let gate = StreamGate::new(4);
        let counter = CountingWaker::new();

        let mut zero = Acquisition::start(&gate, 0);
        assert_eq!(
            zero.poll_rejected(&counter),
            WaveError::ZeroWave,
            "a zero-width wave is a caller bug, reported on the first poll"
        );

        let mut over = Acquisition::start(&gate, 5);
        assert_eq!(
            over.poll_rejected(&counter),
            WaveError::WaveExceedsCapacity {
                requested: 5,
                capacity: 4,
            },
            "waves larger than capacity could never grant; they error rather than hang"
        );

        assert_eq!(counter.count(), 0, "error paths never park a waker");
        assert_eq!(gate.free(), 4, "rejected waves take no weight");
        assert_eq!(gate.waiting_len(), 0, "rejected waves enqueue nothing");
    }

    #[test]
    fn stream_gate_grant_is_atomic_no_partial_hold() {
        let gate = StreamGate::new(4);
        let counter = CountingWaker::new();

        let held = hold(&gate, 3, &counter);
        assert_eq!(gate.free(), 1);

        // 3 of 4 are out, so a 2-wave cannot be satisfied. It must take
        // NOTHING rather than reserve the one free slot.
        let mut waiter = Acquisition::start(&gate, 2);
        waiter.poll_pending(&counter, "an unsatisfiable wave waits");
        assert_eq!(
            gate.free(),
            1,
            "the waiting 2-wave reserved no partial weight"
        );
        assert_eq!(gate.granted(), 0, "nothing was granted to the waiter yet");

        drop(held);
        assert_eq!(counter.count(), 1, "the release woke the satisfiable head");
        let permit = waiter.poll_granted(&counter);
        assert_eq!(gate.free(), 2, "the 2-wave took all 2 at once");
        drop(permit);
        assert_eq!(gate.free(), 4);
    }

    #[test]
    fn stream_gate_mixed_sizes_are_fifo_with_head_of_line_blocking() {
        let gate = StreamGate::new(4);
        let counter = CountingWaker::new();

        let held = hold(&gate, 4, &counter);

        // A big head and a small tail: strict FIFO means the small one
        // does NOT overtake, even once enough weight for it is free.
        let mut big = Acquisition::start(&gate, 3);
        let mut small = Acquisition::start(&gate, 1);
        big.poll_pending(&counter, "the gate is full");
        small.poll_pending(&counter, "the gate is full");

        drop(held);
        // One release ran the scan past the head: the 3-wave took 3 and
        // the remaining 1 satisfied the tail in the same scan, so both
        // are already granted before either future repolls.
        assert_eq!(gate.granted(), 4);
        assert_eq!(gate.free(), 0);
        let big_permit = big.poll_granted(&counter);
        let small_permit = small.poll_granted(&counter);
        assert_eq!(gate.in_flight(), 4, "both waves repolled into live guards");
        assert_eq!(gate.free(), 0);
        drop((big_permit, small_permit));
        assert_eq!(gate.free(), 4);

        // Head-of-line: with only 2 free, a 3-head blocks a 1-tail that
        // could otherwise be served immediately.
        let block = hold(&gate, 2, &counter);
        let mut wide = Acquisition::start(&gate, 3);
        let mut narrow = Acquisition::start(&gate, 1);
        wide.poll_pending(&counter, "only 2 are free, so the 3-wave blocks");
        narrow.poll_pending(
            &counter,
            "the 1-wave is blocked behind the unsatisfiable 3-head, though 2 are free",
        );
        assert_eq!(gate.free(), 2, "the blocked pair holds nothing");
        drop((block, wide, narrow));
    }

    #[test]
    fn stream_gate_cancel_before_grant_releases_nothing() {
        let gate = StreamGate::new(4);
        let counter = CountingWaker::new();

        let held = hold(&gate, 3, &counter);
        assert_eq!(gate.free(), 1);

        let mut waiter = Acquisition::start(&gate, 2);
        waiter.poll_pending(&counter, "3 of 4 are out, so a 2-wave waits");
        assert_eq!(gate.waiting_len(), 1);
        drop(waiter);
        // Cancelling a waiter that never held weight returns nothing and
        // must leave no trace in either store.
        assert_eq!(gate.free(), 1, "a cancelled waiter released no weight");
        assert_eq!(gate.waiting_len(), 0, "its id left the waiting order");
        assert_eq!(gate.node_count(), 0, "its node left the node map");

        drop(held);
        assert_eq!(gate.free(), 4);
    }

    #[test]
    fn stream_gate_cancel_after_grant_releases_all_k() {
        let gate = StreamGate::new(4);
        let counter = CountingWaker::new();

        let held = hold(&gate, 4, &counter);

        let mut waiter = Acquisition::start(&gate, 3);
        waiter.poll_pending(&counter, "the gate is full");

        drop(held);
        // The waiter is now Granted but has NOT been repolled: its 3 are
        // parked on the node.
        assert_eq!(gate.granted(), 3, "the woken waiter parks its weight");
        assert_eq!(gate.free(), 1);
        assert_eq!(counter.count(), 1);
        drop(waiter);
        // Dropping a granted-but-never-repolled acquisition must hand the
        // full weight back, not strand it.
        assert_eq!(gate.granted(), 0, "a cancelled winner strands nothing");
        assert_eq!(gate.free(), 4, "all 3 came back at once");
        assert_eq!(gate.node_count(), 0);
    }

    #[test]
    fn stream_gate_waiting_head_cancel_unblocks_satisfiable_next() {
        let gate = StreamGate::new(8);
        let counter = CountingWaker::new();

        let held = hold(&gate, 4, &counter);
        assert_eq!(gate.free(), 4);

        let next_waker = CountingWaker::new();
        let mut next = Acquisition::start(&gate, 1);
        // The head wants the whole gate; 4 are free, so it blocks, and
        // with it the 1-wave behind it.
        let mut too_wide = Acquisition::start(&gate, 8);
        too_wide.poll_pending(&counter, "8 wanted, 4 free");
        next.poll_pending(&next_waker, "queued behind the too-wide head");
        assert_eq!(next_waker.count(), 0);
        drop(too_wide);
        // No weight moved when the head cancelled, so ONLY a rescan on
        // the cancellation path can ever release the successor.
        assert_eq!(
            next_waker.count(),
            1,
            "cancelling the too-big head rescans and wakes the satisfiable next"
        );
        let permit = next.poll_granted(&next_waker);
        assert_eq!(gate.free(), 3);
        drop((permit, held));
        assert_eq!(gate.free(), 8);
    }

    #[test]
    fn stream_gate_granted_then_dropped_hands_weight_onward() {
        let gate = StreamGate::new(4);
        let counter = CountingWaker::new();

        let held = hold(&gate, 4, &counter);

        let second_waker = CountingWaker::new();
        let mut second = Acquisition::start(&gate, 2);
        let first_waker = CountingWaker::new();
        let mut first = Acquisition::start(&gate, 4);
        first.poll_pending(&first_waker, "the gate is full");
        second.poll_pending(&second_waker, "queued behind the 4-head");

        drop(held);
        assert_eq!(first_waker.count(), 1, "the head is granted first");
        assert_eq!(
            second_waker.count(),
            0,
            "nothing is left for the second while the head holds all 4"
        );
        assert_eq!(gate.granted(), 4);
        drop(first);
        // The granted-but-abandoned head's weight flows to the next
        // satisfiable waiter instead of being stranded.
        assert_eq!(
            second_waker.count(),
            1,
            "the abandoned grant's rescan wakes the successor"
        );
        let permit = second.poll_granted(&second_waker);
        assert_eq!(gate.free(), 2);
        drop(permit);
        assert_eq!(gate.free(), 4);
    }

    #[test]
    fn stream_gate_waker_replacement_repolls_same_node() {
        let gate = StreamGate::new(2);
        let first_waker = CountingWaker::new();
        let second_waker = CountingWaker::new();

        let held = hold(&gate, 2, &first_waker);

        let mut waiter = Acquisition::start(&gate, 1);
        waiter.poll_pending(&first_waker, "the gate is full");
        assert_eq!(gate.waiting_len(), 1);

        // Repolling from a different task replaces the stored waker in
        // place: one node, one queue entry, no requeue.
        waiter.poll_pending(&second_waker, "still queued after the repoll");
        assert_eq!(
            gate.waiting_len(),
            1,
            "repolling did not enqueue a second id"
        );
        assert_eq!(
            gate.node_count(),
            1,
            "repolling did not create a second node"
        );

        drop(held);
        assert_eq!(second_waker.count(), 1, "the latest waker is the one woken");
        let permit = waiter.poll_granted(&second_waker);
        drop(permit);
        assert_eq!(gate.free(), 2);
    }

    #[test]
    fn stream_gate_notifies_only_the_latest_waker_exactly_once() {
        let gate = StreamGate::new(1);
        let waker_a = CountingWaker::new();
        let waker_b = CountingWaker::new();

        let held = hold(&gate, 1, &waker_a);

        let mut waiter = Acquisition::start(&gate, 1);
        waiter.poll_pending(&waker_a, "the gate is full");
        waiter.poll_pending(&waker_b, "still queued after the repoll");

        drop(held);
        assert_eq!(
            waker_b.count(),
            1,
            "the replacement waker is woken exactly once, not zero or twice"
        );
        assert_eq!(
            waker_a.count(),
            0,
            "the replaced waker is never woken; waking it would lose the wakeup"
        );
    }

    #[test]
    fn stream_gate_wakes_outside_the_lock_reentrant_waker() {
        /// A waker whose `wake` re-enters the gate. If the gate woke
        /// waiters while holding its mutex, this would deadlock (or
        /// poison the mutex on a re-entrant lock panic).
        struct ReentrantWaker {
            gate: Arc<StreamGate>,
            reentered: Arc<AtomicUsize>,
            observed_free: Arc<StdMutex<Vec<u32>>>,
        }

        impl Wake for ReentrantWaker {
            fn wake(self: Arc<Self>) {
                self.wake_by_ref();
            }

            fn wake_by_ref(self: &Arc<Self>) {
                self.reentered.fetch_add(1, Ordering::AcqRel);
                // Re-enter: a second acquisition polled from inside the
                // wake callback needs the gate's lock to be free.
                let observed = self.gate.free();
                let mut second = Box::pin(self.gate.acquire_wave(1));
                let noop = Waker::noop();
                let mut cx = Context::from_waker(noop);
                let outcome = second.as_mut().poll(&mut cx);
                // Drop the acquisition (granted or waiting) right here,
                // which takes the lock a second time.
                drop(outcome);
                drop(second);
                if let Ok(mut log) = self.observed_free.lock() {
                    log.push(observed);
                }
            }
        }

        let gate = StreamGate::new(2);
        let counter = CountingWaker::new();
        let reentered = Arc::new(AtomicUsize::new(0));
        let observed_free = Arc::new(StdMutex::new(Vec::new()));

        let held = hold(&gate, 2, &counter);

        let reentrant = Waker::from(Arc::new(ReentrantWaker {
            gate: Arc::clone(&gate),
            reentered: Arc::clone(&reentered),
            observed_free: Arc::clone(&observed_free),
        }));
        let mut waiter = Acquisition::start(&gate, 2);
        let mut reentrant_cx = Context::from_waker(&reentrant);
        assert!(
            waiter.future.as_mut().poll(&mut reentrant_cx).is_pending(),
            "the gate is full, so the 2-wave queues under the re-entrant waker"
        );

        // This release wakes the re-entrant waker. Reaching the next line
        // at all is the assertion: a wake under the lock never returns.
        drop(held);
        assert_eq!(reentered.load(Ordering::Acquire), 1, "the waker ran once");
        assert_eq!(
            observed_free
                .lock()
                .expect("waker thread did not panic")
                .as_slice(),
            &[0],
            "the re-entrant read saw consistent state: all 2 parked on the grant"
        );

        let permit = waiter.poll_granted(&counter);
        drop(permit);
        assert_eq!(gate.free(), 2);
        assert_eq!(gate.waiting_len(), 0);
        assert_eq!(gate.node_count(), 0);
    }

    #[test]
    fn stream_gate_bounds_concurrent_live_bodies() {
        let gate = StreamGate::new(8);
        let counter = CountingWaker::new();

        // 100 single-permit waves, each a distinct acquisition, driven
        // to completion by hand. No more than 8 bodies are ever live.
        let mut acquisitions: Vec<Acquisition<'_>> =
            iter::repeat_with(|| Acquisition::start(&gate, 1))
                .take(100)
                .collect();
        let mut permits: Vec<Option<WavePermit>> = Vec::new();
        permits.resize_with(100, || None);
        let mut results: Vec<Option<usize>> = vec![None; 100];
        let mut completed = 0_usize;
        let mut high_water = 0_u32;

        let mut rounds = 0_usize;
        while completed < 100 {
            rounds = rounds.saturating_add(1);
            assert!(rounds <= 1000, "the drive loop must converge");

            for (index, acquisition) in acquisitions.iter_mut().enumerate() {
                if results[index].is_some() {
                    continue;
                }
                if let Poll::Ready(outcome) = acquisition.poll(&counter) {
                    let permit = outcome.expect("a 1-wave on a capacity-8 gate is valid");
                    permits[index] = Some(permit);
                    results[index] = Some(index);
                    completed = completed.saturating_add(1);
                    high_water = high_water.max(gate.in_flight());
                }
            }

            // Retire everything currently held so the next batch can run.
            for slot in &mut permits {
                *slot = None;
            }
        }

        assert!(
            high_water <= 8,
            "concurrent live bodies peaked at {high_water}, above the capacity-8 bound"
        );
        assert!(high_water > 0, "the gate did admit work");
        let positions: Vec<usize> = results.iter().map(|r| r.expect("all completed")).collect();
        let expected: Vec<usize> = (0..100).collect();
        assert_eq!(
            positions, expected,
            "each acquisition's result stayed at its own position"
        );
        assert_eq!(gate.free(), 8, "every wave released");
        assert_eq!(gate.waiting_len(), 0);
        assert_eq!(gate.node_count(), 0);
    }

    #[test]
    fn cancellation_churn_behind_blocked_head_does_not_grow_storage() {
        let gate = StreamGate::new(8);
        let counter = CountingWaker::new();

        let held = hold(&gate, 8, &counter);

        let mut blocker = Acquisition::start(&gate, 1);
        blocker.poll_pending(&counter, "the gate is full");

        // 10_000 enqueue/cancel pairs behind a head that never grants.
        // Cancellation must reclaim the queue entry immediately; a scan
        // that only pops stale ids off the FRONT never reaches these.
        for _ in 0..10_000_u32 {
            let mut churn = Acquisition::start(&gate, 1);
            churn.poll_pending(&counter, "queued behind a head that never grants");
        }

        assert_eq!(
            gate.waiting_len(),
            1,
            "retained waiting-order storage must track LIVE waiters, not lifetime arrivals"
        );
        assert_eq!(gate.node_count(), 1, "retained node storage likewise");

        drop(held);
        let permit = blocker.poll_granted(&counter);
        drop(permit);
        assert_eq!(gate.free(), 8);
    }
}
