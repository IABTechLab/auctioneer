//! End-to-end integration tests for `auctioneer_openrtb::prepare`.
//!
//! These exercise the public API only.

use auctioneer_openrtb::types::bid_request::BidRequest;

#[expect(
    clippy::expect_used,
    reason = "integration-test fixture inputs are hardcoded; parse must succeed"
)]
fn parse(json: serde_json::Value) -> BidRequest {
    serde_json::from_value(json).expect("fixture must parse")
}

#[cfg(test)]
mod tests {
    use super::parse;
    use auctioneer_openrtb::{
        NormalizedBidRequest, PrepareLimits, RequestHeaders, ValidationError, prepare,
    };

    /// Impression cap wide enough that the fixtures below never reach
    /// it; the cap itself is exercised by the max-impressions tests
    /// further down this file.
    const IMP_CAP_NOT_UNDER_TEST: PrepareLimits = PrepareLimits {
        max_impressions: 100,
        max_copied_header_bytes: 4_096,
    };

    #[test]
    fn prepare_defaults_channel_for_app() {
        let req = parse(serde_json::json!({"id":"r","imp":[{"id":"i","banner":{}}],"app":{}}));
        let normalized = prepare(req, &RequestHeaders::default(), &IMP_CAP_NOT_UNDER_TEST).unwrap();
        let name = normalized
            .ext
            .as_ref()
            .unwrap()
            .prebid
            .as_ref()
            .unwrap()
            .channel
            .as_ref()
            .unwrap()
            .name
            .as_deref();
        assert_eq!(name, Some("app"));
    }

    #[test]
    fn prepare_does_not_default_cur() {
        // No cur defaulting.
        let req = parse(serde_json::json!({"id":"r","imp":[{"id":"i","banner":{}}],"site":{}}));
        let normalized = prepare(req, &RequestHeaders::default(), &IMP_CAP_NOT_UNDER_TEST).unwrap();
        assert!(normalized.cur.is_empty());
    }

    #[test]
    fn prepare_does_not_emit_consent_warnings_in_m1() {
        // Consent parsers live in auctioneer-privacy. prepare() never
        // populates warnings in M1, even for a malformed us_privacy
        // that would later become an InvalidPrivacyConsent warning
        // emitted by PrivacyContext::resolve.
        let req = parse(serde_json::json!({
            "id": "r",
            "imp": [{"id": "i", "banner": {}}],
            "site": {},
            "regs": {"us_privacy": "NOPE"}
        }));
        let normalized: NormalizedBidRequest =
            prepare(req, &RequestHeaders::default(), &IMP_CAP_NOT_UNDER_TEST).unwrap();
        assert!(normalized.warnings().is_empty());
    }

    #[test]
    fn prepare_overrides_gpc_from_header_even_with_zero_body() {
        let req = parse(serde_json::json!({
            "id": "r",
            "imp": [{"id": "i", "banner": {}}],
            "site": {},
            "regs": {"ext": {"gpc": "0"}}
        }));
        let headers = RequestHeaders {
            sec_gpc: Some("1"),
            ..Default::default()
        };
        let normalized = prepare(req, &headers, &IMP_CAP_NOT_UNDER_TEST).unwrap();
        let gpc = normalized
            .regs
            .as_ref()
            .unwrap()
            .ext
            .as_ref()
            .unwrap()
            .gpc
            .as_deref();
        assert_eq!(gpc, Some("1"));
    }

    #[test]
    fn prepare_rejects_empty_channel_name() {
        let req = parse(serde_json::json!({
            "id": "r",
            "imp": [{"id": "i", "banner": {}}],
            "site": {},
            "ext": {"prebid": {"channel": {"name": ""}}}
        }));
        let err = prepare(req, &RequestHeaders::default(), &IMP_CAP_NOT_UNDER_TEST).unwrap_err();
        assert!(matches!(err, ValidationError::EmptyChannelName));
    }

    #[test]
    fn prepare_rejects_legacy_25_privacy() {
        let req = parse(serde_json::json!({
            "id": "r",
            "imp": [{"id": "i", "banner": {}}],
            "site": {},
            "regs": {"ext": {"gdpr": 1_i32}}
        }));
        let err = prepare(req, &RequestHeaders::default(), &IMP_CAP_NOT_UNDER_TEST).unwrap_err();
        assert!(matches!(
            err,
            ValidationError::Legacy25Privacy("regs.ext.gdpr")
        ));
    }

