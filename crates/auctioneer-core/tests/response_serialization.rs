//! Allocation and byte-boundary guards for final-response serialization.

mod alloc_track {
    #![expect(
        unsafe_code,
        reason = "a test-only GlobalAlloc delegate measures serialization allocations \
                  and injects a recoverable reservation failure"
    )]

    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    use std::ptr;

    #[derive(Clone, Copy, Debug, Default)]
    pub struct Measurements {
        pub largest_request: usize,
        pub requested_bytes: usize,
        pub denied_requests: usize,
    }

    thread_local! {
        static ACTIVE: Cell<Option<Measurements>> = const { Cell::new(None) };
        static DENY_SIZE: Cell<Option<usize>> = const { Cell::new(None) };
    }

    struct Reset;

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for Reset {
        fn drop(&mut self) {
            ACTIVE.with(|cell| cell.set(None));
            DENY_SIZE.with(|cell| cell.set(None));
        }
    }

    pub fn measure<T, F: FnOnce() -> T>(deny_size: Option<usize>, run: F) -> (T, Measurements) {
        DENY_SIZE.with(|cell| cell.set(deny_size));
        ACTIVE.with(|cell| cell.set(Some(Measurements::default())));
        let reset = Reset;
        let result = run();
        let measured = ACTIVE.with(Cell::take).unwrap_or_default();
        drop(reset);
        (result, measured)
    }

    fn record(size: usize) -> bool {
        ACTIVE
            .try_with(|cell| {
                let Some(mut measured) = cell.get() else {
                    return false;
                };
                measured.largest_request = measured.largest_request.max(size);
                measured.requested_bytes = measured.requested_bytes.saturating_add(size);
                let denied = DENY_SIZE.try_with(Cell::get).ok().flatten() == Some(size);
                if denied {
                    measured.denied_requests = measured.denied_requests.saturating_add(1);
                }
                cell.set(Some(measured));
                denied
            })
            .unwrap_or(false)
    }

    pub struct MeasuringAllocator;

    // SAFETY: successful operations delegate unchanged to System; injected
    // failures return null without touching any existing allocation.
    unsafe impl GlobalAlloc for MeasuringAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            if record(layout.size()) {
                return ptr::null_mut();
            }
            // SAFETY: the caller supplies a valid allocation layout.
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: ptr and layout describe a matching System allocation.
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            if record(layout.size()) {
                return ptr::null_mut();
            }
            // SAFETY: the caller supplies a valid allocation layout.
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            if record(new_size) {
                return ptr::null_mut();
            }
            // SAFETY: ptr/layout match System and new_size is the caller's request.
            unsafe { System.realloc(ptr, layout, new_size) }
        }
    }
}

#[global_allocator]
static MEASURING_ALLOC: alloc_track::MeasuringAllocator = alloc_track::MeasuringAllocator;

#[cfg(test)]
mod tests {
    use super::alloc_track::measure;
    use auctioneer_core::serialize::{
        SerializeError, serialize_response, serialize_response_fitting_debug_within,
        serialize_response_within,
    };
    use auctioneer_openrtb::types::bid_response::BidResponse;
    use serde_json::json;
    use std::io::ErrorKind;

    fn escaped_response() -> BidResponse {
        serde_json::from_value(json!({
            "id": "auction\"\\\n\u{0000}\u{00e9}",
            "cur": "USD",
            "seatbid": [{
                "seat": "fake_bidder",
                "bid": [{
                    "id": "bid-1",
                    "impid": "imp-1",
                    "price": 1.25_f64,
                    "adm": "<div>\"quoted\"\\\n\t\u{0001}\u{1f680}</div>"
                }]
            }],
            "ext": {"message": "\r\n\t\\\"\u{0000}"},
            "extra": {"escaped\"key": [true, null, "\u{00e9}"]}
        }))
        .expect("response fixture deserializes")
    }

    #[test]
    fn oversized_response_does_not_allocate_an_oversized_temporary() {
        const CAP: usize = 4096;
        // Only the serialization call is measured, never fixture construction.
        let large_string = BidResponse {
            id: "x".repeat(4 * 1024 * 1024),
            ..BidResponse::default()
        };
        let many_escapes = BidResponse {
            ext: Some(json!(vec!["\u{0000}\"\\\n"; 64 * 1024])),
            ..BidResponse::default()
        };
        for response in [large_string, many_escapes] {
            let (result, measured) = measure(None, || serialize_response_within(&response, CAP));
            assert!(matches!(result, Err(SerializeError::OverCap { cap: CAP })));
            assert!(
                measured.largest_request <= CAP,
                "serialization requested an allocation larger than cap {CAP}: {measured:?}"
            );
            // Fixed error metadata is not response storage. Counting cumulative
            // requests also catches chunked full-body buffering before rejection.
            assert!(
                measured.requested_bytes <= CAP + 256,
                "serialization allocated beyond the output cap and fixed error allowance: \
                 {measured:?}"
            );
        }
    }

