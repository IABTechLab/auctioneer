//! What rejecting an oversized response actually ALLOCATES, measured
//! against the cap it is rejected for.
//!
//! `serialize_response_within` promises two different things, and only
//! one of them is visible from its return value. That no over-cap
//! response is ever SENT is provable from the error alone, and
//! `serialize.rs`'s own tests pin it. That rejecting one does not
//! itself allocate past the cap is not: an implementation that builds
//! the whole body and then measures it returns exactly the same error,
//! having allocated however far over the response overshot.
//!
//! So the quantity under test is the PEAK transient allocation across
//! the call, and the allocator is the only place it is observable. A
//! counting global allocator can only exist in a test binary, which is
//! why this measurement lives here rather than beside the function.
//! Caller-owned response fixtures are built before measurement; no fixture
//! subtraction or host-exact allocation baseline contributes to the bound.

/// Test-scoped counting allocator (per-thread, peak bytes) backing the
/// serialization allocation bound. Delegates every operation to
/// `System`.
mod alloc_track {
    #![expect(
        unsafe_code,
        reason = "a GlobalAlloc impl is unavoidably unsafe; test-only \
                  measurement backing the final-response allocation bound"
    )]

    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static LIVE: Cell<usize> = const { Cell::new(0) };
        static PEAK: Cell<usize> = const { Cell::new(0) };
        static REALLOC_MIN_SIZE: Cell<usize> = const { Cell::new(usize::MAX) };
        static REALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    }

    /// Reset this thread's counters, so a measurement starts from zero
    /// regardless of what the harness allocated before it.
    pub fn reset_on_this_thread() {
        let _live = LIVE.try_with(|cell| cell.set(0));
        let _peak = PEAK.try_with(|cell| cell.set(0));
    }

    /// The HIGHEST live-byte total this thread reached since the last
    /// reset.
    ///
    /// Peak rather than live, because the defect under test is a
    /// transient: a serialize-then-measure implementation frees the
    /// oversized buffer before returning, so a live-bytes reading taken
    /// afterwards shows nothing. The peak is what the isolate budget
    /// has to survive.
    pub fn peak_on_this_thread() -> usize {
        PEAK.try_with(Cell::get).unwrap_or(0)
    }

    pub fn track_reallocations_at_least_on_this_thread(size: usize) {
        let _minimum = REALLOC_MIN_SIZE.try_with(|cell| cell.set(size));
        let _count = REALLOCATIONS.try_with(|cell| cell.set(0));
    }

    pub fn reallocations_on_this_thread() -> usize {
        REALLOCATIONS.try_with(Cell::get).unwrap_or(0)
    }

    fn add(bytes: usize) {
        // TLS teardown can make try_with fail; a lost count only
        // relaxes the measurement, never breaks the allocator.
        let _outcome = LIVE.try_with(|cell| {
            let now = cell.get().saturating_add(bytes);
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
            cell.set(cell.get().saturating_sub(bytes));
        });
    }

    pub struct CountingAllocator;

    // SAFETY: every operation delegates verbatim to `System`; the
    // counting side effect touches only thread-local counters and
    // never the returned memory.
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            // SAFETY: same layout contract as the caller's.
            let ptr = unsafe { System.alloc(layout) };
            if !ptr.is_null() {
                add(layout.size());
            }
            ptr
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            sub(layout.size());
            // SAFETY: ptr/layout come from a matching System alloc.
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            // SAFETY: same layout contract as the caller's.
            let ptr = unsafe { System.alloc_zeroed(layout) };
            if !ptr.is_null() {
                add(layout.size());
            }
            ptr
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let _minimum = REALLOC_MIN_SIZE.try_with(|minimum| {
                if new_size >= minimum.get() {
                    let _count = REALLOCATIONS.try_with(|count| {
                        count.set(count.get().saturating_add(1));
                    });
                }
            });
            // SAFETY: ptr/layout come from a matching System alloc;
            // new_size is the caller's requested size.
            let result = unsafe { System.realloc(ptr, layout, new_size) };
            if !result.is_null() {
                if new_size >= layout.size() {
                    add(new_size.saturating_sub(layout.size()));
                } else {
                    sub(layout.size().saturating_sub(new_size));
                }
            }
            result
        }
    }
}

