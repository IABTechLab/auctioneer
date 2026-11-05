//! What a parsed GVL actually retains, measured against what the
//! isolate budget charges for it.
//!
//! The budget bounds resident GVL memory with
//! `(gvl_cache_entries + 1) x gvl_parsed_bytes`. That charge is a claim
//! about `Gvl` — the type this crate owns — so it is measured here,
//! against real vendor lists from the baked archive rather than
//! synthetic fixtures, because the charge has to cover production
//! shapes.

/// Test-scoped counting allocator (per-thread, live-bytes) backing the
/// retained-GVL charge. Delegates every operation to `System`.
mod alloc_track {
    #![expect(
        unsafe_code,
        reason = "a GlobalAlloc impl is unavoidably unsafe; test-only \
                  measurement backing the parsed-GVL memory charge"
    )]

    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static LIVE: Cell<i64> = const { Cell::new(0) };
    }

    /// Bytes currently held by this thread's live allocations. The
    /// difference across an operation is what the operation RETAINED,
    /// which is what the GVL charge bounds — a cumulative counter would
    /// also count the validator's transient scratch, which the parsed
    /// value does not keep.
    pub fn live_on_this_thread() -> i64 {
        LIVE.try_with(Cell::get).unwrap_or(0)
    }

    fn add(bytes: usize) {
        // TLS teardown can make try_with fail; a lost count only
        // relaxes the measurement, never breaks the allocator.
        let _outcome = LIVE.try_with(|cell| {
            cell.set(
                cell.get()
                    .saturating_add(i64::try_from(bytes).unwrap_or(i64::MAX)),
            );
        });
    }

    fn sub(bytes: usize) {
        let _outcome = LIVE.try_with(|cell| {
            cell.set(
                cell.get()
                    .saturating_sub(i64::try_from(bytes).unwrap_or(i64::MAX)),
            );
        });
    }

    pub struct CountingAllocator;

    // SAFETY: every operation delegates verbatim to `System`; the
    // counting side effect touches only a thread-local counter and
    // never the returned memory.
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            add(layout.size());
            // SAFETY: same layout contract as the caller's.
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            sub(layout.size());
            // SAFETY: ptr/layout come from a matching System alloc.
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            add(layout.size());
            // SAFETY: same layout contract as the caller's.
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            add(new_size);
            sub(layout.size());
            // SAFETY: ptr/layout come from a matching System alloc;
            // new_size is the caller's requested size.
            unsafe { System.realloc(ptr, layout, new_size) }
        }
    }
}

#[global_allocator]
static COUNTING_ALLOC: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
mod tests {
    use std::cmp::Reverse;
    use std::thread;

    use super::alloc_track::live_on_this_thread;
    use auctioneer_privacy::gvl::{GVL_PARSE_CACHE_MAX_ENTRIES, Gvl};
    use auctioneer_runtime::MAX_GVL_PARSED_BYTES;

    /// Every bundled entry as `(spec, list, raw)`.
    fn archive_entries() -> Vec<(u32, u32, &'static [u8])> {
        auctioneer_config_data::gvl_archive()
            .iter()
            .map(|entry| (entry.spec, entry.vendor_list_version, entry.raw))
            .collect()
    }

    /// The largest bundled entry — the worst case the charge covers.
    fn largest_entry() -> (u32, u32, &'static [u8]) {
        archive_entries()
            .into_iter()
            .max_by_key(|&(_spec, _list, raw)| raw.len())
            .expect("the baked archive has entries")
    }