    #[test]
    fn escaped_json_at_exact_cap_is_byte_identical() {
        let response = escaped_response();
        let expected = serde_json::to_vec(&response).expect("reference serialization");
        let spare = expected.len().checked_add(1).expect("reference size fits");
        for cap in [expected.len(), spare] {
            let actual = serialize_response_within(&response, cap).expect("response fits");
            assert_eq!(actual, expected);
            assert!(
                actual.capacity() <= cap,
                "output storage must also fit the cap"
            );
        }
    }

    #[test]
    fn escaped_json_one_byte_over_cap_returns_no_prefix() {
        let response = escaped_response();
        let expected = serde_json::to_vec(&response).expect("reference serialization");
        let cap = expected.len().checked_sub(1).expect("nonempty JSON");
        assert!(matches!(
            serialize_response_within(&response, cap),
            Err(SerializeError::OverCap { cap: got }) if got == cap
        ));
    }

    #[test]
    fn browser_configurations_are_not_trimmed_to_fit_the_final_response_cap() {
        use auctioneer_bidder::{BidderName, BrowserAuctionConfig};
        use auctioneer_exchange::{
            BidderResultBundle, BrowserAuctionResult, ResponseInput, build_response,
        };
        use std::collections::BTreeMap;

        let mut response = build_response(ResponseInput {
            bidder_visible_tmax_ms: 100,
            auction_timestamp_ms: 0,
            return_all_bid_status: false,
            seatnonbid: BTreeMap::new(),
            bundles: BTreeMap::from([(
                BidderName::new("fake_alias"),
                BidderResultBundle {
                    dispatched: true,
                    browser_auctions: Some(BrowserAuctionResult {
                        adapter: BidderName::new("fake_canonical"),
                        configs: vec![BrowserAuctionConfig {
                            imp_id: "i".into(),
                            config: json!({"payload":"\"\\\n".repeat(1024)}),
                        }],
                    }),
                    ..BidderResultBundle::default()
                },
            )]),
            construction_diagnostics: BTreeMap::new(),
            prebid_errors: vec![],
            prebid_warnings: vec![],
        })
        .unwrap();
        let expected = serde_json::to_vec(&response).unwrap();
        assert!(
            !response.ext.as_ref().unwrap()["prebid"]["fledge"]["auctionconfigs"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            serialize_response_within(&response, expected.len()).unwrap(),
            expected
        );
        let cap = expected.len() - 1;
        assert!(matches!(
            serialize_response_fitting_debug_within(&mut response, cap),
            Err(SerializeError::OverCap { cap: got }) if got == cap
        ));
        assert_eq!(serde_json::to_vec(&response).unwrap(), expected);
    }

    #[test]
    fn zero_cap_refuses_without_allocating_response_storage() {
        let response = escaped_response();
        let (result, measured) = measure(None, || serialize_response_within(&response, 0));
        assert!(matches!(result, Err(SerializeError::OverCap { cap: 0 })));
        assert!(measured.requested_bytes <= 256, "{measured:?}");
    }

    #[test]
    fn capacity_overflow_is_terminal_without_panicking() {
        let result = serialize_response_within(&BidResponse::default(), usize::MAX);
        assert!(matches!(
            result,
            Err(SerializeError::Encoding(err))
                if err.io_error_kind() == Some(ErrorKind::OutOfMemory)
        ));
    }

    #[test]
    fn allocation_failure_is_terminal_without_panicking_or_fallback() {
        const CAP: usize = 4093;
        let response = escaped_response();
        let (result, measured) = measure(Some(CAP), || serialize_response_within(&response, CAP));
        assert!(matches!(
            result,
            Err(SerializeError::Encoding(err))
                if err.io_error_kind() == Some(ErrorKind::OutOfMemory)
        ));
        assert_eq!(
            measured.denied_requests, 1,
            "the allocation fault must be exercised"
        );
    }

    #[test]
    fn no_envelope_preserves_unbounded_serialization() {
        let response = escaped_response();
        assert_eq!(
            serialize_response(&response).expect("no envelope installed in this test binary"),
            serde_json::to_vec(&response).expect("reference serialization")
        );
    }
}
