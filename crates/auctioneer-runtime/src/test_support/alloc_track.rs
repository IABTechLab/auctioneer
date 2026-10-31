//! Thread-local live/peak allocation measurement, isolated from other tests.
#![expect(unsafe_code, reason = "test-only System allocator delegate")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static LIVE: Cell<i64> = const { Cell::new(0) };
    static PEAK: Cell<i64> = const { Cell::new(0) };
    static ALLOCATED: Cell<u64> = const { Cell::new(0) };
}

/// Cumulative requested bytes on the calling thread, without deallocation refunds.
#[must_use]
pub fn allocated() -> u64 {
    ALLOCATED.try_with(Cell::get).unwrap_or(0)
}

/// Net requested bytes allocated and released on the calling thread.
#[must_use]
pub fn live() -> i64 {
    LIVE.try_with(Cell::get).unwrap_or(0)
}

/// Start a peak window at the calling thread's current live byte count.
#[must_use]
pub fn reset_peak() -> i64 {
    let baseline = live();
    PEAK.with(|peak| peak.set(baseline));
    baseline
}

/// Peak requested-byte growth above a baseline returned by [`reset_peak`].
///
/// # Panics
/// Panics if the baseline exceeds the tracked peak or thread-local storage is unavailable.
#[must_use]
#[expect(
    clippy::unwrap_used,
    reason = "an invalid measurement baseline must fail rather than understate allocation evidence"
)]
pub fn peak_since(baseline: i64) -> u64 {
    u64::try_from(PEAK.with(Cell::get).saturating_sub(baseline)).unwrap()
}

fn change(bytes: i64) {
    if let Ok(requested) = u64::try_from(bytes) {
        let _allocated =
            ALLOCATED.try_with(|total| total.set(total.get().saturating_add(requested)));
    }
    let _outcome = LIVE.try_with(|live| {
        live.set(live.get().saturating_add(bytes));
        let _peak = PEAK.try_with(|peak| peak.set(peak.get().max(live.get())));
    });
}

pub struct CountingAllocator;

// SAFETY: all memory operations delegate unchanged to System; counters are TLS only.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        change(i64::try_from(layout.size()).unwrap_or(i64::MAX));
        // SAFETY: forwards the caller's layout unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        change(
            i64::try_from(layout.size())
                .unwrap_or(i64::MAX)
                .saturating_neg(),
        );
        // SAFETY: ptr/layout originate from a matching allocation.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        change(i64::try_from(layout.size()).unwrap_or(i64::MAX));
        // SAFETY: forwards the caller's layout unchanged.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        change(i64::try_from(new_size).unwrap_or(i64::MAX));
        change(
            i64::try_from(layout.size())
                .unwrap_or(i64::MAX)
                .saturating_neg(),
        );
        // SAFETY: forwards the caller's valid allocation and new size.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}
