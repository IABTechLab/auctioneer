//! Native generated-App allocation evidence, not helper-only codec measurements.
//! Fixtures, runtime-config generation and boot are outside each measured call.
//! URI/header values and body are prepared first; final Request construction
//! inside shared `TestApp::dispatch` is conservatively included in the counts.
//! The input Body is kept alive so dropping it cannot mask handler allocations.
//! Direct catalog/chooser scopes exclude boot, config and privacy preparation,
//! but retain the complete constructed catalog through selection.

#[cfg(test)]
use auctioneer_core::test_support::live_catalog;

#[cfg(test)]
pub use auctioneer_core::test_support::bidder_fixture;

use auctioneer_runtime::test_support::alloc_track;

#[expect(
    dead_code,
    reason = "shared support is used by multiple integration targets"
)]
mod usersync_app;

use auctioneer_core::test_support::usersync_keys;

#[global_allocator]
static ALLOCATOR: allocation_counts::CountingAllocator = allocation_counts::CountingAllocator;

// Keep the existing live/peak counter and add cumulative work measurements.
mod allocation_counts {
    #![expect(
        unsafe_code,
        reason = "test-only GlobalAlloc delegation to the shared counter"
    )]

    use std::{
        alloc::{GlobalAlloc, Layout},
        cell::Cell,
    };

    use super::alloc_track;

    thread_local! {
        static CALLS: Cell<u64> = const { Cell::new(0) };
        // Requested allocation bytes, including each realloc's complete new
        // size. Unlike the live peak, this intentionally counts repeated work.
        static BYTES: Cell<u64> = const { Cell::new(0) };
    }

    pub fn totals() -> (u64, u64) {
        (CALLS.with(Cell::get), BYTES.with(Cell::get))
    }

    fn count(bytes: usize) {
        let _calls = CALLS.try_with(|calls| calls.set(calls.get().saturating_add(1)));
        let _bytes = BYTES.try_with(|total| {
            total.set(
                total
                    .get()
                    .saturating_add(u64::try_from(bytes).unwrap_or(u64::MAX)),
            );
        });
    }

    pub struct CountingAllocator;

    // SAFETY: all operations preserve the shared allocator's pointer/layout
    // contract; allocation-free TLS counters do not touch allocated storage.
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            // SAFETY: the caller's unchanged layout is delegated.
            let ptr = unsafe { alloc_track::CountingAllocator.alloc(layout) };
            if !ptr.is_null() {
                count(layout.size());
            }
            ptr
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            // SAFETY: the caller's unchanged layout is delegated.
            let ptr = unsafe { alloc_track::CountingAllocator.alloc_zeroed(layout) };
            if !ptr.is_null() {
                count(layout.size());
            }
            ptr
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: ptr/layout came from a matching delegated allocation.
            unsafe { alloc_track::CountingAllocator.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            // SAFETY: ptr/layout and the requested size are delegated unchanged.
            let result = unsafe { alloc_track::CountingAllocator.realloc(ptr, layout, new_size) };
            if !result.is_null() {
                count(new_size);
            }
            result
        }
    }
}

