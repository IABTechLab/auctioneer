//! What the exchange holds in outbound BODY bytes once a call is
//! dispatched, measured against what the isolate budget charges.
//!
//! The budget charges `max_outbound_raw_bytes + max_outbound_request_bytes`
//! per call. That funds exactly TWO bodies live at once, which is what
//! the gzip finalizer needs: it allocates the encoded buffer at the cap
//! and fills it while the plain body is still resident, and says so.
//!
//! Two is also the ceiling, and that is what this file measures. The
//! exchange used to clone each request for dispatch while keeping the
//! original for response processing, so a compressed call held THREE
//! bodies — retained plain, dispatched plain, encoded — against a term
//! that funds two. Response processing then cloned the request list a
//! second time to read two metadata fields off it.
//!
//! Neither copy was needed. Response processing reads the method, the
//! URL, the headers and the covered imp ids; the sink already builds
//! all four into an `OutboundRequestMeta`, and `make_bids` takes that
//! instead. So the body has exactly one owner at every instant: the
//! sink's buffer, moved into the dispatched request.
//!
//! # What is measured, and what is reasoned
//!
//! MEASURED: that handing an accepted call to dispatch does not grow
//! the live body bytes — the dispatched request's body IS the sink's
//! allocation, not a copy with the same length. A length comparison
//! could not tell those apart, which is why the probe counts bytes the
//! allocator actually holds.
//!
//! REASONED: the encoded buffer's own residency, which
//! `finalize_for_transport` documents and funds, and which happens
//! downstream of this boundary in `auctioneer-core`.

