//! Allocation and byte-boundary guards for final-response serialization.
//! Real-App refusal controls run in a child of this target so its boot envelope
//! cannot contaminate the existing no-envelope serialization controls.

#[cfg(all(test, not(target_family = "wasm")))]
pub use auctioneer_core::test_support::bidder_fixture;
#[cfg(all(test, not(target_family = "wasm")))]
mod bounded_store;
#[cfg(all(test, not(target_family = "wasm")))]
mod fanout_limits;
#[cfg(all(test, not(target_family = "wasm")))]
#[expect(
    dead_code,
    reason = "shared App support exposes helpers used by other integration targets"
)]
mod usersync_app;

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

    #[cfg(not(target_family = "wasm"))]
    pub fn arm_denial(size: usize) {
        assert!(
            ACTIVE.with(|cell| cell.get().is_some()),
            "an allocation fault requires an active measurement"
        );
        DENY_SIZE.with(|cell| {
            assert_eq!(
                cell.replace(Some(size)),
                None,
                "arm only one allocation fault"
            );
        });
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
    use auctioneer_core::handlers::sync_http::{
        ResponseBuildError, SyncDebugStatus, cookie_sync_success, getuids_success,
    };
    use auctioneer_core::host_primitives::boot_gates;
    use auctioneer_core::serialize::{
        SerializeError, serialize_response, serialize_response_fitting_debug_within,
        serialize_response_within,
    };
    use auctioneer_core::usersync::{
        cookie::{CookieWriteError, CookieWriteOptions, UidCookie, prepare_set_cookie_value},
        limits::CookieLimits,
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
    fn sync_raw_json_reservation_failure_returns_no_partial_response_or_retry() {
        for input_bytes in [2049, 8193] {
            let input = "x".repeat(input_bytes);
            let debug = [SyncDebugStatus {
                bidder: "fake_bidder",
                error: &input,
            }];
            let expected = json!({
                "status": "no_cookie", "bidder_status": [],
                "debug": [{"bidder": "fake_bidder", "error": input}]
            });
            let reservation = serde_json::to_vec(&expected)
                .unwrap()
                .len()
                .checked_add(1)
                .unwrap();
            let cookie = UidCookie::default();
            let build = || {
                cookie_sync_success(
                    &cookie,
                    time::OffsetDateTime::UNIX_EPOCH,
                    &[],
                    Some(&debug),
                    reservation,
                )
            };
            let (failed, measured) = measure(Some(reservation), build);
            assert_eq!(failed.unwrap_err(), ResponseBuildError::Serialization);
            assert_eq!(measured.denied_requests, 1, "{measured:?}");
            assert_eq!(
                measured.requested_bytes, reservation,
                "no retry or fallback buffer"
            );

            let (accepted, success_measurement) = measure(None, build);
            let response = accepted.unwrap();
            let body = response.body().as_bytes().unwrap();
            assert_eq!(body.len(), reservation);
            assert_eq!(body.last(), Some(&b'\n'));
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(body).unwrap(),
                expected
            );
            assert!(!response.headers().contains_key("set-cookie"));
            assert_eq!(success_measurement.denied_requests, 0);
            assert_eq!(success_measurement.largest_request, reservation);
        }
    }

    fn html_json_fixture(
        input_bytes: usize,
        now: time::OffsetDateTime,
    ) -> (UidCookie, Vec<u8>, Vec<u8>) {
        let input = "<".repeat(input_bytes);
        let mut cookie = UidCookie::default();
        cookie
            .set_uid("fake_bidder", &input, now, &CookieLimits::default())
            .unwrap();
        let raw = serde_json::to_vec(&json!({"buyeruids": {"fake_bidder": input}})).unwrap();
        let mut escaped = String::from_utf8(raw.clone())
            .unwrap()
            .replace('<', "\\u003c");
        escaped.push('\n');
        (cookie, raw, escaped.into_bytes())
    }

    #[test]
    fn sync_html_json_reservation_failures_return_no_partial_response_or_retry() {
        for input_bytes in [1049, 2053] {
            let (cookie, raw, expected) =
                html_json_fixture(input_bytes, time::OffsetDateTime::UNIX_EPOCH);
            assert!(expected.len() > raw.len());
            // Only the owning input projection may allocate before the first
            // reservation; do not hide retries inside an aggregate allowance.
            let (projection, preparation) = measure(None, || cookie.uids());
            assert!(preparation.requested_bytes > 0);
            drop(projection);
            for reservation in [raw.len(), expected.len()] {
                let (failed, measured) = measure(Some(reservation), || {
                    getuids_success(&cookie, expected.len())
                });
                assert_eq!(failed.unwrap_err(), ResponseBuildError::Serialization);
                assert_eq!(measured.denied_requests, 1, "{measured:?}");
                assert_eq!(measured.largest_request, reservation, "{measured:?}");
                let raw_requested = preparation.requested_bytes.checked_add(raw.len()).unwrap();
                let requested = if reservation == expected.len() {
                    raw_requested.checked_add(expected.len()).unwrap()
                } else {
                    raw_requested
                };
                assert_eq!(
                    measured.requested_bytes, requested,
                    "no retry or fallback buffer"
                );
            }
            let (accepted, measured) = measure(None, || getuids_success(&cookie, expected.len()));
            let response = accepted.unwrap();
            assert_eq!(response.body().as_bytes().unwrap(), expected);
            assert!(!response.headers().contains_key("set-cookie"));
            assert_eq!(measured.denied_requests, 0);
            assert_eq!(measured.largest_request, expected.len());
        }
    }

    #[test]
    fn sync_cookie_field_reservation_failure_returns_no_prepared_header_or_retry() {
        for value_bytes in [257, 1049] {
            let value = "a".repeat(value_bytes);
            for reservation in [3079, 4096] {
                let options = CookieWriteOptions {
                    domain: "sync.example.test",
                    user_agent: Some("Chrome/130.0"),
                    hard_cap_bytes: reservation,
                    ..CookieWriteOptions::default()
                };
                let build =
                    || prepare_set_cookie_value(&value, &options, time::OffsetDateTime::UNIX_EPOCH);
                let (name, validation) = measure(None, || {
                    http::HeaderName::from_bytes(options.name.as_bytes()).unwrap()
                });
                drop(name);
                let (failed, measured) = measure(Some(reservation), build);
                assert_eq!(failed.unwrap_err(), CookieWriteError::CodecLimit);
                assert_eq!(measured.denied_requests, 1, "{measured:?}");
                assert_eq!(measured.largest_request, reservation);
                assert_eq!(
                    measured.requested_bytes,
                    validation.requested_bytes.checked_add(reservation).unwrap(),
                    "reservation failure must precede formatting and header conversion"
                );

                let (accepted, success_measurement) = measure(None, build);
                let prepared = accepted.unwrap();
                let field = prepared.field_value().to_str().unwrap();
                let parsed = cookie::Cookie::parse(field).unwrap();
                assert_eq!(parsed.name(), "uids");
                assert_eq!(parsed.value(), value);
                assert_eq!(parsed.domain(), Some("sync.example.test"));
                assert_eq!(parsed.same_site(), Some(cookie::SameSite::None));
                assert_eq!(parsed.secure(), Some(true));
                assert!(field.len() > value_bytes && field.len() <= reservation);
                assert_eq!(success_measurement.denied_requests, 0);
                assert_eq!(success_measurement.largest_request, reservation);
            }
        }
    }

    #[cfg(not(target_family = "wasm"))]
    mod sync_reservation_egress {
        use super::{html_json_fixture, measure};
        use crate::{alloc_track, usersync_app};
        use auctioneer_core::{
            host_primitives::boot_gates,
            test_support::{live_catalog, usersync_keys},
            usersync::limits::CookieLimits,
        };
        use bytes::Bytes;
        use edgezero_core::{
            body::Body,
            config_store::BoundedStoreRead,
            http::{Method, StatusCode},
            response_egress::{ResponseEgressEnvelope, ResponseEgressOutcome},
            response_egress_framing::ResponseEgressFramingMode,
            secret_store::{SecretError, SecretHandle, SecretStore},
            store_registry::{BoundSecretStore, SecretRegistry, StoreRegistry},
            time::{Deadline, MonotonicClock},
        };
        use futures::executor::block_on;
        use serde_json::{Value, json};
        use std::{
            env::{current_exe, var_os},
            process::Command,
            sync::{
                Arc,
                atomic::{AtomicUsize, Ordering},
            },
        };

        struct ReservationSecrets {
            deny_size: Option<usize>,
            reads: AtomicUsize,
        }

        fn assert_auxiliary_receipts(held: bool) {
            let gates = boot_gates().unwrap();
            let peak = gates.aux_work_derived.peak_bytes();
            assert!(peak > 0);
            assert_eq!(gates.aux.in_use(), u32::from(held));
            assert_eq!(gates.aux_work_budget.in_use(), if held { peak } else { 0 });
            assert_eq!(gates.static_egress.in_use(), 0);
        }

        #[async_trait::async_trait(?Send)]
        impl SecretStore for ReservationSecrets {
            async fn get_bytes(&self, _: &str, _: &str) -> Result<Option<Bytes>, SecretError> {
                panic!("the App must use the bounded keyring read");
            }

            async fn get_bytes_bounded(
                &self,
                store_name: &str,
                key: &str,
                clock: &MonotonicClock,
                deadline: Deadline,
                max_backend_bytes: u64,
                max_value_bytes: u64,
            ) -> Result<BoundedStoreRead<Bytes>, SecretError> {
                assert_eq!((store_name, key), ("default", "usersync_cookie_keyring"));
                assert_eq!((max_backend_bytes, max_value_bytes), (4096, 4096));
                assert!(!deadline.is_expired_at(clock.now()));
                let value = Bytes::from_static(usersync_keys::SECRET_JSON.as_bytes());
                let backend_bytes = u64::try_from(value.len()).unwrap();
                assert!(backend_bytes <= max_backend_bytes && backend_bytes <= max_value_bytes);
                self.reads.fetch_add(1, Ordering::SeqCst);
                assert_auxiliary_receipts(true);
                // Configuration and the bounded provider read precede the TLS
                // fault; key parsing and the real handler still run normally.
                if let Some(size) = self.deny_size {
                    alloc_track::arm_denial(size);
                }
                Ok(BoundedStoreRead {
                    backend_bytes,
                    value: Some(value),
                })
            }
        }

        struct ReservationCase {
            method: Method,
            uri: String,
            headers: Vec<(&'static str, String)>,
            body: Vec<u8>,
            expected_json: Option<Value>,
            deny_size: usize,
            failure_status: StatusCode,
            failure_body: &'static [u8],
        }

        fn reservation_cases() -> Vec<ReservationCase> {
            let bidders: Vec<_> = (0_usize..9)
                .map(|index| format!("fake_reservation_{index}_{}", "x".repeat(83)))
                .collect();
            let debug: Vec<_> = bidders
                .iter()
                .map(|bidder| json!({"bidder": bidder, "error": "unknown bidder"}))
                .collect();
            let expected = json!({"status": "no_cookie", "bidder_status": [], "debug": debug});
            let raw_size = serde_json::to_vec(&expected)
                .unwrap()
                .len()
                .checked_add(1)
                .unwrap();
            assert!(raw_size > 1024 && !raw_size.is_power_of_two());
            let mut cases = vec![ReservationCase {
                method: Method::POST,
                uri: "/cookie_sync".to_owned(),
                headers: vec![("content-type", "application/json".to_owned())],
                body: serde_json::to_vec(&json!({
                    "bidders": bidders, "debug": true, "gdpr": 0_u8, "coopSync": false
                }))
                .unwrap(),
                expected_json: Some(expected),
                deny_size: raw_size,
                failure_status: StatusCode::BAD_REQUEST,
                failure_body: b"response limit exceeded\n",
            }];
            let (cookie, raw, escaped) = html_json_fixture(1049, usersync_app::TEST_NOW);
            let header = format!(
                "uids={}",
                usersync_keys::encode_cookie(&cookie, &CookieLimits::default()).unwrap()
            );
            for deny_size in [raw.len(), escaped.len()] {
                cases.push(ReservationCase {
                    method: Method::GET,
                    uri: "/getuids".to_owned(),
                    headers: vec![("cookie", header.clone())],
                    body: Vec::new(),
                    expected_json: Some(serde_json::from_slice(&raw).unwrap()),
                    deny_size,
                    failure_status: StatusCode::INTERNAL_SERVER_ERROR,
                    failure_body: b"response limit exceeded",
                });
            }
            cases.push(ReservationCase {
                method: Method::GET,
                uri: live_catalog::redirect().setuid("uid=reservation-uid&f=b&gdpr=0"),
                headers: Vec::new(),
                body: Vec::new(),
                expected_json: None,
                deny_size: 4096,
                failure_status: StatusCode::BAD_REQUEST,
                failure_body: b"cookie resource limit exceeded",
            });
            cases
        }

        fn dispatch(
            app: &usersync_app::TestApp,
            case: &ReservationCase,
            deny_size: Option<usize>,
        ) -> ResponseEgressEnvelope {
            assert_auxiliary_receipts(false);
            let provider = Arc::new(ReservationSecrets {
                deny_size,
                reads: AtomicUsize::new(0),
            });
            let concrete = Arc::clone(&provider);
            let secrets: Arc<dyn SecretStore> = concrete;
            let registry: SecretRegistry = StoreRegistry::single_id(
                "default".to_owned(),
                BoundSecretStore::new(SecretHandle::new(secrets), "default".to_owned()),
            );
            let headers: Vec<_> = case
                .headers
                .iter()
                .map(|(name, value)| (*name, value.as_str()))
                .collect();
            let body = Body::from_bytes(case.body.clone());
            let config_reads = app.store().reads();
            let (envelope, measured) = measure(None, || {
                block_on(app.dispatch_envelope_with(
                    case.method.clone(),
                    &case.uri,
                    &headers,
                    body,
                    |request| {
                        request.extensions_mut().insert(registry);
                    },
                ))
            });
            assert_eq!(app.store().reads(), config_reads.checked_add(1).unwrap());
            assert_eq!(provider.reads.load(Ordering::SeqCst), 1, "no keyring retry");
            assert_eq!(
                app.secrets().reads(),
                0,
                "the request-bound provider must be used"
            );
            assert_eq!(
                measured.denied_requests,
                usize::from(deny_size.is_some()),
                "{measured:?}"
            );
            assert!(measured.requested_bytes > 0);
            assert_auxiliary_receipts(true);
            envelope
        }

        fn assert_success(mut envelope: ResponseEgressEnvelope, case: &ReservationCase) {
            let response = envelope.response_mut();
            assert_eq!(response.status(), StatusCode::OK);
            let body = usersync_app::body(response);
            if let Some(expected) = &case.expected_json {
                let mut actual: Value = serde_json::from_slice(body).unwrap();
                // The chooser shuffles diagnostics. Compare every independent
                // expected row by identity, without depending on RNG state.
                if let Some(debug) = actual.get_mut("debug").and_then(Value::as_array_mut) {
                    debug.sort_unstable_by(|left, right| {
                        left["bidder"]
                            .as_str()
                            .unwrap()
                            .cmp(right["bidder"].as_str().unwrap())
                    });
                }
                assert_eq!(actual, *expected);
                assert_eq!(body.last(), Some(&b'\n'));
                assert!(!response.headers().contains_key("set-cookie"));
            } else {
                assert!(body.is_empty());
                let field = response.headers()["set-cookie"].to_str().unwrap();
                assert!(field.len() <= 4096);
                let cookie = cookie::Cookie::parse(field).unwrap();
                let decoded =
                    usersync_keys::decode_cookie(cookie.value(), &CookieLimits::default()).unwrap();
                assert!(!decoded.corrupt);
                assert_eq!(
                    decoded.cookie.uid(live_catalog::redirect().family()),
                    Some("reservation-uid")
                );
            }
            assert_auxiliary_receipts(true);
            drop(envelope);
            assert_auxiliary_receipts(false);
        }

        #[test]
        fn allocation_refusals_keep_original_auxiliary_receipts_through_egress() {
            const CHILD: &str = "AUCTIONEER__TEST__SYNC_RESERVATION_CHILD";
            if var_os(CHILD).is_none() {
                let child = Command::new(current_exe().unwrap())
                    .args([
                        "--exact",
                        "tests::sync_reservation_egress::allocation_refusals_keep_original_auxiliary_receipts_through_egress",
                        "--nocapture",
                    ])
                    .env(CHILD, "1")
                    .output()
                    .unwrap();
                let output = format!(
                    "{}{}",
                    String::from_utf8_lossy(&child.stdout),
                    String::from_utf8_lossy(&child.stderr)
                );
                assert!(child.status.success(), "{output}");
                assert!(
                    output.contains("1 passed"),
                    "the child must execute: {output}"
                );
                return;
            }
            let app = usersync_app::TestApp::new(
                "schema_version = 1\n[host.user_sync]\nexternal_url = 'https://sync.example.test'\n",
            );
            let mut failures = 0_usize;
            for case in reservation_cases() {
                // Warm lazy shared state and prove the identical request reaches
                // successful output before targeting its one reservation.
                assert_success(dispatch(&app, &case, None), &case);
                for outcome in [
                    Some(ResponseEgressOutcome::Completed),
                    Some(ResponseEgressOutcome::RequestCancelled),
                    None,
                ] {
                    let mut envelope = dispatch(&app, &case, Some(case.deny_size));
                    let response = envelope.response_mut();
                    assert_eq!(response.status(), case.failure_status);
                    assert_eq!(usersync_app::body(response), case.failure_body);
                    assert_eq!(response.headers()["cache-control"], "no-store");
                    assert!(!response.headers().contains_key("set-cookie"));
                    let (prepared, _policy, mut attempt, clock) = envelope
                        .begin(ResponseEgressFramingMode::ExplicitBufferedLength)
                        .unwrap_or_else(|_| {
                            panic!("the complete refusal response must begin egress");
                        });
                    assert!(attempt.begin_writing());
                    drop(prepared);
                    assert_auxiliary_receipts(true);
                    if let Some(terminal) = outcome {
                        assert!(attempt.terminate(terminal, clock.now()));
                        assert_auxiliary_receipts(false);
                        assert!(!attempt.terminate(terminal, clock.now()));
                    }
                    drop(attempt);
                    assert_auxiliary_receipts(false);
                    failures = failures.checked_add(1).unwrap();
                }
                assert_success(dispatch(&app, &case, None), &case);
            }
            assert_eq!(
                failures, 12,
                "all four reservations and three egress endings must execute"
            );
        }
    }

    #[test]
    fn no_envelope_preserves_unbounded_serialization() {
        assert!(boot_gates().is_none());
        let response = escaped_response();
        assert_eq!(
            serialize_response(&response).expect("no envelope installed in this test binary"),
            serde_json::to_vec(&response).expect("reference serialization")
        );
    }
}
