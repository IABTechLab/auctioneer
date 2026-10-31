//! Test-only allocation counting, shared by every test in this crate.
//!
//! Several gates assert that a REFUSED acquisition allocates nothing —
//! that is the shed path under load, so an allocation there is a real
//! regression. Measuring it needs a `#[global_allocator]`, and a binary
//! may define exactly ONE. The crate's unit tests all compile into a
//! single lib-test binary, so the allocator cannot live next to the
//! tests that use it: two gate modules each defining their own would be
//! a hard compile error. It lives here, once, and the gate tests call
//! [`allocated_on_this_thread`].
//!
//! The counter is a THREAD-LOCAL cumulative byte total, not a global
//! one, because `cargo test` runs test functions concurrently: a global
//! counter would attribute a neighbour's allocations to whichever test
//! happened to be measuring. Callers therefore read the counter before
//! and after the single call under test and compare the difference on
//! their own thread.
//!
//! [`measure_on_this_thread`] separately enables a same-thread live/peak
//! window. It does not change the cumulative counter or track ownership
//! transferred between threads or across measurement windows.

#![expect(
    unsafe_code,
    reason = "a GlobalAlloc impl is unavoidably unsafe; test-only \
              measurement backing the gates' full-gate no-allocation bounds"
)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static ALLOCATED: Cell<u64> = const { Cell::new(0) };
    static MEASUREMENT: Cell<Option<Measurement>> = const { Cell::new(None) };
}

#[derive(Clone, Copy)]
struct Measurement {
    live: u64,
    peak: u64,
    valid: bool,
}

struct MeasurementGuard;

impl Drop for MeasurementGuard {
    fn drop(&mut self) {
        // Disable even if the measured operation unwinds; the allocator itself
        // must never panic if a thread-local has already been torn down.
        let _outcome = MEASUREMENT.try_with(|cell| cell.set(None));
    }
}

/// Cumulative bytes allocated on the CALLING thread since it started.
///
/// Meaningful only as a difference across the call under test.
pub fn allocated_on_this_thread() -> u64 {
    ALLOCATED.try_with(Cell::get).unwrap_or(0)
}

/// Requested heap bytes within one explicitly enabled same-thread operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllocationStats {
    /// Maximum live requested bytes, conservatively overlapping old and new
    /// storage during every successful realloc, including in-place reallocs.
    pub peak_bytes: u64,
    /// Requested bytes still live when the operation returns.
    pub retained_bytes: u64,
}

/// Measure one same-thread allocation window without resetting the cumulative API.
///
/// Allocations must originate within this operation and be freed within it or
/// retained in its returned value. Do not free or reallocate pre-existing
/// allocations, or transfer allocation ownership across threads. This measures
/// requested heap sizes, not allocator metadata, rounding, stack or process RSS.
///
/// Returns `None` without running the operation for an unavailable or nested
/// window, or after the operation if counters overflow or underflow. Such a
/// window must not be treated as evidence of a memory bound.
pub fn measure_on_this_thread<T>(operation: impl FnOnce() -> T) -> Option<(T, AllocationStats)> {
    let started = MEASUREMENT
        .try_with(|cell| {
            if cell.get().is_some() {
                return false;
            }
            cell.set(Some(Measurement {
                live: 0,
                peak: 0,
                valid: true,
            }));
            true
        })
        .unwrap_or(false);
    if !started {
        return None;
    }

    let guard = MeasurementGuard;
    let result = operation();
    let snapshot = MEASUREMENT.try_with(Cell::get).ok().flatten();
    drop(guard);
    let measured = snapshot.filter(|state| state.valid)?;
    Some((
        result,
        AllocationStats {
            peak_bytes: measured.peak,
            retained_bytes: measured.live,
        },
    ))
}

fn record_change(allocated: usize, released: usize) {
    let _outcome = MEASUREMENT.try_with(|cell| {
        let Some(mut measurement) = cell.get() else {
            return;
        };
        if !measurement.valid {
            return;
        }

        let grown = u64::try_from(allocated)
            .ok()
            .and_then(|bytes| measurement.live.checked_add(bytes));
        if let Some(live) = grown {
            // Charge the complete new allocation before releasing old storage,
            // even when System realloc kept the original address.
            measurement.peak = measurement.peak.max(live);
            if let Some(retained) = u64::try_from(released)
                .ok()
                .and_then(|bytes| live.checked_sub(bytes))
            {
                measurement.live = retained;
            } else {
                measurement.valid = false;
            }
        } else {
            measurement.valid = false;
        }
        cell.set(Some(measurement));
    });
}

fn count(bytes: usize) {
    // TLS teardown can make try_with fail; a lost count only
    // relaxes the measurement, never breaks the allocator.
    let _outcome = ALLOCATED.try_with(|cell| {
        cell.set(
            cell.get()
                .saturating_add(u64::try_from(bytes).unwrap_or(u64::MAX)),
        );
    });
}

/// A `System` delegate that counts allocated bytes per thread.
pub struct CountingAllocator;

// SAFETY: every operation delegates verbatim to `System`; the
// counting side effect touches only a thread-local counter and
// never the returned memory.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        // SAFETY: same layout contract as the caller's.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record_change(layout.size(), 0);
        }
        pointer
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        record_change(0, layout.size());
        // SAFETY: ptr/layout come from a matching System alloc.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        // SAFETY: same layout contract as the caller's.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record_change(layout.size(), 0);
        }
        pointer
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count(new_size.saturating_sub(layout.size()));
        // SAFETY: ptr/layout come from a matching System alloc;
        // new_size is the caller's requested size.
        let pointer = unsafe { System.realloc(ptr, layout, new_size) };
        if !pointer.is_null() {
            record_change(new_size, layout.size());
        }
        pointer
    }
}