/// Test-scoped counting allocator (per-thread, live-bytes) backing the
/// outbound-body ownership bound. Delegates every operation to `System`.
mod alloc_track {
    #![expect(
        unsafe_code,
        reason = "a GlobalAlloc impl is unavoidably unsafe; test-only \
                  measurement backing the outbound-body memory charge"
    )]

    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static LIVE: Cell<i64> = const { Cell::new(0) };
        static PEAK: Cell<i64> = const { Cell::new(0) };
    }

    /// Reset both counters, so a measurement starts from zero whatever
    /// the harness allocated before it.
    pub fn reset_on_this_thread() {
        let _live = LIVE.try_with(|cell| cell.set(0));
        let _peak = PEAK.try_with(|cell| cell.set(0));
    }

    /// The HIGHEST live-byte total this thread reached since the last
    /// reset.
    ///
    /// The PEAK is the only reading that can see this defect. A copy
    /// made while its source is still alive, and freed in the same
    /// step, nets to nothing: measured live-before against live-after,
    /// the hand-off came out NEGATIVE whether it copied the body or
    /// moved it, because the meta's byte pool is released either way.
    /// What the isolate has to survive is the instant both buffers are
    /// resident, which is exactly what a peak records.
    pub fn peak_on_this_thread() -> i64 {
        PEAK.try_with(Cell::get).unwrap_or(0)
    }

    fn add(bytes: usize) {
        // TLS teardown can make try_with fail; a lost count only
        // relaxes the measurement, never breaks the allocator.
        let _outcome = LIVE.try_with(|cell| {
            let now = cell
                .get()
                .saturating_add(i64::try_from(bytes).unwrap_or(i64::MAX));
            cell.set(now);
            let _peak = PEAK.try_with(|peak| {
                if now > peak.get() {
                    peak.set(now);
                }
            });
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
    use std::thread;

    use super::alloc_track::{peak_on_this_thread, reset_on_this_thread};
    use auctioneer_bidder::bidder::EntryPoint;
    use auctioneer_bidder::{
        BidderHttpRequest, BidderName, BidderRequestInfo, CallBudget, EndpointRejectReason,
        EndpointValidator, OutboundRequestSink, SinkLimits,
    };

    /// The body every fixture writes: big enough that a copy of it is
    /// unmistakable against allocator noise.
    const BODY_BYTES: usize = 64 * 1024;

    /// What the hand-off is allowed to allocate.
    ///
    /// An eighth of the body, stated as bytes rather than derived by
    /// division so the figure is readable and the bound is one
    /// comparison. A move allocates the metadata strings only — a short
    /// method, a short URL, one imp id and their headers, measured well
    /// under a kilobyte — while a copy allocates `BODY_BYTES` on its
    /// own. The ceiling sits between them with room on both sides, so
    /// neither allocator noise nor a slightly longer URL can flip it.
    const MOVE_ALLOCATION_CEILING: i64 = 8 * 1024;

    /// Accepts every URL — endpoint policy has its own tests, and the
    /// subject here is ownership.
    struct AllowAll;

    impl EndpointValidator for AllowAll {
        fn validate(
            &self,
            _url: &url::Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            Ok(())
        }
    }

    fn request_info() -> BidderRequestInfo<'static> {
        const MACROS: &[&str] = &[];
        const CURRENCY: &[String] = &[];
        BidderRequestInfo::new(
            BidderName::new("fake_bb"),
            BidderName::new("fake_bb"),
            "acct-ownership",
            None,
            "https://demo.example.test/openrtb",
            MACROS,
            None,
            None,
            EntryPoint::Auction,
            CURRENCY,
        )
    }

    /// Dispatching an accepted call does not ALLOCATE a second body.
    ///
    /// The measurement brackets exactly the hand-off: a sink has
    /// accepted one request with a 64 KiB body, and the window covers
    /// turning that into the dispatchable request. A move allocates
    /// nothing but the metadata strings; the clone this replaces
    /// allocated a second 64 KiB buffer, because `BidderHttpRequest`
    /// owns its body and `.clone()` copies it.
    ///
    /// Read as a PEAK, not as a net change. The copy's source is freed
    /// in the same step, so a live-before/live-after reading comes out
    /// the same either way — it was tried, and it passed against the
    /// copying version.
    ///
    /// Run on its own thread so the counter sees only this work.
    #[test]
    fn dispatching_a_call_does_not_duplicate_its_body() {
        let peak = thread::spawn(|| {
            let budget = CallBudget::new(4);
            let validator = AllowAll;
            let limits = SinkLimits::new(256 * 1024, 100);
            let mut sink = OutboundRequestSink::new(limits, &budget, &validator);
            let info = request_info();
            sink.push_request(
                &info,
                |meta| {
                    meta.set_method("POST")?;
                    meta.set_url("https://demo.example.test/openrtb")?;
                    meta.imp_id("imp-1")
                },
                |body| body.write_all(&vec![b'x'; BODY_BYTES]),
            )
            .expect("the fixture fits every cap");
            let accepted = sink.into_outcome().accepted;

            // The window opens with the accepted call already built, so
            // the sink's own buffer is NOT counted and the peak measures
            // only what the hand-off adds on top of it.
            reset_on_this_thread();
            let dispatched: Vec<_> = accepted
                .into_iter()
                .map(BidderHttpRequest::into_parts)
                .collect();
            let peak = peak_on_this_thread();

            // Hold both halves across the reading, so nothing measured
            // here has been dropped yet.
            assert_eq!(dispatched.len(), 1, "one push, one dispatchable call");
            assert_eq!(
                dispatched.first().map(|(_meta, body)| body.len()),
                Some(BODY_BYTES),
                "the dispatched request carries the whole body",
            );
            assert_eq!(
                dispatched
                    .first()
                    .map(|(meta, _request)| meta.covered_imp_ids().count()),
                Some(1),
                "and the retained metadata still answers for seatnonbid",
            );
            peak
        })
        .join()
        .expect("the measuring thread does not panic");

        // A move allocates only the metadata strings re-materialized
        // from the meta's byte pool: a short method, a short URL, one
        // imp id and their `Vec`/`String` headers — well under a
        // kilobyte at this fixture. A COPY allocates a second 65 536
        // byte buffer, so the bound separates the two by orders of
        // magnitude rather than by a margin.
        assert!(
            peak < MOVE_ALLOCATION_CEILING,
            "handing one accepted call to dispatch peaked at {peak} bytes of fresh \
             allocation, over the {MOVE_ALLOCATION_CEILING}-byte ceiling a move needs \
             and within reach of the {BODY_BYTES}-byte body itself, which is the \
             signature of copying the body rather than moving it; the exchange must \
             hold ONE body per call, because the per-call budget term funds the plain \
             body and the encoder's output and has no room for a third",
        );
    }
}
