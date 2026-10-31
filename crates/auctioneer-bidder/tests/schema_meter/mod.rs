//! Native requested-heap observations, not work, RSS, or WASM qualification.

#![expect(
    unsafe_code,
    reason = "test-only GlobalAlloc instrumentation delegates every operation to System"
)]

use serde_json::{Value, json};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

#[derive(Clone, Copy, Default)]
pub(crate) struct Metrics {
    allocated: u64,
    peak: i64,
    live: i64,
    calls: u64,
    overflow: bool,
}

impl Metrics {
    pub(crate) fn json(self) -> Value {
        assert!(!self.overflow, "allocation counter overflow");
        json!({"allocated_bytes": self.allocated, "peak_live_bytes": self.peak,
            "retained_bytes": self.live, "allocation_calls": self.calls})
    }

    pub(crate) fn append(&mut self, next: Self) {
        assert!(!next.overflow);
        self.peak = self.peak.max(self.live.checked_add(next.peak).unwrap());
        self.live = self.live.checked_add(next.live).unwrap();
        self.allocated = self.allocated.checked_add(next.allocated).unwrap();
        self.calls = self.calls.checked_add(next.calls).unwrap();
    }
}

thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static COUNTERS: Cell<Metrics> = const { Cell::new(Metrics {
        allocated: 0, peak: 0, live: 0, calls: 0, overflow: false,
    }) };
}

fn change(added: usize, removed: usize, allocation: bool) {
    if !ENABLED.try_with(Cell::get).unwrap_or(false) {
        return;
    }
    let _result = COUNTERS.try_with(|cell| {
        let mut counters = cell.get();
        let updated = || {
            let live = counters
                .live
                .checked_sub(i64::try_from(removed).ok()?)?
                .checked_add(i64::try_from(added).ok()?)?;
            Some((
                live,
                counters.allocated.checked_add(u64::try_from(added).ok()?)?,
                counters.calls.checked_add(u64::from(allocation))?,
            ))
        };
        if let Some((live, allocated, calls)) = updated() {
            counters.live = live;
            counters.peak = counters.peak.max(live);
            counters.allocated = allocated;
            counters.calls = calls;
        } else {
            counters.overflow = true;
        }
        cell.set(counters);
    });
}

struct CountingAllocator;

// SAFETY: memory operations delegate unchanged to System; TLS accounting never allocates.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forward the caller's valid layout.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            change(layout.size(), 0, true);
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forward the caller's valid layout.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            change(layout.size(), 0, true);
        }
        pointer
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        change(0, layout.size(), false);
        // SAFETY: ptr/layout identify a matching System allocation.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: forward a matching allocation and the caller's valid new size.
        let result = unsafe { System.realloc(ptr, layout, new_size) };
        if !result.is_null() {
            // Count the full successful request, but replace old logical live
            // bytes rather than inventing an overlapping old/new live block.
            change(new_size, layout.size(), true);
        }
        result
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

struct Stop;
impl Drop for Stop {
    fn drop(&mut self) {
        ENABLED.with(|enabled| enabled.set(false));
    }
}

pub(crate) fn measure<R>(call: impl FnOnce() -> R) -> (R, Metrics) {
    assert!(!ENABLED.with(Cell::get), "nested measurement");
    COUNTERS.with(|counters| counters.set(Metrics::default()));
    ENABLED.with(|enabled| enabled.set(true));
    let stop = Stop;
    let result = call();
    drop(stop);
    let counters = COUNTERS.with(Cell::get);
    assert!(!counters.overflow, "allocation counter overflow");
    (result, counters)
}

#[cfg(test)]
mod tests {
    use super::{Metrics, measure};
    use serde_json::json;
    use std::hint::{black_box, spin_loop};
    use std::panic::{catch_unwind, panic_any};
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::thread;

    #[test]
    fn successful_allocations_and_reallocations_have_exact_metrics() {
        let ((), metrics) = measure(|| {
            let mut first = black_box(vec![black_box(7_u8); black_box(37)]);
            let zeroed = black_box(vec![0_u8; black_box(19)]);
            first.reserve_exact(black_box(64));
            black_box(&first);
            first.truncate(black_box(23));
            first.shrink_to_fit();
            black_box(&first);
            drop(black_box(first));
            drop(black_box(zeroed));
        });

        assert_eq!(
            metrics.json(),
            json!({"allocated_bytes": 180_u64, "peak_live_bytes": 120_i64,
                "retained_bytes": 0_i64, "allocation_calls": 4_u64})
        );
    }

