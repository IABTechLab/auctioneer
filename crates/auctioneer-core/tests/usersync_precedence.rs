#[cfg(test)]
use auctioneer_core::test_support::live_catalog;

mod usersync_app;

#[cfg(test)]
use auctioneer_core::test_support::usersync_keys;

use auctioneer_privacy::test_support::tc_encode;

#[cfg(test)]
mod bounded_store;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use super::{
        live_catalog, tc_encode,
        usersync_app::{TEST_NOW, TestApp, body},
        usersync_keys::{decode_cookie, encode_cookie},
    };
    use auctioneer_core::usersync::{
        cookie::{self, UidCookie},
        limits::CookieLimits,
    };
    use edgezero_core::{
        body::Body,
        http::{Method, Response, StatusCode},
    };
    use serde_json::{Value, json};
    use tc_encode::VendorSectionSpec;

    const ORIGIN: &str = "https://publisher.example";
    const POLICY: &str = "[user_sync]\nallowed_origins = ['https://publisher.example']\n\
                     [host_cookie.optout_cookie]\nname = 'optout'\nvalue = 'yes'\n\
                     [accounts.disabled]\ndisabled = true\n\
                     [accounts.denied.privacy.allowactivities.syncUser]\ndefault = false\n";
    type HeaderFields<'a> = &'a [(&'a str, &'a str)];
    type CookieSyncCase<'a> = (&'a str, HeaderFields<'a>, Vec<u8>, StatusCode, &'a [u8]);

    fn consent(purpose_one: bool) -> String {
        let mut spec = tc_encode::valid_core_spec();
        spec.vendor_list_version = 178;
        spec.tcf_policy_version = 5;
        spec.purposes_consent = if purpose_one { vec![1] } else { Vec::new() };
        let bidder_vendor = u16::try_from(live_catalog::redirect().info.gvl_vendor_id.unwrap())
            .expect("role declares a TCF vendor");
        let mut vendors = vec![32, bidder_vendor];
        vendors.sort_unstable();
        vendors.dedup();
        spec.vendor_consent = VendorSectionSpec::Bitfield {
            max_vendor_id: bidder_vendor.max(32),
            set: vendors,
        };
        tc_encode::with_minimal_dv(&tc_encode::encode_core(&spec))
    }

    const GDPR_POLICY: &str = "[gdpr]\nenabled = true\nhost_vendor_id = 32\n\
                              [gdpr.tcf2]\nenabled = true\n\
                              [gdpr.tcf2.purposes.1]\nenforce_purpose = 'enabled'\nenforce_vendors = 'enabled'\n";

    fn assert_wire(
        response: &Response,
        status: StatusCode,
        expected_body: &[u8],
        content_type: Option<&str>,
        cors: bool,
    ) {
        assert_eq!(
            response.status(),
            status,
            "{}",
            String::from_utf8_lossy(body(response))
        );
        assert_eq!(body(response), expected_body);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .map(|value| value.to_str().unwrap()),
            content_type
        );
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        assert!(response.headers().get("set-cookie").is_none());
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-origin")
                .map(|value| value.to_str().unwrap()),
            cors.then_some(ORIGIN),
        );
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-credentials")
                .map(|value| value.to_str().unwrap()),
            cors.then_some("true")
        );
        if cors {
            assert!(
                response
                    .headers()
                    .get("vary")
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .split(',')
                    .any(|part| part.trim().eq_ignore_ascii_case("origin"))
            );
        }
    }

    #[test]
    fn cookie_sync_earlier_fault_wins_and_errors_keep_credentialed_cors() {
        let app = TestApp::new(POLICY);
        // The first three cases require literal incomplete JSON: serialization
        // would remove the syntax fault whose precedence they assert.
        let cases: &[CookieSyncCase<'_>] = &[
        (
            "encoding before length and JSON",
            &[("content-encoding", "gzip"), ("content-length", "32769")],
            b"{".to_vec(),
            StatusCode::BAD_REQUEST,
            b"Invalid request: Content-Encoding of type gzip is not supported\n",
        ),
        (
            "length before JSON",
            &[("content-length", "32769")],
            b"{".to_vec(),
            StatusCode::BAD_REQUEST,
            b"cookie_sync request size exceeded max size of 32768 bytes.\n",
        ),
        (
            "JSON before optout",
            &[("cookie", "optout=yes")],
            b"{".to_vec(),
            StatusCode::BAD_REQUEST,
            b"JSON parsing failed: invalid JSON\n",
        ),
        (
            "optout before account",
            &[("cookie", "optout=yes")],
            serde_json::to_vec(&json!({
                "account": "disabled", "bidders": [live_catalog::redirect().name()], "gdpr": 0_i32
            })).unwrap(),
            StatusCode::UNAUTHORIZED,
            b"User has opted out\n",
        ),
        (
            "account before UID decode",
            &[("cookie", "uids=invalid")],
            serde_json::to_vec(&json!({
                "account": "disabled", "bidders": [live_catalog::redirect().name()], "gdpr": 2_i32
            })).unwrap(),
            StatusCode::BAD_REQUEST,
            b"account is disabled, please reach out to the prebid server host\n",
        ),
        (
            "privacy before filters",
            &[],
            serde_json::to_vec(&json!({
                "gdpr": 2_i32, "filterSettings": {"image": {"bidders": 42_i32}}
            })).unwrap(),
            StatusCode::BAD_REQUEST,
            b"GDPR signal should be integer 0 or 1\n",
        ),
        (
            "filters before limit",
            &[],
            serde_json::to_vec(&json!({
                "gdpr": 0_i32, "filterSettings": {"image": {"filter": "bad"}}, "limit": 1.5_f64
            })).unwrap(),
            StatusCode::BAD_REQUEST,
            b"error parsing filtersettings.image: invalid filter value. must be either 'include' or 'exclude'\n",
        ),
        (
            "limit before chooser",
            &[],
            serde_json::to_vec(&json!({
                "gdpr": 0_i32, "limit": 1.5_f64, "bidders": ["bad name"]
            })).unwrap(),
            StatusCode::BAD_REQUEST,
            b"invalid limit\n",
        ),
    ];
        for (label, extra_headers, payload, status, expected) in cases {
            let mut headers = vec![("origin", ORIGIN), ("content-type", "application/json")];
            headers.extend_from_slice(extra_headers);
            let response = app.dispatch(
                Method::POST,
                "/cookie_sync",
                &headers,
                Body::from_bytes(payload.clone()),
            );
            assert_wire(
                &response,
                *status,
                expected,
                Some("text/plain; charset=utf-8"),
                true,
            );
            assert_eq!(
                response.headers().get("x-content-type-options").unwrap(),
                "nosniff",
                "{label}"
            );
        }
        assert_eq!(app.store().reads(), cases.len());
    }

    #[test]
    fn disabled_account_wins_over_valid_uid_cookie_optout() {
        let app = TestApp::new(POLICY);
        let encoded = encode_cookie(&UidCookie::opted_out(), &CookieLimits::default()).unwrap();
        let cookie = format!("uids={encoded}");
        let valid = decode_cookie(&encoded, &CookieLimits::default()).unwrap();
        assert!(!valid.corrupt);
        assert!(!valid.cookie.allow_syncs());

        let response = app.dispatch(
            Method::POST,
            "/cookie_sync",
            &[
                ("origin", ORIGIN),
                ("content-type", "application/json"),
                ("cookie", &cookie),
            ],
            Body::from_bytes(
                serde_json::to_vec(&json!({
                    "account": "disabled", "bidders": [live_catalog::redirect().name()], "gdpr": 0_i32
                }))
                .unwrap(),
            ),
        );
        assert_wire(
            &response,
            StatusCode::BAD_REQUEST,
            b"account is disabled, please reach out to the prebid server host\n",
            Some("text/plain; charset=utf-8"),
            true,
        );
        assert_eq!(app.store().reads(), 1);
    }

    fn assert_invalid_uid_cookie_all_readers(encoded: &str) {
        let app = TestApp::new(&format!("{POLICY}{GDPR_POLICY}"));
        let header = format!("uids={encoded}");
        for (account, gdpr) in [("", 2_i32), ("denied", 0_i32)] {
            let post = app.dispatch(
                Method::POST,
                "/cookie_sync",
                &[
                    ("origin", ORIGIN),
                    ("content-type", "application/json"),
                    ("cookie", &header),
                ],
                Body::from_bytes(
                    serde_json::to_vec(&json!({
                        "account": account, "bidders": [live_catalog::redirect().name()], "gdpr": gdpr,
                    }))
                    .unwrap(),
                ),
            );
            assert_wire(
                &post,
                StatusCode::BAD_REQUEST,
                b"invalid UID cookie\n",
                Some("text/plain; charset=utf-8"),
                true,
            );
            let callback = app.dispatch(
                Method::GET,
                &live_catalog::redirect()
                    .setuid(&format!("uid=new-user&gdpr={gdpr}&account={account}")),
                &[("origin", ORIGIN), ("cookie", &header)],
                Body::empty(),
            );
            assert_wire(
                &callback,
                StatusCode::BAD_REQUEST,
                b"invalid UID cookie",
                None,
                true,
            );
        }
        let readback = app.dispatch(
            Method::GET,
            "/getuids",
            &[("origin", ORIGIN), ("cookie", &header)],
            Body::empty(),
        );
        assert_wire(
            &readback,
            StatusCode::BAD_REQUEST,
            b"invalid UID cookie",
            None,
            true,
        );
    }

    #[test]
    fn unsigned_uid_cookies_are_rejected_on_every_reader() {
        let mut stored = UidCookie::default();
        stored
            .set_uid(
                live_catalog::redirect().family(),
                "unsigned-user",
                TEST_NOW,
                &CookieLimits::default(),
            )
            .unwrap();
        // Inner reference payloads remain valid codec fixtures, not accepted HTTP state.
        let unsigned = cookie::encode_cookie(&stored, &CookieLimits::default()).unwrap();
        assert!(
            !cookie::decode_cookie(&unsigned, &CookieLimits::default())
                .unwrap()
                .corrupt
        );
        assert_invalid_uid_cookie_all_readers(&unsigned);
    }

    #[test]
    fn corrupted_signature_precedes_privacy_and_activity_instead_of_fresh_cookie_fallback() {
        let mut stored = UidCookie::default();
        stored
            .set_uid(
                live_catalog::redirect().family(),
                "authenticated-user",
                TEST_NOW,
                &CookieLimits::default(),
            )
            .unwrap();
        let signed = encode_cookie(&stored, &CookieLimits::default()).unwrap();
        assert_eq!(
            decode_cookie(&signed, &CookieLimits::default())
                .unwrap()
                .cookie,
            stored
        );
        let (prefix, mac) = signed.rsplit_once('.').unwrap();
        let replacement = if mac.starts_with('A') { 'B' } else { 'A' };
        let suffix = mac.get(1..).expect("signed MAC is nonempty ASCII");
        let tampered = format!("{prefix}.{replacement}{suffix}");
        let _error = decode_cookie(&tampered, &CookieLimits::default())
            .expect_err("tampered MAC must be rejected");
        // The approved cutoff replaces the reference's corrupt-as-fresh behavior.
        assert_invalid_uid_cookie_all_readers(&tampered);
    }

    #[test]
    fn cookie_sync_host_permission_denial_precedes_invalid_filters_and_limit() {
        let config = format!("{POLICY}{GDPR_POLICY}");
        let app = TestApp::new(&config);
        for (field, expected_error) in [
            (
                serde_json::json!({"filterSettings": {"image": {"bidders": 42_i32}}}),
                b"error parsing filtersettings.image: invalid bidders type. must either be a string '*' or a string array\n".as_slice(),
            ),
            (
                serde_json::json!({"filterSettings": {"image": {"filter": "bad"}}}),
                b"error parsing filtersettings.image: invalid filter value. must be either 'include' or 'exclude'\n".as_slice(),
            ),
            (
                serde_json::json!({"limit": 1.5_f64}),
                b"invalid limit\n".as_slice(),
            ),
        ] {
            for purpose_one in [false, true] {
                let mut payload = serde_json::json!({
                    "bidders": [live_catalog::redirect().name()],
                    "gdpr": 1_i32,
                    "gdpr_consent": consent(purpose_one),
                    "coopSync": false,
                    "debug": true,
                });
                payload
                    .as_object_mut()
                    .unwrap()
                    .extend(field.as_object().unwrap().clone());
                let response = app.dispatch(
                    Method::POST,
                    "/cookie_sync",
                    &[("origin", ORIGIN), ("content-type", "application/json")],
                    Body::from_bytes(serde_json::to_vec(&payload).unwrap()),
                );
                let (status, expected, content_type) = if purpose_one {
                    (
                        StatusCode::BAD_REQUEST,
                        expected_error,
                        "text/plain; charset=utf-8",
                    )
                } else {
                    (
                        StatusCode::OK,
                        b"{\"status\":\"no_cookie\",\"bidder_status\":[]}\n".as_slice(),
                        "application/json; charset=utf-8",
                    )
                };
                assert_wire(&response, status, expected, Some(content_type), true);
            }
        }
        assert_eq!(app.store().reads(), 6);
    }

    #[test]
    fn cookie_sync_account_denial_precedes_host_uid_merge_limit() {
        let app = TestApp::new(POLICY);
        let cookie = format!("auctioneer_uid={}", "x".repeat(25_000));
        for (account, expected) in [
            (
                "disabled",
                b"account is disabled, please reach out to the prebid server host\n".as_slice(),
            ),
            ("", b"cookie resource limit exceeded\n".as_slice()),
        ] {
            let payload = serde_json::json!({
                "account": account,
                "bidders": [live_catalog::redirect().name()],
                "gdpr": 0_i32,
            });
            let response = app.dispatch(
                Method::POST,
                "/cookie_sync",
                &[
                    ("origin", ORIGIN),
                    ("content-type", "application/json"),
                    ("cookie", &cookie),
                ],
                Body::from_bytes(serde_json::to_vec(&payload).unwrap()),
            );
            assert_wire(
                &response,
                StatusCode::BAD_REQUEST,
                expected,
                Some("text/plain; charset=utf-8"),
                true,
            );
        }
        assert_eq!(app.store().reads(), 2);
    }

    #[test]
    fn cookie_sync_uid_merge_limit_precedes_invalid_privacy() {
        let app = TestApp::new(POLICY);
        let cookie = format!("auctioneer_uid={}", "x".repeat(25_000));
        for (extra_headers, expected) in [
            (
                vec![("cookie", cookie.as_str())],
                b"cookie resource limit exceeded\n".as_slice(),
            ),
            (vec![], b"GDPR signal should be integer 0 or 1\n".as_slice()),
        ] {
            let mut headers = vec![("origin", ORIGIN), ("content-type", "application/json")];
            headers.extend(extra_headers);
            let response = app.dispatch(
                Method::POST,
                "/cookie_sync",
                &headers,
                Body::from_bytes(
                    serde_json::to_vec(&json!({
                        "bidders": [live_catalog::redirect().name()], "gdpr": 2_i32
                    }))
                    .unwrap(),
                ),
            );
            assert_wire(
                &response,
                StatusCode::BAD_REQUEST,
                expected,
                Some("text/plain; charset=utf-8"),
                true,
            );
        }
        assert_eq!(app.store().reads(), 2);
    }

    #[test]
    fn uid_cookie_optout_precedes_invalid_privacy_on_both_write_routes() {
        let app = TestApp::new(POLICY);
        let encoded = encode_cookie(&UidCookie::opted_out(), &CookieLimits::default()).unwrap();
        let cookie = format!("uids={encoded}");
        let headers = [
            ("origin", ORIGIN),
            ("content-type", "application/json"),
            ("cookie", cookie.as_str()),
        ];
        let sync = app.dispatch(
            Method::POST,
            "/cookie_sync",
            &headers,
            Body::from_bytes(
                serde_json::to_vec(&json!({
                    "bidders": [live_catalog::redirect().name()], "gdpr": 2_i32
                }))
                .unwrap(),
            ),
        );
        assert_wire(
            &sync,
            StatusCode::UNAUTHORIZED,
            b"User has opted out\n",
            Some("text/plain; charset=utf-8"),
            true,
        );
        let callback = app.dispatch(
            Method::GET,
            &live_catalog::redirect().setuid("uid=must-not-write&gdpr=2&f=i"),
            &headers,
            Body::empty(),
        );
        assert_wire(&callback, StatusCode::UNAUTHORIZED, b"", None, true);
        assert_eq!(app.store().reads(), 2);
    }

    #[test]
    fn routed_valid_tcf_consent_allows_and_denies_purpose_one() {
        let app = TestApp::new(GDPR_POLICY);
        for (purpose_one, expected_count, expected_setuid) in [
            (true, 1, StatusCode::OK),
            (false, 0, StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS),
        ] {
            let tc = consent(purpose_one);
            let payload = serde_json::json!({
                "bidders": [live_catalog::redirect().name()],
                "gdpr": 1_i32,
                "gdpr_consent": tc,
                "coopSync": false,
            });
            let response = app.dispatch(
                Method::POST,
                "/cookie_sync",
                &[("content-type", "application/json")],
                Body::from_bytes(serde_json::to_vec(&payload).unwrap()),
            );
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "{}",
                String::from_utf8_lossy(body(&response))
            );
            let json: Value = serde_json::from_slice(body(&response)).unwrap();
            assert_eq!(
                json["bidder_status"].as_array().unwrap().len(),
                expected_count
            );

            let uri = live_catalog::redirect()
                .setuid(&format!("uid=consent-user&gdpr=1&gdpr_consent={tc}&f=i"));
            let callback = app.dispatch(Method::GET, &uri, &[], Body::empty());
            assert_eq!(
                callback.status(),
                expected_setuid,
                "{}",
                String::from_utf8_lossy(body(&callback))
            );
            assert_eq!(callback.headers().get("set-cookie").is_some(), purpose_one);
        }
        assert_eq!(app.store().reads(), 4);
    }

    #[test]
    fn ambiguous_gdpr_scope_obeys_both_host_defaults() {
        for (default_value, expected_count, expected_setuid) in [
            ("0", 1, StatusCode::OK),
            ("1", 0, StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS),
        ] {
            let config = GDPR_POLICY.replacen(
                "[gdpr]\n",
                &format!("[gdpr]\ndefault_value = '{default_value}'\n"),
                1,
            );
            let app = TestApp::new(&config);
            let denied_tc = consent(false);
            let payload = serde_json::json!({
                "bidders": [live_catalog::redirect().name()],
                "gdpr_consent": denied_tc,
                "coopSync": false,
            });
            let response = app.dispatch(
                Method::POST,
                "/cookie_sync",
                &[("content-type", "application/json")],
                Body::from_bytes(serde_json::to_vec(&payload).unwrap()),
            );
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "{}",
                String::from_utf8_lossy(body(&response))
            );
            let json: Value = serde_json::from_slice(body(&response)).unwrap();
            assert_eq!(
                json["bidder_status"].as_array().unwrap().len(),
                expected_count
            );
            let uri = live_catalog::redirect()
                .setuid(&format!("uid=ambiguous-user&gdpr_consent={denied_tc}&f=i"));
            let callback = app.dispatch(Method::GET, &uri, &[], Body::empty());
            assert_eq!(
                callback.status(),
                expected_setuid,
                "{}",
                String::from_utf8_lossy(body(&callback))
            );
            assert_eq!(
                callback.headers().get("set-cookie").is_some(),
                default_value == "0"
            );
            assert_eq!(app.store().reads(), 2);
        }
    }

    #[test]
    fn setuid_adjacent_faults_obey_application_order_without_cookies() {
        let app = TestApp::new(POLICY);
        let oversized_cookie = format!("auctioneer_uid={}", "x".repeat(25_000));
        let format_fault = format!(
            "bidder={}&f=bad&account=disabled",
            live_catalog::redirect().name()
        );
        let account_fault = format!(
            "bidder={}&f=b&account=disabled",
            live_catalog::unsyncable().name
        );
        let syncer_fault = format!("bidder={}&f=b&gdpr=2", live_catalog::unsyncable().name);
        let cookie_fault = format!("bidder={}&f=b&gdpr=2", live_catalog::iframe().name());
        let privacy_fault = format!(
            "bidder={}&f=b&gdpr=2&account=denied",
            live_catalog::iframe().name()
        );
        let cases = [
            (
                "query before optout",
                "uid=%&f=bad",
                "optout=yes",
                StatusCode::BAD_REQUEST,
                b"invalid query encoding".as_slice(),
            ),
            (
                "optout before bidder",
                "f=bad",
                "optout=yes",
                StatusCode::UNAUTHORIZED,
                b"".as_slice(),
            ),
            (
                "bidder before format",
                "f=bad",
                "",
                StatusCode::BAD_REQUEST,
                b"\"bidder\" query param is required".as_slice(),
            ),
            (
                "format before account",
                format_fault.as_str(),
                "",
                StatusCode::BAD_REQUEST,
                b"\"f\" query param is invalid. must be \"b\" or \"i\"".as_slice(),
            ),
            (
                "account before syncer",
                account_fault.as_str(),
                "",
                StatusCode::BAD_REQUEST,
                b"account is disabled, please reach out to the prebid server host".as_slice(),
            ),
            (
                "syncer before UID decode",
                syncer_fault.as_str(),
                &oversized_cookie,
                StatusCode::BAD_REQUEST,
                b"The bidder name provided is not supported by Prebid Server".as_slice(),
            ),
            (
                "UID decode before privacy",
                cookie_fault.as_str(),
                &oversized_cookie,
                StatusCode::BAD_REQUEST,
                b"cookie resource limit exceeded".as_slice(),
            ),
            (
                "privacy before activity",
                privacy_fault.as_str(),
                "",
                StatusCode::BAD_REQUEST,
                b"the gdpr query param must be either 0 or 1".as_slice(),
            ),
        ];
        for (label, query, cookie, status, expected) in cases {
            let mut headers = vec![("origin", ORIGIN)];
            if !cookie.is_empty() {
                headers.push(("cookie", cookie));
            }
            let response = app.dispatch(
                Method::GET,
                &format!("/setuid?{query}"),
                &headers,
                Body::empty(),
            );
            assert_wire(&response, status, expected, None, true);
            assert_eq!(response.headers().get("content-type"), None, "{label}");
        }
        assert_eq!(app.store().reads(), cases.len());
    }

    #[test]
    fn cors_preflight_denies_unapproved_and_duplicate_origins() {
        let app = TestApp::new(POLICY);
        for headers in [
            vec![
                ("origin", "https://denied.example"),
                ("access-control-request-method", "POST"),
                ("access-control-request-headers", "content-type"),
            ],
            vec![
                ("origin", ORIGIN),
                ("access-control-request-method", "GET"),
                ("access-control-request-headers", "content-type"),
            ],
            vec![
                ("origin", ORIGIN),
                ("access-control-request-method", "POST"),
                ("access-control-request-headers", "x-secret"),
            ],
            vec![
                ("origin", ORIGIN),
                ("origin", ORIGIN),
                ("access-control-request-method", "POST"),
                ("access-control-request-headers", "content-type"),
            ],
            vec![
                ("origin", "null"),
                ("access-control-request-method", "POST"),
                ("access-control-request-headers", "content-type"),
            ],
        ] {
            let response = app.dispatch(Method::OPTIONS, "/cookie_sync", &headers, Body::empty());
            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{headers:?}");
            assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
            assert!(
                response
                    .headers()
                    .get("access-control-allow-origin")
                    .is_none(),
                "{headers:?}"
            );
            assert!(response.headers().get("set-cookie").is_none());
        }
        assert_eq!(app.store().reads(), 5);
    }

    #[test]
    fn denied_and_duplicate_actual_origins_do_not_grant_cors() {
        let app = TestApp::new(POLICY);
        for origins in [
            vec![("origin", "https://denied.example")],
            vec![("origin", "null")],
            vec![("origin", ORIGIN), ("origin", ORIGIN)],
        ] {
            let mut headers = origins;
            headers.push(("content-type", "application/json"));
            let response = app.dispatch(
                Method::POST,
                "/cookie_sync",
                &headers,
                Body::from_bytes(b"{".to_vec()),
            );
            assert_wire(
                &response,
                StatusCode::BAD_REQUEST,
                b"JSON parsing failed: invalid JSON\n",
                Some("text/plain; charset=utf-8"),
                false,
            );
        }
        assert_eq!(app.store().reads(), 3);
    }

    #[test]
    fn url_expansion_refusal_keeps_cors_and_never_writes_a_partial_cookie() {
        let app = TestApp::new(POLICY);
        let payload = serde_json::to_vec(&json!({
            "bidders": [live_catalog::redirect().name(), live_catalog::iframe().name()],
            "gdpr": 0_i32,
            "gdpr_consent": "<".repeat(10_000),
        }))
        .unwrap();
        let response = app.dispatch(
            Method::POST,
            "/cookie_sync",
            &[("origin", ORIGIN), ("content-type", "application/json")],
            Body::from_bytes(payload),
        );
        assert_wire(
            &response,
            StatusCode::BAD_REQUEST,
            b"sync URL exceeds resource limit\n",
            Some("text/plain; charset=utf-8"),
            true,
        );
    }

    #[test]
    fn chooser_name_rejection_precedes_oversized_url_expansion() {
        let app = TestApp::new(POLICY);
        let payload = serde_json::to_vec(&json!({
            "bidders": ["bad name"],
            "gdpr": 0_i32,
            "gdpr_consent": "<".repeat(10_000),
        }))
        .unwrap();
        let response = app.dispatch(
            Method::POST,
            "/cookie_sync",
            &[("origin", ORIGIN), ("content-type", "application/json")],
            Body::from_bytes(payload),
        );
        assert_wire(
            &response,
            StatusCode::BAD_REQUEST,
            b"invalid bidder list\n",
            Some("text/plain; charset=utf-8"),
            true,
        );
    }

    #[test]
    fn post_config_activity_denial_is_451_with_cors_and_no_cookie() {
        let app = TestApp::new(POLICY);
        let response = app.dispatch(
            Method::GET,
            &live_catalog::iframe().setuid("uid=buyer-1&account=denied&gdpr=0&f=b"),
            &[("origin", ORIGIN)],
            Body::empty(),
        );
        assert_wire(
            &response,
            StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
            b"",
            None,
            true,
        );
    }

    #[test]
    fn activity_denial_precedes_deferred_malformed_tcf_permission() {
        let app = TestApp::new(
            "[user_sync]\nallowed_origins = ['https://publisher.example']\n\
         [gdpr]\nenabled = true\nhost_vendor_id = 32\n\
         [gdpr.tcf2]\nenabled = true\n\
         [accounts.denied.privacy.allowactivities.syncUser]\ndefault = false\n",
        );
        let base =
            live_catalog::iframe().setuid("uid=buyer-1&gdpr=1&gdpr_consent=malformed-tcf&f=b");
        let denied = app.dispatch(
            Method::GET,
            &format!("{base}&account=denied"),
            &[("origin", ORIGIN)],
            Body::empty(),
        );
        assert_wire(
            &denied,
            StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
            b"",
            None,
            true,
        );
        let permitted = app.dispatch(Method::GET, &base, &[("origin", ORIGIN)], Body::empty());
        assert_wire(
            &permitted,
            StatusCode::BAD_REQUEST,
            b"gdpr_consent was invalid. malformed TCF consent",
            None,
            true,
        );
    }

    #[test]
    fn setuid_host_permission_denial_precedes_uid_and_cookie_serialization_limits() {
        let config = format!("{POLICY}{GDPR_POLICY}");
        let app = TestApp::new(&config);
        for uid_length in [cookie::MAX_SET_COOKIE_FIELD_BYTES, 24 * 1_024 + 1] {
            for purpose_one in [false, true] {
                let uri = format!(
                    "/setuid?bidder={}&uid={}&gdpr=1&gdpr_consent={}&f=i",
                    live_catalog::redirect().name(),
                    "x".repeat(uid_length),
                    consent(purpose_one),
                );
                assert!(uri.len() < 32 * 1_024, "must reach the permission stage");
                let response =
                    app.dispatch(Method::GET, &uri, &[("origin", ORIGIN)], Body::empty());
                let (status, expected) = if purpose_one {
                    (
                        StatusCode::BAD_REQUEST,
                        b"cookie resource limit exceeded".as_slice(),
                    )
                } else {
                    (
                        StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
                        b"The gdpr_consent string prevents cookies from being saved".as_slice(),
                    )
                };
                assert_wire(&response, status, expected, None, true);
            }
        }
        assert_eq!(app.store().reads(), 4);
    }

    #[test]
    fn setuid_disabled_base_syncer_precedes_alias_uid_and_cookie_transaction_limits() {
        let allowed = format!(
            "{POLICY}[aliases]\nfake_callback_alias = '{}'\n\
             [bidders.fake_callback_alias]\nusersync_enabled = true\n",
            live_catalog::redirect().name()
        );
        let disabled = format!(
            "{allowed}[bidders.'{}']\nusersync_enabled = false\n",
            live_catalog::redirect().name()
        );
        let app = TestApp::new(&disabled);
        for uid_length in [cookie::MAX_SET_COOKIE_FIELD_BYTES, 24 * 1_024 + 1] {
            let uri = format!(
                "/setuid?bidder=fake_callback_alias&uid={}&gdpr=0&f=b",
                "x".repeat(uid_length),
            );
            assert!(uri.len() < 32 * 1_024, "must reach syncer eligibility");
            for (config, expected) in [
                (&disabled, b"The bidder syncer is disabled".as_slice()),
                (&allowed, b"cookie resource limit exceeded".as_slice()),
            ] {
                app.store().set_config(config);
                let response =
                    app.dispatch(Method::GET, &uri, &[("origin", ORIGIN)], Body::empty());
                assert_wire(&response, StatusCode::BAD_REQUEST, expected, None, true);
            }
        }
        assert_eq!(app.store().reads(), 4);
    }

    #[test]
    fn setuid_host_permission_denial_precedes_disabled_syncer_check() {
        let config = format!(
            "{POLICY}{GDPR_POLICY}[bidders.'{}']\nusersync_enabled = false\n",
            live_catalog::redirect().name()
        );
        let app = TestApp::new(&config);
        for purpose_one in [false, true] {
            let uri = format!(
                "/setuid?bidder={}&uid=must-not-write&gdpr=1&gdpr_consent={}&f=b",
                live_catalog::redirect().name(),
                consent(purpose_one),
            );
            let response = app.dispatch(Method::GET, &uri, &[("origin", ORIGIN)], Body::empty());
            let (status, expected) = if purpose_one {
                (
                    StatusCode::BAD_REQUEST,
                    b"The bidder syncer is disabled".as_slice(),
                )
            } else {
                (
                    StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
                    b"The gdpr_consent string prevents cookies from being saved".as_slice(),
                )
            };
            assert_wire(&response, status, expected, None, true);
        }
        assert_eq!(app.store().reads(), 2);
    }

    #[test]
    fn getuids_host_optout_precedes_uid_merge_resource_refusal() {
        let app = TestApp::new(POLICY);
        let mut stored = UidCookie::default();
        stored
            .set_uid(
                live_catalog::redirect().family(),
                "stored-user",
                TEST_NOW,
                &CookieLimits::default(),
            )
            .unwrap();
        let encoded = encode_cookie(&stored, &CookieLimits::default()).unwrap();
        for (extra_cookie, expected_status, expected) in [
            ("optout=yes; ", StatusCode::OK, b"{}\n".as_slice()),
            (
                "",
                StatusCode::BAD_REQUEST,
                b"cookie resource limit exceeded".as_slice(),
            ),
        ] {
            let cookie = format!(
                "{extra_cookie}uids={encoded}; auctioneer_uid={}",
                "x".repeat(25_000),
            );
            let response = app.dispatch(
                Method::GET,
                "/getuids",
                &[("origin", ORIGIN), ("cookie", &cookie)],
                Body::empty(),
            );
            assert_wire(
                &response,
                expected_status,
                expected,
                (expected_status == StatusCode::OK).then_some("text/plain; charset=utf-8"),
                true,
            );
        }
        assert_eq!(app.store().reads(), 2);
    }

    #[test]
    fn config_failures_have_no_cors_and_store_outage_preserves_retry_after() {
        let app = TestApp::new(POLICY);
        app.store().set_config("[user_sync]\nexternal_url = 'http://unsafe.example'\nallowed_origins = ['https://publisher.example']\n");
        let invalid = app.dispatch(
            Method::GET,
            "/getuids",
            &[("origin", ORIGIN)],
            Body::empty(),
        );
        assert_wire(
            &invalid,
            StatusCode::INTERNAL_SERVER_ERROR,
            b"Malformed host configuration",
            Some("text/plain; charset=utf-8"),
            false,
        );

        app.store().set_unavailable();
        let outage = app.dispatch(
            Method::GET,
            "/getuids",
            &[("origin", ORIGIN)],
            Body::empty(),
        );
        assert_wire(
            &outage,
            StatusCode::SERVICE_UNAVAILABLE,
            b"Config store unavailable, retry after 5s",
            Some("text/plain; charset=utf-8"),
            false,
        );
        assert_eq!(outage.headers().get("retry-after").unwrap(), "5");
        assert_eq!(app.store().reads(), 2);
    }

    #[test]
    fn invalid_optout_and_callback_config_refuses_before_uid_write() {
        for invalid in [
            serde_json::json!({"host_cookie": {"optout_cookie": {"name": "opt{out}", "value": "1"}}}),
            serde_json::json!({"host_cookie": {"optout_cookie": {"name": "stop", "value": "bad\\value"}}}),
            serde_json::json!({"host_cookie": {"optout_cookie": {"name": "stop", "value": "bad\"value"}}}),
            serde_json::json!({"user_sync": {"redirect_url": "{{.ExternalURL}}/setuid?bidder={{.SyncerKey}}&x=}}&uid={{.UserMacro}}"}}),
        ] {
            let app = TestApp::new(&toml::to_string(&invalid).unwrap());
            let response = app.dispatch(
                Method::GET,
                &live_catalog::redirect().setuid("uid=must-not-write&gdpr=0&f=b"),
                &[
                    ("origin", ORIGIN),
                    ("cookie", "opt{out}=1; stop=bad\\value"),
                ],
                Body::empty(),
            );
            assert_wire(
                &response,
                StatusCode::INTERNAL_SERVER_ERROR,
                b"Malformed host configuration",
                Some("text/plain; charset=utf-8"),
                false,
            );
            assert_eq!(app.store().reads(), 1);
        }
    }

    #[test]
    fn unmatched_route_stays_outside_application_cors_and_config_resolution() {
        let app = TestApp::new(POLICY);
        let response = app.dispatch(
            Method::GET,
            "/not-a-sync-route",
            &[("origin", ORIGIN)],
            Body::empty(),
        );
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        assert!(
            response
                .headers()
                .get("access-control-allow-origin")
                .is_none()
        );
        assert!(response.headers().get("set-cookie").is_none());
        assert_eq!(app.store().reads(), 0);
    }

    #[test]
    fn callback_rechecks_policy_between_requests_but_cors_uses_one_snapshot() {
        let allowed =
            "[user_sync]\nallowed_origins = ['https://publisher.example']\n[accounts.publisher]\n";
        let denied = "[user_sync]\nallowed_origins = []\n[accounts.publisher]\ndisabled = true\n";
        let app = TestApp::new(allowed);
        app.store().replace_after_next_read(denied);
        let uri = live_catalog::iframe().setuid("uid=first&gdpr=0&f=b&account=publisher");
        let first = app.dispatch(Method::GET, &uri, &[("origin", ORIGIN)], Body::empty());
        assert_eq!(
            first.status(),
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(body(&first))
        );
        assert_eq!(
            first.headers().get("access-control-allow-origin").unwrap(),
            ORIGIN
        );
        assert!(first.headers().get("set-cookie").is_some());
        assert_eq!(app.store().reads(), 1, "CORS must use the same config read");

        let second = app.dispatch(Method::GET, &uri, &[("origin", ORIGIN)], Body::empty());
        assert_wire(
            &second,
            StatusCode::BAD_REQUEST,
            b"account is disabled, please reach out to the prebid server host",
            None,
            false,
        );
        assert_eq!(app.store().reads(), 2);

        app.store().set_config(allowed);
        let third = app.dispatch(Method::GET, &uri, &[("origin", ORIGIN)], Body::empty());
        assert_eq!(third.status(), StatusCode::OK);
        assert!(third.headers().get("set-cookie").is_some());
        assert_eq!(app.store().reads(), 3);
    }
}
