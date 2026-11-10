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
//! Serialization fixtures are built before measurement. Header-history controls
//! build and drop input owners inside their window before sampling retention.
//! Neither uses fixture subtraction or claims physical allocator/guest bounds.

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

    pub fn live_on_this_thread() -> usize {
        LIVE.try_with(Cell::get).unwrap_or(0)
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
mod tests {
    use std::{hint::black_box, str, thread};

    use super::alloc_track::{
        live_on_this_thread, peak_on_this_thread, reallocations_on_this_thread,
        reset_on_this_thread, track_reallocations_at_least_on_this_thread,
    };
    use auctioneer_config_data::usersync::UserSyncConfig;
    use auctioneer_core::handlers::health_gvl::health_gvl;
    use auctioneer_core::handlers::sync_http::{SyncBrowserPolicy, response_header_cap};
    use auctioneer_core::host_primitives::{
        HealthAccessPolicy, TestClockHandle, install_health_primitives,
    };
    use auctioneer_core::serialize::{SerializeError, serialize_response_within};
    use auctioneer_openrtb::types::bid_response::BidResponse;
    use auctioneer_runtime::FakeClock;
    use bytes::Bytes;
    use edgezero_core::body::Body;
    use edgezero_core::context::RequestContext;
    use edgezero_core::params::PathParams;
    use futures::executor::block_on;
    use http::{HeaderMap, HeaderName, HeaderValue, Request, Response, StatusCode};
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
        RequestContext::new(request, PathParams::default())
    }

    #[test]
    fn health_json_output_does_not_reallocate_its_measured_reservation() {
        thread::spawn(|| {
            let scope: &'static str =
                Box::leak("isolate\0\n\"\\\u{00e9}".repeat(4097).into_boxed_str());
            install_health_primitives(HealthAccessPolicy::UnauthenticatedAcknowledged, scope)
                .unwrap();
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

    #[test]
    fn sync_json_outputs_do_not_reallocate_at_two_independent_sizes() {
        use auctioneer_core::handlers::sync_http::{
            SyncDebugStatus, cookie_sync_success, getuids_success,
        };
        use auctioneer_core::usersync::{cookie::UidCookie, limits::CookieLimits};

        thread::spawn(|| {
            for input_bytes in [2049, 8193] {
                let input = "<".repeat(input_bytes);
                let debug = [SyncDebugStatus {
                    bidder: "fake_bidder",
                    error: &input,
                }];
                let mut cookie = UidCookie::default();
                cookie
                    .set_uid(
                        "fake_bidder",
                        &input,
                        OffsetDateTime::UNIX_EPOCH,
                        &CookieLimits::default(),
                    )
                    .unwrap();
                for escape_html in [false, true] {
                    let build = |cap| {
                        if escape_html {
                            getuids_success(&cookie, cap)
                        } else {
                            cookie_sync_success(
                                &UidCookie::default(),
                                OffsetDateTime::UNIX_EPOCH,
                                &[],
                                Some(&debug),
                                cap,
                            )
                        }
                    };
                    let expected = build(128 * 1024).unwrap().into_body().into_bytes().unwrap();
                    track_reallocations_at_least_on_this_thread(expected.len().div_euclid(2));
                    let mut growable = Vec::new();
                    for chunk in expected.chunks(17) {
                        growable.extend_from_slice(chunk);
                    }
                    let control = reallocations_on_this_thread();
                    assert_eq!(growable, expected);
                    assert!(
                        control > 0,
                        "the growable control must register body-sized reallocations"
                    );

                    track_reallocations_at_least_on_this_thread(expected.len().div_euclid(2));
                    let actual = build(expected.len()).unwrap();
                    let reallocations = reallocations_on_this_thread();
                    assert_eq!(actual.into_body().into_bytes().unwrap(), expected);
                    assert_eq!(
                        reallocations, 0,
                        "{input_bytes} input bytes, escaped={escape_html}"
                    );
                }
            }
        })
        .join()
        .unwrap();
    }

    #[test]
    fn complete_cookie_formatting_never_grows_its_reserved_field() {
        use auctioneer_core::usersync::cookie::{CookieWriteOptions, prepare_set_cookie_value};

        thread::spawn(|| {
            for value_bytes in [2049, 3553] {
                let value = "a".repeat(value_bytes);
                let options = CookieWriteOptions {
                    domain: "sync.example.test",
                    user_agent: Some("Chrome/130.0"),
                    ..CookieWriteOptions::default()
                };
                let expected =
                    prepare_set_cookie_value(&value, &options, OffsetDateTime::UNIX_EPOCH).unwrap();
                let field = expected.field_value();
                assert!(field.as_bytes().len() > value_bytes);
                track_reallocations_at_least_on_this_thread(value_bytes.div_euclid(2));
                let mut control = String::new();
                for chunk in field.to_str().unwrap().as_bytes().chunks(17) {
                    control.push_str(str::from_utf8(chunk).unwrap());
                }
                assert!(reallocations_on_this_thread() > 0);
                assert_eq!(control.as_bytes(), field.as_bytes());

                track_reallocations_at_least_on_this_thread(value_bytes.div_euclid(2));
                let prepared =
                    prepare_set_cookie_value(&value, &options, OffsetDateTime::UNIX_EPOCH).unwrap();
                let growth = reallocations_on_this_thread();
                assert_eq!(prepared.field_value(), field);
                assert_eq!(growth, 0, "complete {value_bytes}-byte cookie value");
            }
        })
        .join()
        .unwrap();
    }

    #[derive(Clone, Copy, Debug)]
    struct HeaderShape {
        unique_fields: usize,
        duplicate_values: usize,
        name_bytes: usize,
        value_bytes: usize,
    }

    const SMALL_HEADER_BACKING_BYTES: usize = 16 * 1024;
    const LARGE_HEADER_BACKING_BYTES: usize = 1024 * 1024;

    fn header_history_response(
        shape: HeaderShape,
        backing_bytes: usize,
        removed_fields: usize,
    ) -> (Response<Body>, Bytes) {
        let origin = b"https://buyer.example";
        let origin_end = 32_usize.checked_add(origin.len()).unwrap();
        let values = shape
            .unique_fields
            .checked_add(shape.duplicate_values)
            .unwrap();
        let mut storage = vec![b'x'; backing_bytes];
        storage[32..origin_end].copy_from_slice(origin);
        for index in 0..values {
            let start = origin_end
                .checked_add(index.checked_mul(shape.value_bytes).unwrap())
                .unwrap();
            let end = start.checked_add(shape.value_bytes).unwrap();
            let marker = b'a'.checked_add(u8::try_from(index).unwrap()).unwrap();
            storage[start..end].fill(marker);
        }
        let backing = Bytes::from(storage);
        let mut headers = HeaderMap::new();
        for index in 0..removed_fields {
            let name = HeaderName::from_bytes(format!("x-removed-{index}").as_bytes()).unwrap();
            headers.append(name.clone(), HeaderValue::from_static("removed-first"));
            headers.append(name, HeaderValue::from_static("removed-second"));
        }
        assert_eq!(headers.len(), removed_fields.checked_mul(2).unwrap());
        let capacity = headers.capacity();
        assert!(capacity >= removed_fields);
        for index in 0..removed_fields {
            let name = format!("x-removed-{index}");
            assert_eq!(headers.get_all(&name).iter().count(), 2);
            assert_eq!(headers.remove(&name).unwrap(), "removed-first");
            assert!(!headers.contains_key(&name));
        }
        assert!(headers.is_empty());
        assert_eq!(headers.capacity(), capacity);
        for index in 0..values {
            let field = if index < shape.unique_fields {
                index
            } else {
                0
            };
            let mut name = "x".repeat(shape.name_bytes.checked_sub(1).unwrap());
            name.push(char::from(
                b'a'.checked_add(u8::try_from(field).unwrap()).unwrap(),
            ));
            let start = origin_end
                .checked_add(index.checked_mul(shape.value_bytes).unwrap())
                .unwrap();
            let end = start.checked_add(shape.value_bytes).unwrap();
            let mut value = HeaderValue::from_maybe_shared(backing.slice(start..end)).unwrap();
            assert_eq!(
                value.as_bytes().as_ptr().addr(),
                backing.as_ptr().addr().checked_add(start).unwrap(),
            );
            assert_eq!(value.as_bytes().len(), shape.value_bytes);
            value.set_sensitive(index.is_multiple_of(2));
            headers.append(HeaderName::from_bytes(name.as_bytes()).unwrap(), value);
        }
        headers.append("vary", HeaderValue::from_static("Accept-Encoding"));
        headers.append("vary", HeaderValue::from_static("Origin"));
        headers.insert("cache-control", HeaderValue::from_static("no-store"));
        headers.insert(
            "access-control-allow-origin",
            HeaderValue::from_maybe_shared(backing.slice(32..origin_end)).unwrap(),
        );
        headers.insert(
            "access-control-allow-credentials",
            HeaderValue::from_static("true"),
        );
        let mut response = Response::new(Body::from_bytes(b"unchanged".to_vec()));
        *response.status_mut() = StatusCode::ACCEPTED;
        *response.headers_mut() = headers;
        (response, backing)
    }

    fn assert_header_history_output(response: &Response<Body>, before: &[(String, Vec<u8>, bool)]) {
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(response.body().as_bytes().unwrap(), b"unchanged");
        assert_eq!(response.headers().len(), before.len());
        for ((name, value), (old_name, old_value, sensitive)) in
            response.headers().iter().zip(before)
        {
            assert_eq!(name.as_str(), old_name);
            assert_eq!(value.as_bytes(), old_value);
            assert_eq!(value.is_sensitive(), *sensitive);
        }
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(
            response.headers()["access-control-allow-origin"],
            "https://buyer.example",
        );
        assert_eq!(
            response.headers()["access-control-allow-credentials"],
            "true"
        );
        assert!(
            response
                .headers()
                .get_all("vary")
                .iter()
                .map(HeaderValue::as_bytes)
                .eq([b"Accept-Encoding".as_slice(), b"Origin".as_slice()]),
        );
    }

    // Requested native heap only: all fixture owners die before the live sample.
    fn measured_header_history(
        shape: HeaderShape,
        backing_bytes: usize,
        removed_fields: usize,
        decorate: bool,
    ) -> (usize, usize) {
        thread::spawn(move || {
            reset_on_this_thread();
            let response = {
                let (source_response, backing) =
                    header_history_response(shape, backing_bytes, removed_fields);
                let source_capacity = source_response.headers().capacity();
                let before: Vec<_> = source_response
                    .headers()
                    .iter()
                    .map(|(name, value)| {
                        (
                            name.as_str().to_owned(),
                            value.as_bytes().to_vec(),
                            value.is_sensitive(),
                        )
                    })
                    .collect();
                let config = UserSyncConfig {
                    allowed_origins: vec!["https://buyer.example".to_owned()],
                    ..UserSyncConfig::default()
                };
                let mut request = HeaderMap::new();
                request.insert(
                    "origin",
                    source_response.headers()["access-control-allow-origin"].clone(),
                );
                assert_eq!(
                    request["origin"].as_bytes().as_ptr().addr(),
                    backing.as_ptr().addr().checked_add(32).unwrap(),
                );
                let response = if decorate {
                    SyncBrowserPolicy::new(&config)
                        .decorate(source_response, &request, response_header_cap())
                        .expect("the complete fixture fits the existing header contract")
                } else {
                    source_response
                };
                assert_header_history_output(&response, &before);
                let start = backing.as_ptr().addr();
                let addresses = start..start.checked_add(backing.len()).unwrap();
                let shared = response
                    .headers()
                    .values()
                    .filter(|value| addresses.contains(&value.as_bytes().as_ptr().addr()))
                    .count();
                if decorate {
                    assert_eq!(shared, 0);
                    if removed_fields > 0 {
                        assert!(response.headers().capacity() < source_capacity);
                    }
                } else {
                    let expected_shared = shape
                        .unique_fields
                        .checked_add(shape.duplicate_values)
                        .unwrap()
                        .checked_add(1)
                        .unwrap();
                    assert_eq!(shared, expected_shared);
                    assert_eq!(response.headers().capacity(), source_capacity);
                }
                response
            };
            let retained = live_on_this_thread();
            let peak = peak_on_this_thread();
            assert!(retained > 0 && retained <= peak);
            assert!(
                peak >= backing_bytes,
                "the window must include the source backing",
            );
            drop(response);
            assert_eq!(
                live_on_this_thread(),
                0,
                "the response was the last measured owner",
            );
            (retained, peak)
        })
        .join()
        .expect("header-history measurement thread completed")
    }

    #[test]
    fn sync_header_retention_is_independent_of_source_backing_and_map_history() {
        for unique_fields in [1_usize, 5] {
            for duplicate_values in [0_usize, 3] {
                for name_bytes in [17_usize, 129] {
                    for value_bytes in [19_usize, 271] {
                        let shape = HeaderShape {
                            unique_fields,
                            duplicate_values,
                            name_bytes,
                            value_bytes,
                        };
                        let (reference, _) =
                            measured_header_history(shape, SMALL_HEADER_BACKING_BYTES, 0, true);
                        for backing_bytes in
                            [SMALL_HEADER_BACKING_BYTES, LARGE_HEADER_BACKING_BYTES]
                        {
                            for removed_fields in [0_usize, 256] {
                                let (retained, _) = measured_header_history(
                                    shape,
                                    backing_bytes,
                                    removed_fields,
                                    true,
                                );
                                assert_eq!(
                                    retained, reference,
                                    "{shape:?}, backing={backing_bytes}, removed={removed_fields}",
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn header_history_control_detects_retained_shared_backing_and_map_storage() {
        let shape = HeaderShape {
            unique_fields: 2,
            duplicate_values: 3,
            name_bytes: 17,
            value_bytes: 19,
        };
        let (small, _) = measured_header_history(shape, SMALL_HEADER_BACKING_BYTES, 0, false);
        let (large, _) = measured_header_history(shape, LARGE_HEADER_BACKING_BYTES, 0, false);
        assert_eq!(
            large.checked_sub(small).unwrap(),
            LARGE_HEADER_BACKING_BYTES
                .checked_sub(SMALL_HEADER_BACKING_BYTES)
                .unwrap(),
        );
        let (with_history, _) =
            measured_header_history(shape, SMALL_HEADER_BACKING_BYTES, 256, false);
        assert!(
            with_history > small,
            "removed fields must leave detectable map storage",
        );
    }
}
