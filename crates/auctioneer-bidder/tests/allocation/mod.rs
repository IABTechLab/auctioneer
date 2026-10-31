//! Thread-local allocation measurements and production-independent construction bounds.

#![cfg(test)]
#![expect(
    unsafe_code,
    reason = "test-only GlobalAlloc instrumentation delegates every operation to System"
)]

use auctioneer_bidder::{
    BidderRequestInfo, EndpointRejectReason, EndpointValidator, outbound_meta_bytes,
};
use auctioneer_runtime::budget::OUTBOUND_HEADER_MAP_CHARGE;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static LIVE: Cell<i64> = const { Cell::new(0) };
    static PEAK: Cell<i64> = const { Cell::new(0) };
    static ALLOCATED: Cell<usize> = const { Cell::new(0) };
    static LARGEST: Cell<usize> = const { Cell::new(0) };
}

#[derive(Debug)]
pub(crate) struct Measurement {
    peak: usize,
    allocated: usize,
    largest: usize,
}

impl Measurement {
    #[must_use]
    pub(crate) const fn peak(&self) -> usize {
        self.peak
    }

    #[must_use]
    pub(crate) const fn allocated(&self) -> usize {
        self.allocated
    }

    #[must_use]
    pub(crate) const fn largest(&self) -> usize {
        self.largest
    }
}

pub(crate) fn start() {
    LIVE.with(|cell| cell.set(0));
    PEAK.with(|cell| cell.set(0));
    ALLOCATED.with(|cell| cell.set(0));
    LARGEST.with(|cell| cell.set(0));
    ENABLED.with(|cell| cell.set(true));
}

pub(crate) fn finish() -> Measurement {
    ENABLED.with(|cell| cell.set(false));
    Measurement {
        peak: PEAK.with(|cell| usize::try_from(cell.get()).unwrap_or(usize::MAX)),
        allocated: ALLOCATED.with(Cell::get),
        largest: LARGEST.with(Cell::get),
    }
}

fn add(bytes: usize) {
    response::add(bytes);
    if !ENABLED.try_with(Cell::get).unwrap_or(false) {
        return;
    }
    let _live = LIVE.try_with(|cell| {
        let now = cell
            .get()
            .saturating_add(i64::try_from(bytes).unwrap_or(i64::MAX));
        cell.set(now);
        let _peak = PEAK.try_with(|peak| peak.set(peak.get().max(now)));
    });
    let _allocated = ALLOCATED.try_with(|cell| cell.set(cell.get().saturating_add(bytes)));
    let _largest = LARGEST.try_with(|cell| cell.set(cell.get().max(bytes)));
}

fn sub(bytes: usize) {
    response::sub(bytes);
    if ENABLED.try_with(Cell::get).unwrap_or(false) {
        let _live = LIVE.try_with(|cell| {
            cell.set(
                cell.get()
                    .saturating_sub(i64::try_from(bytes).unwrap_or(i64::MAX)),
            );
        });
    }
}

pub(crate) struct CountingAllocator;

// SAFETY: all memory operations delegate unchanged to System; counters are thread-local.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies the GlobalAlloc layout contract.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            add(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies the GlobalAlloc layout contract.
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            add(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        sub(layout.size());
        // SAFETY: the pointer and layout come from a matching System allocation.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller supplies a matching allocation and valid new size.
        let result = unsafe { System.realloc(ptr, layout, new_size) };
        if !result.is_null() {
            add(new_size);
            sub(layout.size());
        }
        result
    }
}

// The pool and offsets use the production metadata charge. A separate
// 2 KiB ceiling covers this one-call fixture's URL expansion/validation
// and small accepted-result containers, not input-dependent JSON trees.
// That fixed control work fits the already-funded outbound map charge;
// the transport map itself does not exist during this measurement.
pub(crate) const CONSTRUCTION_CONTROL_BYTES: usize = 2 * 1024;

pub(crate) struct AllowAllEndpoints;

impl EndpointValidator for AllowAllEndpoints {
    fn validate(
        &self,
        _url: &url::Url,
        _info: &BidderRequestInfo<'_>,
    ) -> Result<(), EndpointRejectReason> {
        Ok(())
    }
}

pub(crate) fn construction_budget(cap: usize) -> usize {
    assert!(
        u64::try_from(CONSTRUCTION_CONTROL_BYTES).expect("allowance fits u64")
            <= OUTBOUND_HEADER_MAP_CHARGE,
        "fixed control allowance must fit existing funding",
    );
    cap.checked_add(usize::try_from(outbound_meta_bytes(1)).expect("metadata charge fits usize"))
        .and_then(|bytes| bytes.checked_add(CONSTRUCTION_CONTROL_BYTES))
        .expect("fixture construction budget fits usize")
}

pub(crate) fn assert_bounded(measurement: &Measurement, cap: usize) {
    let budget = construction_budget(cap);
    assert!(
        measurement.peak() <= budget
            && measurement.allocated() <= budget
            && measurement.largest() <= budget,
        "bidder construction exceeded {budget} B: {cap}-byte body cap, \
         funded metadata and {CONSTRUCTION_CONTROL_BYTES}-byte fixed control: {measurement:?}"
    );
}

pub(crate) fn size_growth_is_bounded(small: &Measurement, large: &Measurement) -> bool {
    large.peak() <= small.peak().saturating_add(CONSTRUCTION_CONTROL_BYTES)
        && large.allocated() <= small.allocated().saturating_add(CONSTRUCTION_CONTROL_BYTES)
        && large.largest() <= small.largest().saturating_add(CONSTRUCTION_CONTROL_BYTES)
}

/// Continuous snapshots preserve response windows whose inputs already own allocations.
pub(crate) mod response {
    use std::cell::Cell;
    use std::{hint::black_box, thread};

    thread_local! {
        static LIVE: Cell<i64> = const { Cell::new(0) };
        static PEAK: Cell<i64> = const { Cell::new(0) };
    }

    pub(crate) fn live() -> i64 {
        LIVE.try_with(Cell::get).unwrap_or(0)
    }

    pub(crate) fn reset_peak() -> i64 {
        let baseline = live();
        PEAK.with(|peak| peak.set(baseline));
        baseline
    }

    pub(crate) fn peak_since(baseline: i64) -> u64 {
        u64::try_from(PEAK.with(Cell::get).saturating_sub(baseline)).unwrap()
    }

    pub(super) fn add(bytes: usize) {
        change(i64::try_from(bytes).unwrap_or(i64::MAX));
    }

    pub(super) fn sub(bytes: usize) {
        change(i64::try_from(bytes).unwrap_or(i64::MAX).saturating_neg());
    }

    fn change(bytes: i64) {
        let _live = LIVE.try_with(|live| {
            live.set(live.get().saturating_add(bytes));
            let _peak = PEAK.try_with(|peak| peak.set(peak.get().max(live.get())));
        });
    }

    #[test]
    fn response_snapshots_keep_preexisting_owners_outside_the_peak_window() {
        thread::spawn(|| {
            let before = live();
            let held = black_box(Vec::<u8>::with_capacity(8192));
            let baseline = reset_peak();
            assert_eq!(baseline - before, 8192);
            assert_eq!(peak_since(baseline), 0);
            let added = black_box(Vec::<u8>::with_capacity(4096));
            assert_eq!(peak_since(baseline), 4096);
            drop(added);
            assert_eq!(live(), baseline);
            assert_eq!(peak_since(baseline), 4096);
            drop(held);
        })
        .join()
        .unwrap();
    }
}
