//! Privacy outbound acceptance through the HTTP handler, runtime config loader,
//! and default-active adapters. Only fanout transport is replaced; no network is used.

mod support;

#[cfg(test)]
mod config_funding_support;
use config_funding_support as config_funding;

#[cfg(test)]
use auctioneer_bidder::test_support::serving_catalog;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use crate::config_funding;
    use crate::serving_catalog::{self, ServingBidder};
    use crate::support::ctx_with_config;
    use async_trait::async_trait;
    use auctioneer_bidder::{BidderHttpRequest, BidderHttpResponse};
    use auctioneer_config_data::types::MediaType;
    use auctioneer_core::handlers::{TestFanoutHandle, auction};
    use auctioneer_core::host_primitives::{
        TestClockHandle, TestMonotonicClockHandle, TestRngHandle,
    };
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_runtime::{Clock, FakeClock, Rng, SeededRng};
    use edgezero_core::body::Body;
    use edgezero_core::http::StatusCode;
    use edgezero_core::time::MonotonicClock;
    use flate2::read::GzDecoder;
    use futures::executor::block_on;
    use http::{Method, Request, Uri};
    use serde_json::{Map, Value, json};
    use std::borrow::Cow;
    use std::iter::once;
    use std::mem;
    use std::slice;
    use std::str;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use time::OffsetDateTime;

    const CLIENT_V4: &str = "8.8.4.123";
    const CLIENT_V6: &str = "2001:4860:4860::8844";
    const PRIVATE_ID_MARKERS: [&str; 13] = [
        "private-user-id",
        "private-buyer-id",
        "native-eid",
        "legacy-eid",
        "private-ifa",
        "private-dpidsha1",
        "private-dpidmd5",
        "private-didsha1",
        "private-didmd5",
        "private-macsha1",
        "private-macmd5",
        "private-uid2",
        "private-email",
    ];
    const PRIVATE_PATHS: &[&str] = &[
        "/user/id",
        "/user/buyeruid",
        "/user/yob",
        "/user/gender",
        "/user/eids",
        "/user/ext/eids",
        "/user/keywords",
        "/user/kwarray",
        "/user/data",
        "/user/ext/data",
        "/device/ifa",
        "/device/dpidsha1",
        "/device/dpidmd5",
        "/device/didsha1",
        "/device/didmd5",
        "/device/macsha1",
        "/device/macmd5",
        "/device/uid2",
        "/device/metadata",
        "/device/ext/context",
        "/source/tid",
        "/imp/0/ext/tid",
    ];

    #[derive(Default)]
    struct CaptureFanout(Mutex<Vec<BidderHttpRequest>>);

    #[async_trait(?Send)]
    impl BidderFanout for CaptureFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            let mut captured = self.0.lock().expect("capture lock");
            Ok(calls
                .into_iter()
                .map(|call| {
                    captured.push(call.request);
                    BidderCallResult {
                        result: Ok(BidderHttpResponse::new(204, vec![], vec![])),
                        elapsed: Duration::from_millis(1),
                    }
                })
                .collect())
        }
    }

    fn runtime_config(allow: bool) -> String {
        format!(
            "
[gdpr]
enabled = false
[ccpa]
enforce = true
[lmt]
enforce = true
[accounts.fake_tenant.privacy.ipv4]
anon_keep_bits = 24
[accounts.fake_tenant.privacy.ipv6]
anon_keep_bits = 64
[accounts.fake_tenant.privacy.allowactivities.transmitUfpd]
default = {allow}
[accounts.fake_tenant.privacy.allowactivities.transmitPreciseGeo]
default = {allow}
[accounts.fake_tenant.privacy.allowactivities.transmitTid]
default = {allow}
"
        )
    }

    fn full_geo() -> Value {
        json!({
            "lat": 48.858_844_f64, "lon": -1.125_f64, "country": "USA",
            "region": "CA", "regionfips104": "US06", "metro": "807",
            "city": "San Francisco", "zip": "94107", "type": 1_i32,
            "accuracy": 7_i32, "lastfix": 10_i32, "ipservice": 3_i32,
            "utcoffset": -420_i32, "ext": {"location": "private-location"},
            "extra_geo": "private-extra-location"
        })
    }

    fn baseline_bidders(inventory: &str) -> Vec<ServingBidder> {
        let rows: Vec<_> = serving_catalog::default_active_publications()
            .into_iter()
            .filter(|row| {
                let media = match inventory {
                    "site" => &row.info.capabilities.site,
                    "app" => &row.info.capabilities.app,
                    _ => panic!("unknown fixture inventory"),
                };
                !row.info.disabled && media.contains(&MediaType::Banner)
            })
            .collect();
        assert!(
            !rows.is_empty(),
            "default-active banner cohort must not be empty"
        );
        assert!(
            rows.iter().any(|row| row.info.openrtb.gpp_supported),
            "exercise GPP-capable default-active adapters"
        );
        assert!(
            rows.iter().any(|row| !row.info.openrtb.gpp_supported),
            "exercise legacy-backfill default-active adapters"
        );
        assert!(
            rows.iter().any(|row| row
                .owner_expected_request_body
                .as_ref()
                .is_some_and(|expected| expected.get("ext") != row.owner_request.get("ext"))),
            "retain owner protocol-extension transformation evidence"
        );
        for row in &rows {
            row.privacy_wire_contract().unwrap();
        }
        rows
    }

    fn fixture(coppa: bool, usp: &str, lmt: i32, bidders: &[ServingBidder]) -> BidRequest {
        let params: Map<String, Value> = bidders
            .iter()
            .map(|row| (row.name().to_owned(), row.params.clone()))
            .collect();
        serde_json::from_value(json!({
            "id": "fake_auction_record", "cur": ["USD"], "tmax": 1000_i64,
            "site": {"id": "contextual-site", "publisher": {"id": "fake_tenant"},
                "ext": {"amp": 0_i32, "data": {"section": "news"}}},
            "imp": [{"id": "imp-1", "banner": {"w": 300_i32, "h": 250_i32},
                "ext": {"tid": "private-imp-tid", "data": {"slot": "top"},
                    "bidder": params}}],
            "source": {"tid": "private-source-tid", "ext": {"schain": {
                "ver": "1.0", "complete": 1_i32, "nodes": [{
                    "asi": "seller.example", "sid": "seller", "hp": 1_i32
                }]}}},
            "user": {"id": "private-user-id", "buyeruid": "private-buyer-id",
                "yob": 1980_i32, "gender": "F", "keywords": "private-keywords",
                "kwarray": ["private-keyword-array"], "data": [{"id": "private-segment"}],
                "eids": [{"source": "native.example", "uids": [{"id": "native-eid"}]}],
                "ext": {"data": {"audience": "private-audience"},
                    "eids": [{"source": "legacy.example", "uids": [{"id": "legacy-eid"}]}],
                    "context": "keep-user-context"}, "geo": full_geo()},
            "device": {"ip": CLIENT_V4, "ipv6": CLIENT_V6, "lmt": lmt,
                "make": "Example", "model": "Handset", "devicetype": 4_i32,
                "ifa": "private-ifa", "dpidsha1": "private-dpidsha1",
                "dpidmd5": "private-dpidmd5", "didsha1": "private-didsha1",
                "didmd5": "private-didmd5", "macsha1": "private-macsha1",
                "macmd5": "private-macmd5", "geo": full_geo(),
                "uid2": "private-uid2", "metadata": {"sha256email": "private-email"},
                "ext": {"context": "keep-device-context"}},
            "regs": {"gpp": format!("DBACNY~opaque-tcf~{usp}"),
                "gpp_sid": [6_i32, 2_i32, 6_i32], "coppa": i32::from(coppa),
                "us_privacy": "invalid-private-usp"}
        }))
        .expect("typed OpenRTB fixture")
    }

    fn drive(
        fixture: &BidRequest,
        config: &str,
        bidders: &[ServingBidder],
    ) -> Vec<BidderHttpRequest> {
        config_funding::install();
        let fake = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let clock: Arc<dyn Clock + Send + Sync> = Arc::<FakeClock>::clone(&fake);
        let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));
        let fanout = Arc::new(CaptureFanout::default());
        let serialized = serde_json::to_vec(fixture).expect("typed fixture serializes");
        let mut request = Request::builder()
            .method(Method::POST)
            .uri(Uri::from_static("/openrtb2/auction"))
            .header("content-type", "application/json")
            .header("x-forwarded-for", format!("{CLIENT_V4}, {CLIENT_V6}"))
            .header("x-real-ip", CLIENT_V4)
            .body(Body::from_bytes(serialized))
            .expect("HTTP request builds");
        request
            .extensions_mut()
            .insert(TestFanoutHandle(Arc::<CaptureFanout>::clone(&fanout)));
        request.extensions_mut().insert(TestClockHandle(clock));
        request
            .extensions_mut()
            .insert(TestMonotonicClockHandle(MonotonicClock::new(move || {
                fake.now_monotonic()
            })));
        request.extensions_mut().insert(TestRngHandle(rng));

        block_on(async {
            let response = auction(ctx_with_config(config, request))
                .await
                .expect("auction handler answers");
            let status = response.status();
            let bytes = response
                .into_body()
                .into_bytes_bounded(1 << 20)
                .await
                .expect("response body reads");
            assert_eq!(
                status,
                StatusCode::OK,
                "{}",
                String::from_utf8_lossy(&bytes)
            );
        });
        let captured = mem::take(&mut *fanout.0.lock().expect("capture lock"));
        assert_eq!(
            captured.len(),
            bidders.len(),
            "every capable default-active adapter must send"
        );
        captured
    }

    fn body_for(outbound: &[BidderHttpRequest], row: &ServingBidder, original: &Value) -> Value {
        let bidder = row.name();
        let metadata = row.info;
        let requests: Vec<_> = outbound
            .iter()
            .filter(|request| request.url() == metadata.endpoint)
            .collect();
        assert_eq!(requests.len(), 1, "exactly one send for {bidder}");
        assert_eq!(requests[0].method(), "POST");
        assert_eq!(requests[0].covered_imp_ids().collect::<Vec<_>>(), ["imp-1"]);
        let body: Value = serde_json::from_slice(&decoded_body(requests[0])).expect("adapter JSON");
        let contract = row.privacy_wire_contract().unwrap();
        // Canonical ingress supplies the app channel before any bidder runs.
        // Keep this shared default separate from bidder-owned protocol facts.
        let mut prepared_extension = original.get("ext").cloned();
        if original.get("app").is_some() {
            let extension = prepared_extension.get_or_insert_with(|| json!({}));
            if extension.pointer("/prebid/channel").is_none() {
                if extension.get("prebid").is_none() {
                    extension["prebid"] = json!({});
                }
                extension["prebid"]["channel"] = json!({"name": "app"});
            }
        }
        let expected = contract
            .request_extension(prepared_extension.as_ref(), bidder)
            .unwrap();
        assert_eq!(
            body.get("ext"),
            expected.as_ref(),
            "{bidder}: exact protocol extension"
        );
        if !contract.preserves_impression_extension() {
            let impression_extension = contract.preserved_value(original, "/imp/0/ext").unwrap();
            assert_eq!(
                body.pointer("/imp/0/ext"),
                impression_extension.as_ref(),
                "{bidder}: exact impression extension"
            );
        }
        body
    }

    fn assert_protocol_preservation(
        body: &Value,
        original: &Value,
        paths: &[&str],
        row: &ServingBidder,
    ) {
        let contract = row.privacy_wire_contract().unwrap();
        for path in paths {
            assert!(original.pointer(path).is_some(), "fixture lacks {path}");
            let expected = contract.preserved_value(original, path).unwrap();
            assert_eq!(
                body.pointer(path),
                expected.as_ref(),
                "{}: reviewed protocol preservation at {path}",
                row.name()
            );
        }
    }

    fn assert_context_and_signals(
        body: &Value,
        original: &Value,
        row: &ServingBidder,
        usp: Option<&str>,
        ids_scrubbed: bool,
    ) {
        let bidder = row.name();
        assert_protocol_preservation(
            body,
            original,
            &[
                "/id",
                "/site",
                "/imp/0/id",
                "/imp/0/banner",
                "/imp/0/ext/data",
                "/source/ext",
                "/device/lmt",
                "/device/make",
                "/device/model",
                "/device/devicetype",
                "/regs/coppa",
                "/regs/gpp",
                "/regs/gpp_sid",
            ],
            row,
        );
        if ids_scrubbed {
            assert!(body.pointer("/user/ext/context").is_none());
        } else {
            assert_protocol_preservation(body, original, &["/user/ext/context"], row);
        }
        assert_eq!(body["imp"].as_array().unwrap().len(), 1);
        if row
            .privacy_wire_contract()
            .unwrap()
            .preserves_impression_extension()
        {
            assert!(original["imp"][0]["ext"]["bidder"].get(bidder).is_some());
            assert_eq!(
                body["imp"][0]["ext"]["bidder"],
                original["imp"][0]["ext"]["bidder"][bidder]
            );
        } else {
            assert!(
                body.pointer("/imp/0/ext/bidder").is_none(),
                "{bidder}: no protocol bidder parameters on wire"
            );
        }
        if row.info.openrtb.gpp_supported {
            for path in ["/regs/gdpr", "/regs/us_privacy", "/user/consent"] {
                assert!(
                    body.pointer(path).is_none(),
                    "{bidder}: no backfill at {path}"
                );
            }
        } else {
            assert_eq!(body["regs"]["gdpr"], 1_i32);
            if let Some(validated) = usp {
                assert_eq!(body["regs"]["us_privacy"], validated);
            } else {
                assert!(body.pointer("/regs/us_privacy").is_none());
            }
            assert_eq!(body["user"]["consent"], "opaque-tcf");
        }
    }

    fn assert_no_raw_ips(outbound: &[BidderHttpRequest]) {
        for request in outbound {
            let bytes = decoded_body(request);
            let body = str::from_utf8(&bytes).expect("adapter UTF-8");
            for raw in [CLIENT_V4, CLIENT_V6] {
                assert!(!body.contains(raw), "raw IP in body for {}", request.url());
                assert!(!request.url().contains(raw), "raw IP in outbound URL");
                for (name, value) in request.headers() {
                    assert!(!name.contains(raw), "raw IP in outbound header name");
                    assert!(!value.contains(raw), "raw IP in outbound header {name}");
                }
            }
        }
    }

    fn assert_no_private_identifier_text(text: &str) {
        for marker in PRIVATE_ID_MARKERS {
            assert!(
                !text.contains(marker),
                "denied identifier escaped its original field: {marker}"
            );
        }
    }

    fn assert_no_private_identifiers(outbound: &[BidderHttpRequest], original: &Value) {
        assert!(
            !outbound.is_empty(),
            "identifier checks require a real outbound call"
        );
        let before = serde_json::to_string(original).unwrap();
        for marker in PRIVATE_ID_MARKERS {
            assert!(
                before.contains(marker),
                "fixture lacks identifier marker {marker}"
            );
        }
        for request in outbound {
            assert_no_private_identifier_text(str::from_utf8(&decoded_body(request)).unwrap());
            assert_no_private_identifier_text(request.url());
            for (name, value) in request.headers() {
                assert_no_private_identifier_text(name);
                assert_no_private_identifier_text(value);
            }
        }
    }

    fn decoded_body(request: &BidderHttpRequest) -> Cow<'_, [u8]> {
        if request
            .headers()
            .any(|(name, value)| name.eq_ignore_ascii_case("content-encoding") && value == "gzip")
        {
            use std::io::Read as _;
            let mut bytes = Vec::new();
            GzDecoder::new(request.body())
                .read_to_end(&mut bytes)
                .expect("finalized gzip");
            Cow::Owned(bytes)
        } else {
            Cow::Borrowed(request.body())
        }
    }

    #[test]
    fn activity_denials_compose_with_ccpa_lmt_and_coppa_for_default_active_adapters() {
        let bidders = baseline_bidders("site");
        // The first row isolates activities; COPPA must dominate coarsening in the last.
        for (coppa, usp, lmt) in [
            (false, "1YNN", 0_i32),
            (false, "1YYN", 1_i32),
            (true, "1YYN", 1_i32),
        ] {
            let fixture = fixture(coppa, usp, lmt, &bidders);
            let original = serde_json::to_value(&fixture).unwrap();
            let outbound = drive(&fixture, &runtime_config(false), &bidders);
            for row in &bidders {
                let bidder = row.name();
                let body = body_for(&outbound, row, &original);
                for path in PRIVATE_PATHS {
                    assert!(original.pointer(path).is_some(), "fixture lacks {path}");
                    assert!(body.pointer(path).is_none(), "{bidder}: scrub {path}");
                }
                assert_eq!(body["device"]["ip"], "8.8.4.0");
                assert_eq!(body["device"]["ipv6"], "2001:4860:4860::");
                for owner in ["user", "device"] {
                    let expected = if coppa {
                        json!({})
                    } else {
                        json!({
                            "lat": 48.86_f64,
                            "lon": -1.12_f64,
                            "country": "USA",
                            "region": "CA",
                            "regionfips104": "US06",
                            "city": "San Francisco",
                            "metro": "807",
                            "utcoffset": -420_i32,
                            "type": 1_i32,
                            "ipservice": 3_i32,
                            "lastfix": 10_i32
                        })
                    };
                    assert_eq!(body[owner]["geo"], expected, "{bidder}: {owner} geo");
                }
                assert_context_and_signals(&body, &original, row, Some(usp), true);
            }
            assert_no_raw_ips(&outbound);
            assert_no_private_identifiers(&outbound, &original);
        }
    }

    #[test]
    fn coppa_alone_removes_allowed_user_fpd_despite_no_sale_for_default_active_adapters() {
        for inventory in ["site", "app"] {
            let bidders = baseline_bidders(inventory);
            for coppa in [false, true] {
                let mut fixture = fixture(coppa, "1YYN", 0, &bidders);
                fixture.ext = Some(
                    serde_json::from_value(json!({"prebid": {"nosale": ["*"]}}))
                        .expect("no-sale control"),
                );
                if inventory == "app" {
                    fixture.site = None;
                    fixture.app = Some(
                        serde_json::from_value(json!({
                            "id": "contextual-app", "name": "Example App", "bundle": "example.app",
                            "publisher": {"id": "fake_tenant"},
                            "ext": {"data": {"section": "news"}}
                        }))
                        .expect("app context"),
                    );
                }
                let original = serde_json::to_value(&fixture).unwrap();
                // GDPR is disabled, LMT is zero, all activities allow, and no-sale
                // exempts the valid sale opt-out. Only COPPA can remove user FPD.
                let outbound = drive(&fixture, &runtime_config(true), &bidders);
                for row in &bidders {
                    let bidder = row.name();
                    let body = body_for(&outbound, row, &original);
                    for path in PRIVATE_PATHS {
                        if matches!(*path, "/source/tid" | "/imp/0/ext/tid") {
                            continue;
                        }
                        assert!(original.pointer(path).is_some(), "fixture lacks {path}");
                        if coppa {
                            assert!(body.pointer(path).is_none(), "{bidder}: scrub {path}");
                        } else {
                            assert_protocol_preservation(&body, &original, &[*path], row);
                        }
                    }
                    assert_protocol_preservation(
                        &body,
                        &original,
                        &[
                            if inventory == "app" { "/app" } else { "/site" },
                            "/id",
                            "/imp/0/id",
                            "/imp/0/banner",
                            "/imp/0/ext/data",
                            "/source/tid",
                            "/imp/0/ext/tid",
                            "/source/ext",
                            "/device/lmt",
                            "/regs/coppa",
                            "/regs/gpp",
                            "/regs/gpp_sid",
                        ],
                        row,
                    );
                    if coppa {
                        assert_eq!(body["user"]["geo"], json!({}));
                        assert_eq!(body["device"]["geo"], json!({}));
                        assert_eq!(body["device"]["ip"], "8.8.4.0");
                        assert_eq!(body["device"]["ipv6"], "2001:4860:4860::");
                    } else {
                        assert_protocol_preservation(
                            &body,
                            &original,
                            &["/user/geo", "/device/geo", "/device/ip", "/device/ipv6"],
                            row,
                        );
                    }
                }
                assert_eq!(serde_json::to_value(&fixture).unwrap(), original);
                if coppa {
                    assert_no_raw_ips(&outbound);
                    assert_no_private_identifiers(&outbound, &original);
                }
            }
        }
    }

    #[test]
    fn malformed_selected_usp_scrubs_only_enabled_nonexempt_default_active_adapters() {
        let bidders = baseline_bidders("site");
        let fpd_and_tid_paths = [
            "/user/data",
            "/user/keywords",
            "/user/kwarray",
            "/user/ext/data",
            "/source/tid",
            "/imp/0/ext/tid",
        ];
        for source in ["legacy", "gpp"] {
            for enabled in [false, true] {
                for exempt in once(None).chain(bidders.iter().map(|row| Some(row.name()))) {
                    let mut fixture = fixture(false, "malformed-active-private-usp", 0, &bidders);
                    if source == "legacy" {
                        fixture.regs.as_mut().unwrap().gpp_sid = Some(vec![2_i32]);
                    }
                    let names: Vec<_> = exempt.into_iter().collect();
                    fixture.ext = Some(
                        serde_json::from_value(json!({"prebid": {"nosale": names}}))
                            .expect("exact no-sale control"),
                    );
                    let original = serde_json::to_value(&fixture).unwrap();
                    let enforcement = if enabled { "enabled" } else { "disabled" };
                    let config = format!(
                        "{}\n[accounts.fake_tenant.ccpa]\nenabled = \"{enforcement}\"\n",
                        runtime_config(true)
                    );
                    let outbound = drive(&fixture, &config, &bidders);
                    for row in &bidders {
                        let bidder = row.name();
                        let body = body_for(&outbound, row, &original);
                        let scrubbed = enabled && exempt != Some(bidder);
                        for path in PRIVATE_PATHS {
                            if fpd_and_tid_paths.contains(path) {
                                continue;
                            }
                            assert!(original.pointer(path).is_some(), "fixture lacks {path}");
                            if scrubbed {
                                assert!(body.pointer(path).is_none(), "{bidder}: scrub {path}");
                            } else {
                                assert_protocol_preservation(&body, &original, &[*path], row);
                            }
                        }
                        assert_protocol_preservation(&body, &original, &fpd_and_tid_paths, row);
                        if scrubbed {
                            for owner in ["user", "device"] {
                                assert_eq!(
                                    body[owner]["geo"],
                                    json!({
                                        "lat": 48.86_f64, "lon": -1.12_f64,
                                        "country": "USA", "region": "CA", "regionfips104": "US06",
                                        "city": "San Francisco", "metro": "807", "utcoffset": -420_i32,
                                        "type": 1_i32, "ipservice": 3_i32, "lastfix": 10_i32
                                    }),
                                    "{bidder}: {owner} geo"
                                );
                            }
                            assert_eq!(body["device"]["ip"], "8.8.4.0");
                            assert_eq!(body["device"]["ipv6"], "2001:4860:4860::");
                            let endpoint = &row.info.endpoint;
                            let request = outbound
                                .iter()
                                .find(|request| request.url() == endpoint)
                                .expect("captured bidder");
                            assert_no_raw_ips(slice::from_ref(request));
                            assert_no_private_identifiers(slice::from_ref(request), &original);
                        } else {
                            assert_protocol_preservation(
                                &body,
                                &original,
                                &["/user/geo", "/device/geo", "/device/ip", "/device/ipv6"],
                                row,
                            );
                        }
                        assert_context_and_signals(&body, &original, row, None, scrubbed);
                    }
                    assert_eq!(serde_json::to_value(&fixture).unwrap(), original);
                }
            }
        }
    }

    #[test]
    fn allowing_control_preserves_private_values_except_declared_protocol_changes() {
        let bidders = baseline_bidders("site");
        let fixture = fixture(false, "1YNN", 0, &bidders);
        let original = serde_json::to_value(&fixture).unwrap();
        let outbound = drive(&fixture, &runtime_config(true), &bidders);
        for row in &bidders {
            let body = body_for(&outbound, row, &original);
            assert_protocol_preservation(&body, &original, PRIVATE_PATHS, row);
            assert_protocol_preservation(
                &body,
                &original,
                &["/user/geo", "/device/geo", "/device/ip", "/device/ipv6"],
                row,
            );
            assert_context_and_signals(&body, &original, row, Some("1YNN"), false);
        }
    }

    #[test]
    fn identifier_guard_rejects_each_value_hidden_under_protocol_fields() {
        use std::panic::catch_unwind;

        assert_eq!(PRIVATE_ID_MARKERS.len(), 13);
        for marker in PRIVATE_ID_MARKERS {
            let body = json!({"wrapper": {"renamed": marker}}).to_string();
            drop(catch_unwind(|| assert_no_private_identifier_text(&body)).unwrap_err());
        }
    }

    #[test]
    fn absent_empty_and_nonempty_extensions_reach_every_default_active_protocol() {
        let bidders = baseline_bidders("site");
        for extension in [
            None,
            Some(json!({})),
            Some(json!({"prebid": {"nosale": ["*"]}})),
        ] {
            let mut input = fixture(false, "1YNN", 0, &bidders);
            input.ext = extension.map(|value| serde_json::from_value(value).unwrap());
            let original = serde_json::to_value(&input).unwrap();
            let outbound = drive(&input, &runtime_config(true), &bidders);
            for row in &bidders {
                let body = body_for(&outbound, row, &original);
                assert_protocol_preservation(&body, &original, PRIVATE_PATHS, row);
                assert_context_and_signals(&body, &original, row, Some("1YNN"), false);
            }
            assert_eq!(serde_json::to_value(&input).unwrap(), original);
        }
    }
}