#[global_allocator]
static COUNTING_ALLOC: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
mod config_funding_support;
#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use crate::config_funding_support as config_funding;
    use std::{hint::black_box, thread};

    use super::alloc_track::{
        peak_on_this_thread, reallocations_on_this_thread, reset_on_this_thread,
        track_reallocations_at_least_on_this_thread,
    };
    use auctioneer_core::handlers::health_gvl::health_gvl;
    use auctioneer_core::host_primitives::{TestClockHandle, install_health_counter_scope};
    use auctioneer_core::serialize::{SerializeError, serialize_response_within};
    use auctioneer_core::test_support::config_store::{context, runtime_registry};
    use auctioneer_openrtb::types::bid_response::BidResponse;
    use auctioneer_runtime::FakeClock;
    use edgezero_core::body::Body;
    use edgezero_core::context::RequestContext;
    use futures::executor::block_on;
    use http::{Request, StatusCode};
    use std::sync::Arc;
    use time::OffsetDateTime;

    /// The cap under test. Small enough that the oversized fixture
    /// dwarfs it, so a serialize-then-measure implementation's peak is
    /// unmistakable rather than within measurement noise.
    const CAP: usize = 4096;

    /// How far over the cap the fixture reaches.
    const OVERSIZE: usize = 4 * 1024 * 1024;

    // The funded body buffer is exactly CAP. A refusal also constructs a
    // boxed serde_json error from the writer's fixed io::ErrorKind; 256 B
    // conservatively bounds that fixed bookkeeping on the supported pointer
    // widths. It is not permission for another body or input-sized scratch.
    const ERROR_SCRATCH_BYTES: usize = 256;

    /// Peak bytes `call` reached, measured on a THREAD OF ITS OWN.
    ///
    /// The counter is thread-local and a fresh thread starts at zero,
    /// so the reading covers this call and nothing the harness did
    /// around it.
    fn peak_bytes_of<F>(response: BidResponse, call: F) -> usize
    where
        F: FnOnce(&BidResponse) + Send + 'static,
    {
        thread::spawn(move || {
            reset_on_this_thread();
            call(&response);
            peak_on_this_thread()
        })
        .join()
        .expect("measurement thread completed")
    }

    fn oversized_response(id_bytes: usize) -> BidResponse {
        BidResponse {
            id: "y".repeat(id_bytes),
            ..BidResponse::default()
        }
    }

    fn rejection_peak(response: BidResponse, cap: usize) -> usize {
        peak_bytes_of(response, move |resp| {
            let outcome = serialize_response_within(resp, cap);
            assert!(
                matches!(outcome, Err(SerializeError::OverCap { cap: actual }) if actual == cap),
                "the fixture must take the over-cap branch",
            );
        })
    }

    fn assert_rejection_bound(peak: usize, cap: usize) {
        let budget = cap
            .checked_add(ERROR_SCRATCH_BYTES)
            .expect("test bound fits");
        assert!(
            peak <= budget,
            "rejection peaked at {peak} B against a {budget}-byte bound: \
             {cap}-byte reserved buffer plus {ERROR_SCRATCH_BYTES}-byte fixed error scratch",
        );
    }

    /// Rejecting an over-cap response must not allocate past the cap.
    ///
    /// This is the assertion that separates a bounded implementation
    /// from one that checks the size after the fact. Both return
    /// `OverCap`; only the bounded one keeps its peak near the cap.
    #[test]
    fn rejecting_an_over_cap_response_does_not_allocate_past_the_cap() {
        let within_cap_peak = peak_bytes_of(BidResponse::default(), |resp| {
            let bytes = serialize_response_within(resp, CAP).expect("a default response fits");
            assert!(!bytes.is_empty(), "a serialized response is non-empty");
        });
        assert!(
            within_cap_peak <= CAP,
            "successful serialization must fit its reserved buffer: {within_cap_peak} B",
        );
        assert_rejection_bound(rejection_peak(oversized_response(OVERSIZE), CAP), CAP);
    }

    /// The rejection cost scales with the CAP, not with how far the
    /// response overshot it.
    ///
    /// The previous test bounds the peak once. This one pins the SHAPE
    /// of the bound by rejecting responses of two very different sizes
    /// against the same cap. A bounded implementation stops at the cap
    /// either way, so the two rejections cost about the same. Fixtures
    /// are outside the measurement. A serialize-then-measure one
    /// pays for whichever body it was handed, so its two readings
    /// differ by roughly the difference between the fixtures.
    #[test]
    fn the_rejection_cost_does_not_scale_with_the_overshoot() {
        /// The smaller oversized fixture: over the cap, but far below
        /// `OVERSIZE`.
        const MODEST_OVERSIZE: usize = 64 * 1024;

        let modest_cost = rejection_peak(oversized_response(MODEST_OVERSIZE), CAP);
        let huge_cost = rejection_peak(oversized_response(OVERSIZE), CAP);
        assert_rejection_bound(modest_cost, CAP);
        assert_rejection_bound(huge_cost, CAP);

        let budget = modest_cost + ERROR_SCRATCH_BYTES;
        assert!(
            huge_cost <= budget,
            "rejecting a {OVERSIZE}-byte response cost {huge_cost} bytes \
             while rejecting a {MODEST_OVERSIZE}-byte one cost {modest_cost}, \
             against a {budget}-byte budget: the cost of a refusal is scaling with the \
             response instead of stopping at the {CAP}-byte cap",
        );
    }

    #[test]
    fn rejection_allocation_tracks_the_installed_cap() {
        for cap in [64, CAP, 16 * 1024] {
            assert_rejection_bound(rejection_peak(oversized_response(OVERSIZE), cap), cap);
        }
    }

    #[test]
    fn escaping_expansion_is_rejected_with_only_fixed_error_scratch() {
        let response = BidResponse {
            id: "\0".repeat(CAP),
            ..BidResponse::default()
        };
        assert_rejection_bound(rejection_peak(response, CAP), CAP);
    }

    #[test]
    fn allocation_guards_detect_response_cloning_and_unbounded_serialization() {
        let [small_clone, large_clone] = [64 * 1024, OVERSIZE].map(|id_bytes| {
            peak_bytes_of(oversized_response(id_bytes), |resp| {
                drop(black_box(resp.clone()));
            })
        });
        assert!(small_clone > CAP + ERROR_SCRATCH_BYTES);
        assert!(
            large_clone > small_clone + ERROR_SCRATCH_BYTES,
            "a caller-tree clone must fail the cross-size bound",
        );
        let [small_unbounded, large_unbounded] = [64 * 1024, OVERSIZE].map(|id_bytes| {
            peak_bytes_of(oversized_response(id_bytes), |resp| {
                drop(black_box(serde_json::to_vec(resp).expect("valid response")));
            })
        });
        assert!(small_unbounded > CAP + ERROR_SCRATCH_BYTES);
        assert!(
            large_unbounded > small_unbounded + ERROR_SCRATCH_BYTES,
            "serialize-then-measure must fail the cross-size bound",
        );
    }

    #[test]
    fn allocation_peak_does_not_double_count_reallocated_storage() {
        let _peak = peak_bytes_of(BidResponse::default(), |_| {
            let mut bytes = vec![0_u8; 1024];
            bytes.reserve_exact(1024);
            assert_eq!(
                peak_on_this_thread(),
                bytes.capacity(),
                "a successful realloc replaces the old live allocation",
            );
            drop(black_box(bytes));
        });
    }

    fn health_context() -> RequestContext {
        let mut request = Request::builder()
            .uri("/_auctioneer/health/gvl")
            .body(Body::empty())
            .unwrap();
        request
            .extensions_mut()
            .insert(TestClockHandle(Arc::new(FakeClock::new(
                OffsetDateTime::UNIX_EPOCH,
            ))));
        request.extensions_mut().insert(runtime_registry(
            &serde_json::json!({"schema_version": 1_u8,
            "host": {"gvl": {"authenticate_health": false}}}),
        ));
        context(request)
    }

    #[test]
    fn health_json_output_does_not_reallocate_its_measured_reservation() {
        thread::spawn(|| {
            let scope: &'static str =
                Box::leak("isolate\0\n\"\\\u{00e9}".repeat(4097).into_boxed_str());
            config_funding::install();
            install_health_counter_scope(scope).unwrap();
            let warmup = block_on(health_gvl(health_context())).unwrap();
            assert_eq!(warmup.status(), StatusCode::OK);
            assert_eq!(warmup.headers()["content-type"], "application/json");
            let expected = warmup.into_body().into_bytes().unwrap();
            let payload: serde_json::Value = serde_json::from_slice(&expected).unwrap();
            assert_eq!(payload["counter_scope"], scope);

            // Observe body-sized growth, not small handler scratch allocations.
            track_reallocations_at_least_on_this_thread(expected.len());
            let unbounded = serde_json::to_vec(&payload).unwrap();
            let control_reallocations = reallocations_on_this_thread();
            assert_eq!(unbounded, expected);
            assert!(
                control_reallocations > 0,
                "the control must observe output growth"
            );

            let ctx = health_context();
            track_reallocations_at_least_on_this_thread(expected.len());
            let response = block_on(health_gvl(ctx)).unwrap();
            let output_reallocations = reallocations_on_this_thread();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.into_body().into_bytes().unwrap(), expected);
            assert_eq!(
                output_reallocations, 0,
                "the final JSON buffer must not grow"
            );
        })
        .join()
        .unwrap();
    }
}