    /// Bytes `build` retained, measured on a THREAD OF ITS OWN.
    ///
    /// The counter is thread-local and a fresh thread starts at zero, so
    /// the reading covers this operation and nothing else — no harness
    /// bookkeeping and no neighbouring test can land inside the window.
    /// The value is dropped only after the reading is taken, so what is
    /// measured is what the value RETAINS rather than what the parse
    /// allocated on its way there.
    fn retained_by<T, F>(build: F) -> u64
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        thread::spawn(move || {
            let before = live_on_this_thread();
            let held = build();
            let after = live_on_this_thread();
            drop(held);
            u64::try_from(after.saturating_sub(before)).unwrap_or(0)
        })
        .join()
        .expect("the measuring thread does not panic")
    }

    /// One parsed `Gvl` fits the per-list charge, at every bundled
    /// version rather than only the largest.
    ///
    /// The archive is the production shape: ~890-935 KiB of spec-3 JSON
    /// per list and a 441 KiB spec-2 list. Nothing synthetic here — a
    /// fixture small enough to hand-author would not exercise the
    /// vendor map that dominates the retention.
    #[test]
    fn one_parsed_gvl_fits_the_per_list_charge() {
        for (spec, list, raw) in archive_entries() {
            let retained =
                retained_by(move || Gvl::load_from_json_bytes(raw, Some(spec)).expect("parses"));
            assert!(
                retained <= MAX_GVL_PARSED_BYTES,
                "(spec {spec}, list {list}) parsed from {} raw bytes retained \
                 {retained} B, over the {MAX_GVL_PARSED_BYTES} B the budget \
                 charges per resident list",
                raw.len()
            );
        }
    }

    /// A `Gvl` clone is a refcount bump, not a vendor-map copy.
    ///
    /// This is the measurement the budget's per-auction GVL term rests
    /// on. An auction holds ONE handle cloned out of the store's cache;
    /// if that clone were a deep copy, concurrent auctions would each
    /// cost a full list and the budget would have to charge one per
    /// auction. The marginal cost is asserted well under a kibibyte per
    /// handle — generous room over the pointer-width truth, and still
    /// four orders of magnitude below a parsed list, so a regression to
    /// deep-copying cannot hide under it.
    #[test]
    fn additional_gvl_handles_cost_a_pointer_not_a_copy() {
        let (spec, list, raw) = largest_entry();
        let one = retained_by(move || {
            let base = Gvl::load_from_json_bytes(raw, Some(spec)).expect("parses");
            vec![base]
        });
        let many_count = 64_usize;
        let many = retained_by(move || {
            let base = Gvl::load_from_json_bytes(raw, Some(spec)).expect("parses");
            let mut handles = Vec::with_capacity(many_count);
            for _handle in 0..many_count {
                handles.push(base.clone());
            }
            handles.push(base);
            handles
        });

        let marginal = many.saturating_sub(one);
        let extra = u64::try_from(many_count).unwrap_or(u64::MAX);
        let per_handle = marginal.checked_div(extra).unwrap_or(u64::MAX);
        assert!(
            per_handle < 1_024,
            "(spec {spec}, list {list}): {many_count} extra handles to ONE parsed \
             list cost {marginal} B ({per_handle} B each) — a handle must be a \
             refcount bump, not a vendor-map copy"
        );
    }

    /// Distinct lists are what actually cost bytes, and the co-resident
    /// set the budget charges for fits its charge.
    ///
    /// `GvlStore` holds at most `GVL_PARSE_CACHE_MAX_ENTRIES` parsed
    /// lists of its own; an auction that cloned a handle before its
    /// version was evicted keeps that list alive past the eviction.
    /// The co-resident worst case is therefore cache-owned entries plus
    /// one evicted-but-still-held list per concurrent auction, and each
    /// of those is a genuinely distinct parse. Measured with the largest
    /// bundled lists, which is what the charge has to cover.
    #[test]
    fn distinct_co_resident_lists_fit_the_charge() {
        let mut by_size = archive_entries();
        by_size.sort_by_key(|&(_spec, _list, raw)| Reverse(raw.len()));

        for count in 1_usize..=GVL_PARSE_CACHE_MAX_ENTRIES.saturating_add(1) {
            let picked: Vec<(u32, u32, &'static [u8])> =
                by_size.iter().take(count).copied().collect();
            assert_eq!(picked.len(), count, "the archive bundles enough lists");
            let retained = retained_by(move || {
                picked
                    .into_iter()
                    .map(|(spec, _list, raw)| {
                        Gvl::load_from_json_bytes(raw, Some(spec)).expect("parses")
                    })
                    .collect::<Vec<Gvl>>()
            });
            let charged =
                MAX_GVL_PARSED_BYTES.saturating_mul(u64::try_from(count).unwrap_or(u64::MAX));
            assert!(
                retained <= charged,
                "{count} distinct parsed lists retained {retained} B, over the \
                 {charged} B the budget charges for {count} resident lists"
            );
        }
    }
}