    #[test]
    fn retained_allocation_and_release_have_exact_metrics() {
        let (retained, allocation) =
            measure(|| black_box(vec![black_box(7_u8); black_box(113)].into_boxed_slice()));
        let ((), release) = measure(|| drop(black_box(retained)));

        assert_eq!(
            allocation.json(),
            json!({"allocated_bytes": 113_u64, "peak_live_bytes": 113_i64,
                "retained_bytes": 113_i64, "allocation_calls": 1_u64})
        );
        assert_eq!(
            release.json(),
            json!({"allocated_bytes": 0_u64, "peak_live_bytes": 0_i64,
                "retained_bytes": -113_i64, "allocation_calls": 0_u64})
        );
    }

    #[test]
    fn append_composes_independent_literal_metrics() {
        let mut total = Metrics {
            allocated: 37,
            peak: 37,
            live: 37,
            calls: 1,
            overflow: false,
        };
        total.append(Metrics {
            allocated: 19,
            peak: 19,
            live: 19,
            calls: 1,
            overflow: false,
        });
        assert_eq!(
            total.json(),
            json!({"allocated_bytes": 56_u64, "peak_live_bytes": 56_i64,
                "retained_bytes": 56_i64, "allocation_calls": 2_u64})
        );

        total.append(Metrics {
            allocated: 0,
            peak: 0,
            live: -37,
            calls: 0,
            overflow: false,
        });
        assert_eq!(
            total.json(),
            json!({"allocated_bytes": 56_u64, "peak_live_bytes": 56_i64,
                "retained_bytes": 19_i64, "allocation_calls": 2_u64})
        );

        total.append(Metrics {
            allocated: 101,
            peak: 101,
            live: 0,
            calls: 1,
            overflow: false,
        });
        assert_eq!(
            total.json(),
            json!({"allocated_bytes": 157_u64, "peak_live_bytes": 120_i64,
                "retained_bytes": 19_i64, "allocation_calls": 3_u64})
        );

        total.append(Metrics {
            allocated: 0,
            peak: 0,
            live: -19,
            calls: 0,
            overflow: false,
        });
        assert_eq!(
            total.json(),
            json!({"allocated_bytes": 157_u64, "peak_live_bytes": 120_i64,
                "retained_bytes": 0_i64, "allocation_calls": 3_u64})
        );
    }

    #[test]
    fn panic_cleanup_disables_and_resets_measurement() {
        let result = catch_unwind(|| {
            measure(|| panic_any(71_u8));
        });
        let Err(payload) = result else {
            panic!("measurement did not panic");
        };
        assert_eq!(payload.downcast_ref::<u8>(), Some(&71));
        drop(payload);

        let (retained, metrics) =
            measure(|| black_box(vec![black_box(7_u8); black_box(37)].into_boxed_slice()));
        drop(retained);
        assert_eq!(
            metrics.json(),
            json!({"allocated_bytes": 37_u64, "peak_live_bytes": 37_i64,
                "retained_bytes": 37_i64, "allocation_calls": 1_u64})
        );
    }

    #[test]
    fn other_thread_allocations_are_excluded() {
        let stage = AtomicU8::new(0);
        let ((), metrics) = thread::scope(|scope| {
            let worker = scope.spawn(|| {
                while stage.load(Ordering::Acquire) != 1 {
                    spin_loop();
                }
                drop(black_box(vec![black_box(5_u8); black_box(1 << 20_i32)]));
                stage.store(2, Ordering::Release);
            });
            let result = measure(|| {
                stage.store(1, Ordering::Release);
                let local = black_box(vec![black_box(7_u8); black_box(37)]);
                while stage.load(Ordering::Acquire) != 2 {
                    spin_loop();
                }
                drop(black_box(local));
            });
            worker.join().unwrap();
            result
        });

        assert_eq!(
            metrics.json(),
            json!({"allocated_bytes": 37_u64, "peak_live_bytes": 37_i64,
                "retained_bytes": 0_i64, "allocation_calls": 1_u64})
        );
    }

    #[test]
    fn nested_measurement_is_rejected() {
        let result = catch_unwind(|| {
            measure(|| measure(|| ()));
        });
        let Err(payload) = result else {
            panic!("nested measurement was accepted");
        };
        assert_eq!(payload.downcast_ref::<&str>(), Some(&"nested measurement"));
        drop(payload);

        let (retained, metrics) =
            measure(|| black_box(vec![black_box(7_u8); black_box(37)].into_boxed_slice()));
        drop(retained);
        assert_eq!(
            metrics.json(),
            json!({"allocated_bytes": 37_u64, "peak_live_bytes": 37_i64,
                "retained_bytes": 37_i64, "allocation_calls": 1_u64})
        );
    }
}