#[cfg(test)]
mod bounded_store;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::hint::black_box;

    use super::bidder_fixture::{fake_bidder::FakeBidder, registry as synthetic_registry};
    use auctioneer_bidder::{BidderName, BidderRegistry};
    use auctioneer_config::types::{AccountConfig, HostConfig};
    use auctioneer_core::{
        config::{AuctioneerAppConfig, HostCookieOverride, UserSyncOverride},
        host_primitives::boot_gates,
        usersync::{
            catalog::Catalog,
            choose::{ChooseInput, FilterSettings, choose},
            cookie::{
                CookieWriteError, CookieWriteOptions, UidCookie, decode_cookie,
                prepare_set_cookie_value,
            },
            evict::FamilyPriorities,
            limits::CookieLimits,
            wire::FilterSettingsWire,
        },
    };
    use auctioneer_privacy::{
        GdprSignalIn, SyncEndpoint, SyncPrivacyInput, gcl::Gcl, gvl::GvlStore, resolve_sync_privacy,
    };
    use auctioneer_runtime::{
        SeededRng,
        aux_work::{AuxWorkCharges, AuxWorkLimits, derive_aux_work},
        bounded_json::NODE_RETAINED_BYTES,
    };
    use base64::{
        Engine as _,
        engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
    };
    use edgezero_core::{body::Body, http::Response};
    use http::{Method, StatusCode};
    use serde_json::{Value, json};

    use super::{alloc_track, allocation_counts, live_catalog, usersync_app, usersync_keys};

    // Config extraction, baked host/account clones, registry/context, the test
    // store binding, policy headers and native dispatch bookkeeping. The empty
    // real-App control measured 19,279 bytes; round to 20 KiB (1,201 bytes
    // headroom), independently checked below. NOT the config/privacy envelope.
    const FIXTURE_BYTES: u64 = 20 * 1024;
    const RESPONSE_HEADERS: u64 = 1024;

    fn bytes(n: usize) -> u64 {
        u64::try_from(n).unwrap()
    }

    fn narrow_limits() -> AuxWorkLimits {
        let mut limits = boot_gates().unwrap().aux_work_derived.limits();
        limits.config_envelope_bytes = 0;
        limits.config_blob_bytes = 0;
        limits.baked_config_bytes = 0;
        limits.config_json_values = 0;
        limits.body_bytes = 0;
        limits.body_node_bytes = 0;
        limits.query_bytes = 0;
        limits.query_parameters = 0;
        limits.admitted_header_bytes = 0;
        limits.admitted_header_count = 0;
        limits.cookie_bytes = 0;
        limits.cookie_node_bytes = 0;
        limits.privacy_input_bytes = 0;
        limits.selected_tcf_bytes = 0;
        limits.catalog_names = 0;
        limits.priority_groups = 0;
        limits.priority_entries = 0;
        limits.requested_names = 0;
        limits.filter_names_per_list = 0;
        limits.debug_rows = 0;
        limits.url_bytes = 0;
        limits.output_body_bytes = 0;
        limits.response_header_bytes = RESPONSE_HEADERS;
        limits.response_header_count = 8;
        limits.set_cookie_bytes = 0;
        limits
    }

    #[derive(Clone, Copy)]
    struct AllocationCount {
        peak: u64,
        calls: u64,
        allocated: u64,
    }

    fn measure(
        app: &usersync_app::TestApp,
        method: Method,
        uri: &str,
        headers: &[(&str, &str)],
        raw: Vec<u8>,
    ) -> (Response, AllocationCount) {
        let retained = bytes::Bytes::from(raw);
        let retained_body = Body::from_bytes(retained.clone());
        assert_eq!(
            retained_body
                .as_bytes()
                .expect("buffered fixture body")
                .as_ptr(),
            retained.as_ptr(),
            "the retained fixture must share the dispatched input storage"
        );
        let body = Body::from_bytes(retained);
        let reads = app.store().reads();
        let secret_reads = app.secrets().reads();
        let baseline = alloc_track::reset_peak();
        let (calls, allocated) = allocation_counts::totals();
        let response = app.dispatch(method, uri, headers, body);
        let peak = alloc_track::peak_since(baseline);
        let (after_calls, after_allocated) = allocation_counts::totals();
        drop(retained_body);
        assert_eq!(
            app.store().reads(),
            reads.checked_add(1).unwrap(),
            "one real config extraction"
        );
        assert_eq!(
            app.secrets().reads(),
            secret_reads.checked_add(1).unwrap(),
            "one bounded keyring read"
        );
        (
            response,
            AllocationCount {
                peak,
                calls: after_calls.checked_sub(calls).unwrap(),
                allocated: after_allocated.checked_sub(allocated).unwrap(),
            },
        )
    }

    #[expect(
        clippy::print_stderr,
        reason = "print numeric allocation evidence, never request or secret contents"
    )]
    fn print_allocation(
        label: &str,
        count: AllocationCount,
        relevant: u64,
        keyring: u64,
        fixture: u64,
        budget: u64,
        funded_budget: u64,
    ) {
        let AllocationCount {
            peak,
            calls,
            allocated,
        } = count;
        eprintln!(
            "{label}: peak={peak} calls={calls} allocated={allocated} relevant_aux={relevant} keyring_aux={keyring} fixture={fixture} budget={budget} funded_budget={funded_budget}"
        );
    }

    fn guard(label: &str, count: AllocationCount, relevant: u64, keyring_bytes: usize) {
        // Input-aware counterpart of the runtime's 4096*(1+128) keyring term,
        // eight (32-byte key + 32-byte ID + 128-byte entry) slots and MAC work.
        // The full production signing allowance is checked separately. The
        // complete cookie/URL intermediates are charged separately below.
        let keyring = keyring_input_terms(keyring_bytes, 8);
        let funded_signing = boot_gates().unwrap().aux_work_derived.charges().signing;
        assert_eq!(funded_signing, keyring_input_terms(4096, 8));
        assert!(keyring <= funded_signing);
        let budget = sum_terms([FIXTURE_BYTES, relevant, keyring]);
        let funded_budget = sum_terms([FIXTURE_BYTES, relevant, funded_signing]);
        print_allocation(
            label,
            count,
            relevant,
            keyring,
            FIXTURE_BYTES,
            budget,
            funded_budget,
        );
        assert!(
            count.calls > 0 && count.allocated >= count.peak,
            "the real handler must allocate inside measurement"
        );
        assert!(
            count.peak <= budget,
            "{label}: handler peak {} exceeds {budget} bytes",
            count.peak
        );
        assert!(count.peak <= funded_budget);
    }

    fn payload(families: usize, uid_bytes: usize) -> Value {
        let entries: BTreeMap<_, _> = (0..families)
            .map(|index| {
                (
                    format!("f{index:03}"),
                    json!({"uid": "u".repeat(uid_bytes), "expires": "2026-10-14T12:00:00Z"}),
                )
            })
            .collect();
        json!({"tempUIDs": entries})
    }

    fn cookie_header(payload: &Value) -> String {
        // Payload generation is independent of the production cookie codec.
        // Decode and fixture signing both happen before the real-App scope.
        let encoded = URL_SAFE.encode(serde_json::to_vec(payload).unwrap());
        let decoded = decode_cookie(&encoded, &CookieLimits::default()).unwrap();
        assert!(!decoded.corrupt);
        format!(
            "uids={}",
            usersync_keys::encode_cookie(&decoded.cookie, &CookieLimits::default()).unwrap()
        )
    }

    #[expect(
        clippy::arithmetic_side_effects,
        reason = "independent wire equations use only the small fixed fixture dimensions"
    )]
    fn cookie_charges(
        families: usize,
        uid_bytes: usize,
        header_bytes: usize,
        query: &str,
        writing: bool,
    ) -> AuxWorkCharges {
        let mut limits = narrow_limits();
        // Independent ASCII wire bound: a four-byte family, UID, timestamp,
        // JSON punctuation/keys per entry, plus the tempUIDs wrapper.
        let decoded_bound = 32 + (families + usize::from(writing)) * (uid_bytes + 64);
        // A growable Vec may retain almost twice its written length, including
        // when a large write changes the geometric growth sequence. Fund the
        // three codec stages by capacity, not by serialized length alone.
        limits.cookie_bytes = bytes(header_bytes).max(bytes(decoded_bound)) * 2;
        assert!(
            limits.cookie_bytes <= boot_gates().unwrap().aux_work_derived.limits().cookie_bytes
        );
        limits.admitted_header_bytes = bytes(header_bytes);
        limits.admitted_header_count = u32::from(header_bytes > 0);
        limits.cookie_node_bytes =
            bytes(3 + 6 * (families + usize::from(writing))) * NODE_RETAINED_BYTES;
        limits.query_bytes = bytes(query.len());
        limits.query_parameters = if writing { 4 } else { 0 };
        limits.catalog_names = if writing { 4 } else { 0 };
        limits.priority_groups = u32::from(writing);
        limits.priority_entries = u32::from(writing);
        if writing {
            // Complete signed envelope and attributes get 256 bytes, without
            // relying on the production formatter to specify its own bound.
            limits.set_cookie_bytes = bytes(decoded_bound.div_ceil(3) * 4 + 256);
        } else {
            // Raw JSON and HTML-escaped output coexist. ASCII fixtures do not
            // expand, but geometric Vec growth needs a power-of-two capacity.
            let output_bound = (32 + families * (uid_bytes + 16)).next_power_of_two();
            limits.output_body_bytes = bytes(output_bound);
        }
        derive_aux_work(limits).unwrap().charges()
    }

    fn cookie_relevant(charges: AuxWorkCharges) -> u64 {
        sum_terms([
            charges.input_headers,
            charges.query,
            charges.cookie,
            charges.nodes,
            charges.chooser,
            charges.output,
        ])
    }

    fn sum_terms<const N: usize>(terms: [u64; N]) -> u64 {
        terms.into_iter().try_fold(0_u64, u64::checked_add).unwrap()
    }

    fn keyring_input_terms(source_bytes: usize, keys: usize) -> u64 {
        sum_terms([
            bytes(source_bytes).checked_mul(129).unwrap(),
            bytes(keys).checked_mul(32 + 32 + 128).unwrap(),
            512,
        ])
    }

    #[test]
    fn oversized_cookie_preflight_rejects_without_formatter_allocations() {
        let now = time::macros::datetime!(2026-10-14 12:00 UTC);
        for options in [
            CookieWriteOptions::default(),
            CookieWriteOptions {
                domain: "sync.example.com",
                user_agent: Some("Chrome/130.0"),
                ..CookieWriteOptions::default()
            },
        ] {
            let validation_baseline = alloc_track::reset_peak();
            let validation_before = allocation_counts::totals();
            let name = http::HeaderName::from_bytes(options.name.as_bytes()).unwrap();
            let validation_after = allocation_counts::totals();
            let validation_peak = alloc_track::peak_since(validation_baseline);
            let validation_calls = validation_after.0.checked_sub(validation_before.0).unwrap();
            let validation_bytes = validation_after.1.checked_sub(validation_before.1).unwrap();
            assert_eq!(name.as_str(), options.name);
            drop(name);
            for input_bytes in [4097, 64 * 1024] {
                let encoded = "a".repeat(input_bytes);
                assert!(encoded.len() > options.hard_cap_bytes);
                let baseline = alloc_track::reset_peak();
                let before = allocation_counts::totals();
                let prepared = prepare_set_cookie_value(&encoded, &options, now);
                let after = allocation_counts::totals();
                let peak = alloc_track::peak_since(baseline);
                assert_eq!(prepared.unwrap_err(), CookieWriteError::HardLimit);
                assert_eq!(
                    after.0.checked_sub(before.0).unwrap(),
                    validation_calls,
                    "preflight must not format {input_bytes} bytes"
                );
                assert_eq!(
                    after.1.checked_sub(before.1).unwrap(),
                    validation_bytes,
                    "only bounded cookie-name validation may allocate"
                );
                assert_eq!(peak, validation_peak);
            }
        }

        // Prove the counter observes the formatting path for an accepted value.
        let options = CookieWriteOptions::default();
        let baseline = alloc_track::reset_peak();
        let before = allocation_counts::totals();
        let prepared = prepare_set_cookie_value("e30=", &options, now).unwrap();
        let after = allocation_counts::totals();
        assert!(after.0 > before.0 && after.1 > before.1);
        assert!(alloc_track::peak_since(baseline) > 0);
        assert!(prepared.field_value().as_bytes().len() <= options.hard_cap_bytes);
    }

    #[expect(
        clippy::print_stderr,
        reason = "record full-handler cross-input peak deltas"
    )]
    fn delta_guard(
        label: &str,
        reference: &mut Option<(AllocationCount, u64)>,
        count: AllocationCount,
        relevant: u64,
    ) {
        if let Some((small, small_terms)) = *reference {
            let term_delta = relevant.checked_sub(small_terms).unwrap();
            // One formatting/header leaf, not another fixture or signing
            // allowance. Growing input terms must otherwise fund the delta.
            let budget = sum_terms([small.peak, term_delta, 512]);
            eprintln!(
                "{label}: peak_delta={} aux_delta={term_delta} delta_scratch=512",
                count.peak.saturating_sub(small.peak)
            );
            assert!(
                count.peak <= budget,
                "{label}: input growth peaked at {} against delta bound {budget}",
                count.peak
            );
        } else {
            *reference = Some((count, relevant));
        }
    }

    #[test]
    fn fixed_fixture_overhead_is_checked_without_uid_or_chooser_work() {
        let app = usersync_app::TestApp::new("");
        let (response, count) = measure(&app, Method::GET, "/getuids", &[], Vec::new());
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(usersync_app::body(&response), b"{}\n");
        guard("empty fixture", count, 0, usersync_keys::SECRET_JSON.len());
    }

    #[test]
    fn generated_app_cookie_decode_and_readback_scale_with_uid_bytes_and_families() {
        let app = usersync_app::TestApp::new("");
        let mut uid_reference = None;
        let mut family_reference = None;
        for (families, uid_bytes) in [
            (4, 16),
            (4, 256),
            (4, 512),
            (1, 32),
            (8, 32),
            (16, 32),
            (24, 32),
        ] {
            let header = cookie_header(&payload(families, uid_bytes));
            let charges = cookie_charges(families, uid_bytes, header.len(), "", false);
            let (response, peak) = measure(
                &app,
                Method::GET,
                "/getuids",
                &[("cookie", &header)],
                Vec::new(),
            );
            assert_eq!(response.status(), StatusCode::OK);
            let decoded: Value = serde_json::from_slice(usersync_app::body(&response)).unwrap();
            let uids = decoded["buyeruids"].as_object().unwrap();
            assert_eq!(uids.len(), families);
            assert!(
                uids.values()
                    .all(|uid| uid.as_str().unwrap().len() == uid_bytes)
            );
            assert!(!response.headers().contains_key("set-cookie"));
            let reference = if families == 4 {
                &mut uid_reference
            } else {
                &mut family_reference
            };
            delta_guard(
                "decode input growth",
                reference,
                peak,
                cookie_relevant(charges),
            );
            guard(
                &format!("decode families={families} uid={uid_bytes}"),
                peak,
                cookie_relevant(charges),
                usersync_keys::SECRET_JSON.len(),
            );
        }
    }

    fn write_config(cap: usize) -> String {
        let config = AuctioneerAppConfig {
            host_cookie: Some(HostCookieOverride {
                max_cookie_size_bytes: Some(cap),
                ..HostCookieOverride::default()
            }),
            user_sync: Some(UserSyncOverride {
                priority_groups: Some(vec![vec![live_catalog::redirect().name().to_owned()]]),
                ..UserSyncOverride::default()
            }),
            ..AuctioneerAppConfig::default()
        };
        toml::to_string(&config).unwrap()
    }

    fn read_written(app: &usersync_app::TestApp, response: &Response) -> Value {
        let field = response.headers()["set-cookie"].to_str().unwrap();
        let header = field.split(';').next().unwrap();
        let readback = app.dispatch(
            Method::GET,
            "/getuids",
            &[("cookie", header)],
            Body::empty(),
        );
        assert_eq!(readback.status(), StatusCode::OK);
        serde_json::from_slice(usersync_app::body(&readback)).unwrap()
    }

    #[test]
    fn generated_app_cookie_update_clone_and_reencode_scale_independently() {
        let app = usersync_app::TestApp::new(&write_config(0));
        let mut uid_reference = None;
        let mut family_reference = None;
        for (families, uid_bytes) in [
            (4, 16),
            (4, 256),
            (4, 384),
            (1, 32),
            (8, 32),
            (16, 32),
            (24, 32),
        ] {
            let header = cookie_header(&payload(families, uid_bytes));
            let query = format!(
                "bidder={}&uid={}&f=b&gdpr=0",
                live_catalog::redirect().name(),
                "n".repeat(uid_bytes)
            );
            let uri = format!("/setuid?{query}");
            let charges = cookie_charges(families, uid_bytes, header.len(), &query, true);
            let (response, peak) =
                measure(&app, Method::GET, &uri, &[("cookie", &header)], Vec::new());
            assert_eq!(response.status(), StatusCode::OK);
            assert!(response.headers()["set-cookie"].as_bytes().len() <= 4096);
            let decoded = read_written(&app, &response);
            assert_eq!(
                decoded["buyeruids"].as_object().unwrap().len(),
                families + 1
            );
            assert_eq!(
                decoded["buyeruids"][live_catalog::redirect().family()],
                "n".repeat(uid_bytes)
            );
            assert_eq!(decoded["buyeruids"]["f000"], "u".repeat(uid_bytes));
            let reference = if families == 4 {
                &mut uid_reference
            } else {
                &mut family_reference
            };
            delta_guard(
                "write input growth",
                reference,
                peak,
                cookie_relevant(charges),
            );
            guard(
                &format!("write families={families} uid={uid_bytes}"),
                peak,
                cookie_relevant(charges),
                usersync_keys::SECRET_JSON.len(),
            );
        }
    }

    #[test]
    fn generated_app_repeated_eviction_is_bounded_by_live_cookie_not_attempts() {
        let app = usersync_app::TestApp::new(&write_config(500));
        let mut reference = None;
        for families in [4, 8, 12] {
            let header = cookie_header(&payload(families, 128));
            let query = format!(
                "bidder={}&uid={}&f=b&gdpr=0",
                live_catalog::redirect().name(),
                "n".repeat(128)
            );
            let uri = format!("/setuid?{query}");
            let charges = cookie_charges(families, 128, header.len(), &query, true);
            let (response, peak) =
                measure(&app, Method::GET, &uri, &[("cookie", &header)], Vec::new());
            assert_eq!(response.status(), StatusCode::OK);
            assert!(response.headers()["set-cookie"].as_bytes().len() <= 500);
            let decoded = read_written(&app, &response);
            assert_eq!(
                decoded["buyeruids"],
                json!({(live_catalog::redirect().family()): "n".repeat(128)})
            );
            delta_guard(
                "eviction input growth",
                &mut reference,
                peak,
                cookie_relevant(charges),
            );
            guard(
                &format!("evict families={families}"),
                peak,
                cookie_relevant(charges),
                usersync_keys::SECRET_JSON.len(),
            );
        }
    }

    fn nodes(value: &Value) -> u64 {
        let descendants = match value {
            Value::Array(items) => items.iter().map(nodes).sum::<u64>(),
            Value::Object(items) => items
                .values()
                .map(|child| nodes(child).checked_add(1).unwrap())
                .sum::<u64>(),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => 0,
        };
        descendants.checked_add(1).unwrap()
    }

    fn cooperative_fixture(
        alias_count: usize,
        name_bytes: usize,
        consent_bytes: usize,
    ) -> (String, Value) {
        let aliases: BTreeMap<_, _> = (0..alias_count)
            .map(|index| {
                let prefix = format!(
                    "fake_{}{:02}",
                    char::from(
                        b'a'.checked_add(u8::try_from(index.checked_div(100).unwrap()).unwrap())
                            .unwrap()
                    ),
                    index.checked_rem(100).unwrap()
                );
                (
                    format!(
                        "{prefix}{}",
                        "a".repeat(name_bytes.checked_sub(prefix.len()).unwrap())
                    ),
                    live_catalog::redirect().name().to_owned(),
                )
            })
            .collect();
        let mut bidders = vec![
            live_catalog::redirect().name().to_owned(),
            live_catalog::iframe().name().to_owned(),
        ];
        bidders.extend(aliases.keys().cloned());
        let config = AuctioneerAppConfig {
            aliases: Some(aliases),
            user_sync: Some(UserSyncOverride {
                priority_groups: Some(vec![bidders.clone()]),
                ..UserSyncOverride::default()
            }),
            ..AuctioneerAppConfig::default()
        };
        let body = json!({
            "bidders": bidders,
            "gdpr": 0_i32,
            "gdpr_consent": "a".repeat(consent_bytes),
            "coopSync": true,
            "debug": true,
            "filterSettings": {
                "iframe": {"filter": "include", "bidders": bidders},
                "image": {"filter": "include", "bidders": bidders},
            },
        });
        (toml::to_string(&config).unwrap(), body)
    }

    fn catalog_host(catalog_names: usize, name_bytes: usize, mixed_case: bool) -> HostConfig {
        let alias_count = catalog_names.checked_sub(1).unwrap();
        let aliases = (0..alias_count)
            .map(|index| {
                let prefix = format!(
                    "fake_{}{:02}",
                    char::from(
                        b'a'.checked_add(u8::try_from(index.checked_div(100).unwrap()).unwrap())
                            .unwrap()
                    ),
                    index.checked_rem(100).unwrap()
                );
                let mut name = format!(
                    "{prefix}{}",
                    "a".repeat(name_bytes.checked_sub(prefix.len()).unwrap())
                );
                if mixed_case {
                    name = name
                        .chars()
                        .enumerate()
                        .map(|(offset, ch)| {
                            if offset >= "fake_".len() {
                                ch.to_ascii_uppercase()
                            } else {
                                ch
                            }
                        })
                        .collect();
                }
                assert_eq!(name.len(), name_bytes);
                (name, "fake_aaa".to_owned())
            })
            .collect();
        HostConfig {
            aliases,
            ..HostConfig::default()
        }
    }

    struct CatalogChooserFixture {
        registry: BidderRegistry,
        names: Vec<BidderName>,
        app: AuctioneerAppConfig,
        account: AccountConfig,
        gvl: GvlStore,
        gcl: Gcl,
        cookie: UidCookie,
        compiled_configured: usize,
    }

    impl CatalogChooserFixture {
        fn new() -> Self {
            let app = AuctioneerAppConfig::default();
            let account = AccountConfig::default();
            let mut registry = BidderRegistry::new();
            let (bidder, _) = synthetic_registry().get("fake_aaa").unwrap();
            registry
                .register(bidder, Box::<FakeBidder>::default())
                .expect("unique test bidder");
            let names = vec![BidderName::new(bidder.name.as_str())];
            let host = HostConfig::default();
            let primed = Catalog::from_registry(&host, &app, &registry).unwrap();
            let compiled_configured = primed.available_names().len();
            drop(primed);
            Self {
                registry,
                names,
                app,
                account,
                gvl: GvlStore::for_test(vec![]),
                gcl: Gcl::for_test(vec![], usersync_app::TEST_NOW),
                cookie: UidCookie::default(),
                compiled_configured,
            }
        }
    }

    struct CatalogChooserCall<'a> {
        lower_name: &'a str,
        cooperative: bool,
        debug: bool,
        requested: Vec<String>,
        groups: Vec<Vec<String>>,
        wire: FilterSettingsWire,
    }

    impl<'a> CatalogChooserCall<'a> {
        fn new(lower_name: &'a str, mode: &str) -> Self {
            let cooperative = mode == "cooperative";
            let debug = mode == "requested_debug" || cooperative;
            let requested = if mode == "fallback" {
                Vec::new()
            } else {
                vec![lower_name.to_owned()]
            };
            let groups = if cooperative {
                vec![vec![lower_name.to_owned()]]
            } else {
                Vec::new()
            };
            let wire = serde_json::from_value(if cooperative {
                json!({
                    "iframe": {"filter": "include", "bidders": [lower_name]},
                    "image": {"filter": "include", "bidders": [lower_name]},
                })
            } else {
                json!({})
            })
            .unwrap();
            Self {
                lower_name,
                cooperative,
                debug,
                requested,
                groups,
                wire,
            }
        }

        fn budgets(&self, catalog_names: usize) -> (u64, u64) {
            let mut limits = narrow_limits();
            limits.catalog_names = u32::try_from(catalog_names).unwrap();
            limits.requested_names = u32::try_from(self.requested.len()).unwrap();
            limits.priority_groups = u32::from(self.cooperative);
            limits.priority_entries = u32::from(self.cooperative);
            limits.filter_names_per_list = u32::from(self.cooperative);
            limits.debug_rows = u32::from(self.debug);
            let catalog_budget = bytes(catalog_names).checked_mul(256).unwrap();
            let budget = sum_terms([
                catalog_budget,
                bytes(self.requested.len()).checked_mul(256).unwrap(),
                u64::from(self.cooperative).checked_mul(256).unwrap(),
                u64::from(self.cooperative).checked_mul(512).unwrap(),
                u64::from(self.debug).checked_mul(256).unwrap(),
                u64::from(self.cooperative).checked_mul(128).unwrap(),
            ]);
            assert_eq!(derive_aux_work(limits).unwrap().charges().chooser, budget);
            (catalog_budget, budget)
        }

        fn measure(
            &self,
            fixture: &CatalogChooserFixture,
            host: &HostConfig,
        ) -> [AllocationCount; 2] {
            let now = usersync_app::TEST_NOW;
            let mut privacy = resolve_sync_privacy(SyncPrivacyInput {
                gdpr: GdprSignalIn::CookieNumber(0),
                consent: None,
                us_privacy: None,
                gpp: None,
                gpp_sid: None,
                account: &fixture.account,
                host,
                endpoint: SyncEndpoint::CookieSync,
                gvl: &fixture.gvl,
                gcl: &fixture.gcl,
                now,
            })
            .unwrap();
            let rng = SeededRng::new(17);
            let baseline = alloc_track::reset_peak();
            let before = allocation_counts::totals();
            let snapshot = || {
                let after = allocation_counts::totals();
                AllocationCount {
                    peak: alloc_track::peak_since(baseline),
                    calls: after.0.checked_sub(before.0).unwrap(),
                    allocated: after.1.checked_sub(before.1).unwrap(),
                }
            };
            let catalog = Catalog::from_registry(host, &fixture.app, &fixture.registry).unwrap();
            let catalog_count = snapshot();
            let filters = FilterSettings::from_wire(&self.wire).unwrap();
            let result = choose(
                ChooseInput {
                    catalog: &catalog,
                    requested: &self.requested,
                    cooperative: self.cooperative,
                    priority_groups: &self.groups,
                    filters: &filters,
                    cookie: &fixture.cookie,
                    now,
                    limit: Some(1),
                    debug: self.debug,
                },
                &mut privacy,
                &rng,
            )
            .unwrap();
            black_box((&catalog, &filters, &result));
            let combined_count = snapshot();

            assert_eq!(
                catalog.available_names().len(),
                host.aliases
                    .len()
                    .checked_add(fixture.compiled_configured)
                    .unwrap()
            );
            assert_eq!(result.selected.len(), 1);
            assert_eq!(result.debug.len(), usize::from(self.debug));
            if !self.requested.is_empty() {
                assert_eq!(result.selected[0].name, self.lower_name);
                assert_eq!(result.selected[0].normalized_name, self.lower_name);
            }
            assert_eq!(
                catalog.lookup(self.lower_name).unwrap().catalog_name,
                host.aliases.keys().next().unwrap()
            );
            drop(result);
            drop(filters);
            drop(catalog);
            [catalog_count, combined_count]
        }

        fn measure_family_projection(
            &self,
            fixture: &CatalogChooserFixture,
            host: &HostConfig,
        ) -> AllocationCount {
            let baseline = alloc_track::reset_peak();
            let before = allocation_counts::totals();
            let catalog = Catalog::from_registry(host, &fixture.app, &fixture.registry).unwrap();
            let priorities = FamilyPriorities::from_catalog(&self.groups, &catalog).unwrap();
            black_box((&catalog, &priorities));
            let after = allocation_counts::totals();
            let count = AllocationCount {
                peak: alloc_track::peak_since(baseline),
                calls: after.0.checked_sub(before.0).unwrap(),
                allocated: after.1.checked_sub(before.1).unwrap(),
            };
            assert_eq!(priorities.rank("fam_a"), self.cooperative.then_some(0));
            count
        }
    }

    #[test]
    fn catalog_and_chooser_peak_is_funded_with_limit_one_and_small_callers() {
        let _serial = usersync_app::TestApp::new("");
        let fixture = CatalogChooserFixture::new();
        let mut cases = 0_u64;
        let mut stages = 0_u64;
        let mut over_budget = 0_u64;
        for catalog_names in [128_usize, 256, 512] {
            for name_bytes in [8_usize, 64, 128] {
                for mixed_case in [false, true] {
                    let host = catalog_host(catalog_names, name_bytes, mixed_case);
                    assert!(
                        fixture.app.aliases.is_none(),
                        "aliases are baked, not an overlay"
                    );
                    fixture
                        .app
                        .validate_effective_with_catalog(&host, &fixture.account, &fixture.names)
                        .unwrap();
                    let alias = host.aliases.keys().next().unwrap();
                    let lower_name = alias.to_ascii_lowercase();
                    assert_eq!(alias != &lower_name, mixed_case);
                    for mode in ["fallback", "requested", "requested_debug", "cooperative"] {
                        let call = CatalogChooserCall::new(&lower_name, mode);
                        let (catalog_budget, budget) = call.budgets(catalog_names);
                        let label = format!(
                            "catalog names={catalog_names} name_bytes={name_bytes} mixed={mixed_case} caller={} mode={mode} limit=1",
                            call.requested.len()
                        );
                        let [catalog_count, combined_count] = call.measure(&fixture, &host);
                        let projection_count = call.measure_family_projection(&fixture, &host);
                        let projection_budget = sum_terms([
                            catalog_budget,
                            u64::from(call.cooperative).checked_mul(384).unwrap(),
                        ]);
                        cases = cases.checked_add(1).unwrap();
                        for (stage, count, bound) in [
                            ("construction", catalog_count, catalog_budget),
                            ("through_choose", combined_count, budget),
                            ("through_projection", projection_count, projection_budget),
                        ] {
                            stages = stages.checked_add(1).unwrap();
                            print_allocation(
                                &format!("{label} stage={stage}"),
                                count,
                                bound,
                                0,
                                0,
                                bound,
                                bound,
                            );
                            assert!(count.calls > 0 && count.allocated >= count.peak);
                            if count.peak > bound {
                                over_budget = over_budget.checked_add(1).unwrap();
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 72, "all catalog/caller cases must execute");
        assert_eq!(stages, 216, "all allocation stages must execute per case");
        assert_eq!(
            over_budget, 0,
            "catalog/chooser peaks exceed fixed owner charges; see numeric evidence"
        );
    }

    #[test]
    fn generated_app_cooperative_chooser_and_retained_urls_scale_independently() {
        // Only the two real compiled sync families produce URLs. Aliases must
        // exercise requested/priority/filter/debug storage without inventing
        // dozens of unsupported independent bidders or disabling deduplication.
        let mut alias_reference = None;
        let mut name_reference = None;
        let mut url_reference = None;
        for (aliases, name_bytes, consent_bytes) in [
            (0, 8, 0),
            (8, 8, 0),
            (32, 8, 0),
            (64, 8, 0),
            (32, 32, 0),
            (32, 64, 0),
            (0, 8, 64),
            (0, 8, 256),
            (0, 8, 1536),
        ] {
            let (config, request) = cooperative_fixture(aliases, name_bytes, consent_bytes);
            let app = usersync_app::TestApp::new(&config);
            let raw = serde_json::to_vec(&request).unwrap();
            let names = u32::try_from(aliases + 2).unwrap();
            let mut limits = narrow_limits();
            limits.body_bytes = bytes(raw.len());
            limits.body_node_bytes = nodes(&request) * NODE_RETAINED_BYTES;
            limits.catalog_names = u32::try_from(aliases + 4).unwrap();
            limits.priority_groups = 1;
            limits.priority_entries = names;
            limits.requested_names = names;
            limits.filter_names_per_list = names;
            limits.debug_rows = names;
            // Independent endpoint/callback punctuation allowance plus two
            // occurrences of the ASCII consent. This is not a measured URL.
            let url_bound = 512 + 2 * consent_bytes;
            let output_bound =
                (512 + 2 * url_bound + aliases * (name_bytes + 64)).next_power_of_two();
            limits.url_bytes = bytes(url_bound);
            limits.output_body_bytes = bytes(output_bound);
            let charges = derive_aux_work(limits).unwrap().charges();
            let relevant = charges.body
                + charges.nodes
                + charges.chooser
                + charges.urls
                + charges.retained_urls
                + charges.output;
            let (response, peak) = measure(
                &app,
                Method::POST,
                "/cookie_sync",
                &[("content-type", "application/json")],
                raw,
            );
            assert_eq!(response.status(), StatusCode::OK);
            let body: Value = serde_json::from_slice(usersync_app::body(&response)).unwrap();
            let selected = body["bidder_status"].as_array().unwrap();
            assert_eq!(selected.len(), 2);
            if aliases > 0 {
                assert_eq!(body["debug"].as_array().unwrap().len(), aliases);
            }
            let url_bytes: usize = selected
                .iter()
                .map(|item| {
                    let url = item["usersync"]["url"].as_str().unwrap();
                    assert!(url.len() <= url_bound);
                    url.len()
                })
                .sum();
            assert!(url_bytes <= output_bound);
            assert!(usersync_app::body(&response).len() <= output_bound);
            if aliases == 0 {
                delta_guard("URL input growth", &mut url_reference, peak, relevant);
            }
            if name_bytes == 8 && consent_bytes == 0 {
                delta_guard("chooser count growth", &mut alias_reference, peak, relevant);
            }
            if aliases == 32 {
                delta_guard("chooser name growth", &mut name_reference, peak, relevant);
            }
            guard(
                &format!("coop aliases={aliases} name={name_bytes} consent={consent_bytes}"),
                peak,
                relevant,
                usersync_keys::SECRET_JSON.len(),
            );
        }
    }

    fn keyring_source(key_count: usize, source_bytes: usize) -> Vec<u8> {
        let keys: Vec<_> = (0..key_count).map(|index| json!({
            "id": format!("key_{index}"),
            "key": URL_SAFE_NO_PAD.encode([u8::try_from(index.checked_add(1).unwrap()).unwrap(); 32]),
        })).collect();
        let mut source = serde_json::to_vec(&json!({"active": "key_0", "keys": keys})).unwrap();
        assert!(source.len() <= source_bytes);
        source.resize(source_bytes, b' ');
        source
    }

    #[test]
    fn generated_app_keyring_source_and_retained_keys_scale_independently() {
        let app = usersync_app::TestApp::new("");
        let query = format!(
            "bidder={}&uid=probe&f=b&gdpr=0",
            live_catalog::redirect().name()
        );
        let uri = format!("/setuid?{query}");
        let charges = cookie_charges(0, "probe".len(), 0, &query, true);
        let mut one_key_source_reference = None;
        let mut eight_key_source_reference = None;
        let mut key_count_reference = None;
        for (keys, source_bytes) in [(1, 128), (1, 1024), (1, 4096), (8, 1024), (8, 4096)] {
            // The synthetic source is prebuilt and remains in TestSecrets.
            // This qualifies its bounded read/parse, not provider-wide secret
            // value materialization or a provider's native allocation behavior.
            let source = keyring_source(keys, source_bytes);
            app.secrets().set(Some(&source));
            let (response, count) = measure(&app, Method::GET, &uri, &[], Vec::new());
            assert_eq!(response.status(), StatusCode::OK);
            let value = response.headers()["set-cookie"].to_str().unwrap();
            assert!(value.starts_with("uids=v1.key_0."));
            assert!(value.len() <= 4096);
            let decoded = read_written(&app, &response);
            assert_eq!(
                decoded["buyeruids"],
                json!({(live_catalog::redirect().family()): "probe"})
            );
            let terms = sum_terms([
                cookie_relevant(charges),
                keyring_input_terms(source_bytes, keys),
            ]);
            let source_reference = if keys == 1 {
                &mut one_key_source_reference
            } else {
                &mut eight_key_source_reference
            };
            delta_guard("keyring source growth", source_reference, count, terms);
            if source_bytes == 1024 {
                delta_guard(
                    "retained key count growth",
                    &mut key_count_reference,
                    count,
                    terms,
                );
            }
            guard(
                &format!("keyring keys={keys} source={source_bytes}"),
                count,
                cookie_relevant(charges),
                source_bytes,
            );
        }
    }
}