    #[test]
    fn prepare_succeeds_on_minimal_site_request() {
        let req = parse(serde_json::json!(
            {"id":"r","imp":[{"id":"i","banner":{"w":300_i32,"h":250_i32}}],"site":{}}
        ));
        let normalized = prepare(req, &RequestHeaders::default(), &IMP_CAP_NOT_UNDER_TEST).unwrap();
        assert_eq!(normalized.id, "r");
        assert_eq!(normalized.at, Some(1_i32));
        assert_eq!(
            normalized.site.as_ref().unwrap().ext.as_ref().unwrap().amp,
            Some(0_i32),
        );
        assert!(normalized.warnings().is_empty());
    }

    /// Builds a request JSON with `count` distinct imps, each with a
    /// unique id and a minimal banner.
    fn request_with_imps(count: usize) -> serde_json::Value {
        let imps: Vec<serde_json::Value> = (0..count)
            .map(|n| serde_json::json!({"id": format!("i{n}"), "banner": {"w": 300_i32, "h": 250_i32}}))
            .collect();
        serde_json::json!({"id": "r", "imp": imps, "site": {}})
    }

    #[test]
    fn prepare_accepts_exactly_the_limit() {
        let limits = PrepareLimits {
            max_impressions: 3,
            max_copied_header_bytes: 4_096,
        };
        let req = parse(request_with_imps(3));
        let normalized = prepare(req, &RequestHeaders::default(), &limits).unwrap();
        assert_eq!(normalized.imp.len(), 3);
    }

    #[test]
    fn prepare_rejects_over_limit_impressions_before_allocation() {
        let limits = PrepareLimits {
            max_impressions: 3,
            max_copied_header_bytes: 4_096,
        };
        // Built (and parsed) BEFORE the measurement window opens, so the
        // vec's own backing allocation never counts against the
        // reject-path budget being asserted below.
        let req = parse(request_with_imps(4));

        let before = alloc_track::allocated_on_this_thread();
        let err = prepare(req, &RequestHeaders::default(), &limits).unwrap_err();
        let allocated = alloc_track::allocated_on_this_thread().saturating_sub(before);

        assert_eq!(err, ValidationError::TooManyImpressions { got: 4, max: 3 });
        assert_eq!(
            allocated, 0,
            "step-0 impression-cap reject must not allocate, got {allocated} bytes"
        );
    }

    /// Test-scoped counting allocator (per-thread, cumulative) backing
    /// the reject-path zero-allocation assertion above. Delegates every
    /// operation to `System`.
    mod alloc_track {
        #![expect(
            unsafe_code,
            reason = "a GlobalAlloc impl is unavoidably unsafe; test-only \
                      measurement backing the step-0 reject-path allocation bound"
        )]

        use std::alloc::{GlobalAlloc, Layout, System};
        use std::cell::Cell;

        thread_local! {
            static ALLOCATED: Cell<u64> = const { Cell::new(0) };
        }

        pub fn allocated_on_this_thread() -> u64 {
            ALLOCATED.try_with(Cell::get).unwrap_or(0)
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

        pub struct CountingAllocator;

        // SAFETY: every operation delegates verbatim to `System`; the
        // counting side effect touches only a thread-local counter and
        // never the returned memory.
        unsafe impl GlobalAlloc for CountingAllocator {
            unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
                count(layout.size());
                // SAFETY: same layout contract as the caller's.
                unsafe { System.alloc(layout) }
            }

            unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
                // SAFETY: ptr/layout come from a matching System alloc.
                unsafe { System.dealloc(ptr, layout) }
            }

            unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
                count(layout.size());
                // SAFETY: same layout contract as the caller's.
                unsafe { System.alloc_zeroed(layout) }
            }

            unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
                count(new_size.saturating_sub(layout.size()));
                // SAFETY: ptr/layout come from a matching System alloc;
                // new_size is the caller's requested size.
                unsafe { System.realloc(ptr, layout, new_size) }
            }
        }
    }

    #[global_allocator]
    static COUNTING_ALLOC: alloc_track::CountingAllocator = alloc_track::CountingAllocator;
}