/// The one global allocator for this crate's test binary.
#[global_allocator]
static COUNTING_ALLOC: CountingAllocator = CountingAllocator;

#[cfg(test)]
mod tests {
    use super::{AllocationStats, allocated_on_this_thread, measure_on_this_thread};
    use std::hint::black_box;

    #[test]
    fn measurement_reports_positive_allocation_and_retained_requested_bytes() {
        let before = allocated_on_this_thread();
        let (buffer, stats) = measure_on_this_thread(|| black_box(Vec::<u8>::with_capacity(64)))
            .expect("an ordinary measurement window must be available");
        let cumulative = allocated_on_this_thread().checked_sub(before).unwrap();
        assert_eq!(buffer.capacity(), 64);
        assert_eq!(
            stats,
            AllocationStats {
                peak_bytes: 64,
                retained_bytes: 64,
            }
        );
        assert_eq!(cumulative, 64);
        drop(buffer);
    }

    #[test]
    fn measurement_counts_zeroed_allocations() {
        let (buffer, stats) = measure_on_this_thread(|| black_box(vec![0_u8; 96]))
            .expect("a zeroed allocation must be measured");
        assert_eq!(buffer.len(), 96);
        assert!(buffer.iter().all(|byte| *byte == 0));
        assert_eq!(
            stats,
            AllocationStats {
                peak_bytes: 96,
                retained_bytes: 96,
            }
        );
    }

    #[test]
    fn measurement_subtracts_deallocation_without_erasing_peak() {
        let ((), stats) = measure_on_this_thread(|| {
            let buffer = black_box(Vec::<u8>::with_capacity(64));
            drop(buffer);
        })
        .expect("allocation followed by deallocation must be measured");
        assert_eq!(
            stats,
            AllocationStats {
                peak_bytes: 64,
                retained_bytes: 0,
            }
        );
    }

    #[test]
    fn measurement_peak_is_not_cumulative_across_disjoint_lifetimes() {
        let before = allocated_on_this_thread();
        let (buffer, stats) = measure_on_this_thread(|| {
            drop(black_box(Vec::<u8>::with_capacity(64)));
            black_box(Vec::<u8>::with_capacity(32))
        })
        .expect("disjoint allocation lifetimes must be measured");
        let cumulative = allocated_on_this_thread().checked_sub(before).unwrap();
        assert_eq!(buffer.capacity(), 32);
        assert_eq!(
            stats,
            AllocationStats {
                peak_bytes: 64,
                retained_bytes: 32,
            }
        );
        assert_eq!(cumulative, 96);
    }

    #[test]
    fn measurement_realloc_growth_overlaps_old_and_new_storage() {
        let before = allocated_on_this_thread();
        let (buffer, stats) = measure_on_this_thread(|| {
            let mut buffer = black_box(vec![0_u8; 64]);
            buffer.reserve_exact(32);
            black_box(buffer)
        })
        .expect("a realloc growth must be measured");
        let cumulative = allocated_on_this_thread().checked_sub(before).unwrap();
        assert_eq!(buffer.capacity(), 96);
        assert_eq!(
            stats,
            AllocationStats {
                peak_bytes: 160,
                retained_bytes: 96,
            }
        );
        // Preserve the cumulative API: 64 initial bytes plus 32 growth bytes,
        // not the conservative peak's full old-plus-new overlap.
        assert_eq!(cumulative, 96);
    }

    #[test]
    fn measurement_realloc_shrink_overlaps_old_and_new_storage() {
        let before = allocated_on_this_thread();
        let (buffer, stats) = measure_on_this_thread(|| {
            let mut buffer = black_box(vec![0_u8; 64]);
            buffer.truncate(16);
            buffer.shrink_to_fit();
            black_box(buffer)
        })
        .expect("a realloc shrink must be measured");
        let cumulative = allocated_on_this_thread().checked_sub(before).unwrap();
        assert_eq!(buffer.capacity(), 16);
        assert_eq!(
            stats,
            AllocationStats {
                peak_bytes: 80,
                retained_bytes: 16,
            }
        );
        assert_eq!(cumulative, 64);
    }

    #[test]
    fn measurement_windows_reset_and_disabled_work_does_not_carry_over() {
        let (first, first_stats) =
            measure_on_this_thread(|| black_box(Vec::<u8>::with_capacity(64)))
                .expect("the first measurement window must be available");
        assert_eq!(first_stats.retained_bytes, 64);
        drop(first);
        let outside = black_box(Vec::<u8>::with_capacity(512));
        let ((), empty_stats) = measure_on_this_thread(|| black_box(()))
            .expect("a subsequent empty window must be available");
        assert_eq!(
            empty_stats,
            AllocationStats {
                peak_bytes: 0,
                retained_bytes: 0,
            }
        );
        drop(outside);
    }

    #[test]
    fn measurement_refuses_nested_window_without_running_inner_operation() {
        let ((inner, called), stats) = measure_on_this_thread(|| {
            let mut called = false;
            let inner = measure_on_this_thread(|| {
                called = true;
                black_box(Vec::<u8>::with_capacity(64))
            });
            (inner, called)
        })
        .expect("the outer measurement window must be available");
        assert!(inner.is_none());
        assert!(!called);
        assert_eq!(
            stats,
            AllocationStats {
                peak_bytes: 0,
                retained_bytes: 0,
            }
        );
    }

    #[test]
    fn measurement_rejects_counter_underflow_instead_of_saturating_to_zero() {
        let outside = black_box(Vec::<u8>::with_capacity(64));
        let result = measure_on_this_thread(|| drop(outside));
        assert!(result.is_none());
    }
}
