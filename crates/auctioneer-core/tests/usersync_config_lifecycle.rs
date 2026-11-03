//! Config replacement, refusal, and recovery through the generated App.

#[cfg(test)]
use auctioneer_core::test_support::live_catalog;

mod usersync_app;

#[cfg(test)]
use auctioneer_core::test_support::usersync_keys;

#[cfg(test)]
mod bounded_store;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use super::live_catalog;
    use super::usersync_app::{STORE_ERROR_CANARY, TEST_NOW, TestApp, body};
    use super::usersync_keys::{decode_cookie, encode_cookie};
    use auctioneer_core::usersync::{cookie::UidCookie, limits::CookieLimits};
    use cookie::Cookie;
    use edgezero_core::{
        blob_envelope::BlobEnvelope,
        body::Body,
        http::{Method, Response, StatusCode},
    };
    use serde_json::{Value, json};

    const ORIGIN: &str = "https://publisher.example";
    const ALLOWED: &str =
        "[user_sync]\nallowed_origins = ['https://publisher.example']\n[accounts.publisher]\n";
    const CONFIG_CANARY: &str = "synthetic-config-secret-id";
    const WRONG_SHA: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    #[derive(Clone, Copy)]
    enum Endpoint {
        CookieSync,
        Setuid,
        Getuids,
    }

    impl Endpoint {
        fn dispatch(self, app: &TestApp) -> Response {
            let before = app.store().reads();
            let response = match self {
                Self::CookieSync => app.dispatch(
                    Method::POST,
                    "/cookie_sync",
                    &[("origin", ORIGIN), ("content-type", "application/json")],
                    Body::from_bytes(
                        serde_json::to_vec(&json!({
                            "account": "publisher",
                            "bidders": [live_catalog::redirect().name()],
                            "gdpr": 0_i32,
                            "coopSync": false,
                        }))
                        .unwrap(),
                    ),
                ),
                Self::Setuid => app.dispatch(
                    Method::GET,
                    &format!(
                        "/setuid?bidder={}&uid=callback-user&account=publisher&gdpr=0&f=b",
                        live_catalog::redirect().name()
                    ),
                    &[("origin", ORIGIN)],
                    Body::empty(),
                ),
                Self::Getuids => {
                    let mut cookie = UidCookie::default();
                    cookie
                        .set_uid(
                            live_catalog::redirect().family(),
                            "stored-user",
                            TEST_NOW,
                            &CookieLimits::default(),
                        )
                        .unwrap();
                    let encoded = encode_cookie(&cookie, &CookieLimits::default()).unwrap();
                    let header = format!("uids={encoded}; stop=yes");
                    app.dispatch(
                        Method::GET,
                        "/getuids",
                        &[("origin", ORIGIN), ("cookie", &header)],
                        Body::empty(),
                    )
                }
            };
            assert_eq!(
                app.store().reads().checked_sub(before),
                Some(1),
                "each request must read exactly one config snapshot"
            );
            response
        }
    }

    fn denied_config(cors: bool) -> String {
        toml::to_string(&json!({
            "user_sync": {"allowed_origins": if cors { vec![ORIGIN] } else { vec![] }},
            "accounts": {"publisher": {"privacy": {"allowactivities": {
                "syncUser": {"default": false}
            }}}},
            "host_cookie": {"optout_cookie": {"name": "stop", "value": "yes"}},
        }))
        .unwrap()
    }

    fn assert_browser_headers(response: &Response, cors: bool) {
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-origin")
                .map(|value| value.to_str().unwrap()),
            cors.then_some(ORIGIN)
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
        } else {
            assert!(response.headers().get("vary").is_none());
            assert!(
                response
                    .headers()
                    .keys()
                    .all(|name| !name.as_str().starts_with("access-control-"))
            );
        }
    }

    fn assert_json(response: &Response, expected: &Value, content_type: &str) {
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            content_type
        );
        assert!(body(response).ends_with(b"\n"));
        assert_eq!(
            serde_json::from_slice::<Value>(body(response)).unwrap(),
            *expected
        );
    }

    fn assert_allowed(endpoint: Endpoint, response: &Response) {
        assert_eq!(response.status(), StatusCode::OK);
        assert_browser_headers(response, true);
        assert!(response.headers().get("retry-after").is_none());
        match endpoint {
            Endpoint::CookieSync => {
                assert_eq!(
                    response.headers().get("content-type").unwrap(),
                    "application/json; charset=utf-8"
                );
                assert!(body(response).ends_with(b"\n"));
                let value: Value = serde_json::from_slice(body(response)).unwrap();
                assert_eq!(value["status"], "no_cookie");
                let bidders = value["bidder_status"].as_array().unwrap();
                assert_eq!(bidders.len(), 1);
                assert_eq!(bidders[0]["bidder"], live_catalog::redirect().name());
                assert_eq!(bidders[0]["usersync"]["type"], "redirect");
                let callback_url = live_catalog::redirect()
                    .callback(bidders[0]["usersync"]["url"].as_str().unwrap());
                assert_eq!(callback_url.path(), "/setuid");
                assert!(
                    callback_url
                        .query_pairs()
                        .any(|(key, query_value)| key == "account" && query_value == "publisher")
                );
                assert!(response.headers().get("set-cookie").is_none());
            }
            Endpoint::Setuid => {
                assert!(body(response).is_empty());
                assert_eq!(response.headers().get("content-type").unwrap(), "text/html");
                let field = response
                    .headers()
                    .get("set-cookie")
                    .unwrap()
                    .to_str()
                    .unwrap();
                let cookie = Cookie::parse(field.to_owned()).unwrap();
                assert_eq!(cookie.name(), "uids");
                let decoded = decode_cookie(cookie.value(), &CookieLimits::default()).unwrap();
                assert!(!decoded.corrupt);
                assert_eq!(
                    decoded.cookie.uid(live_catalog::redirect().family()),
                    Some("callback-user")
                );
            }
            Endpoint::Getuids => {
                assert_json(
                    response,
                    &json!({"buyeruids": {(live_catalog::redirect().family()): "stored-user"}}),
                    "text/plain; charset=utf-8",
                );
                assert!(response.headers().get("set-cookie").is_none());
            }
        }
    }

    fn assert_denied(endpoint: Endpoint, response: &Response, cors: bool) {
        assert_browser_headers(response, cors);
        assert!(response.headers().get("set-cookie").is_none());
        assert!(response.headers().get("retry-after").is_none());
        match endpoint {
            Endpoint::CookieSync => {
                assert_eq!(response.status(), StatusCode::OK);
                assert_json(
                    response,
                    &json!({"status": "no_cookie", "bidder_status": []}),
                    "application/json; charset=utf-8",
                );
            }
            Endpoint::Setuid => {
                assert_eq!(response.status(), StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS);
                assert!(body(response).is_empty());
                assert!(response.headers().get("content-type").is_none());
            }
            Endpoint::Getuids => {
                assert_eq!(response.status(), StatusCode::OK);
                assert_json(response, &json!({}), "text/plain; charset=utf-8");
            }
        }
    }

    fn assert_config_failure(response: &Response, transient: bool, reason: &str) {
        let (status, expected) = if transient {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                b"Config store unavailable, retry after 5s".as_slice(),
            )
        } else {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                b"Malformed host configuration".as_slice(),
            )
        };
        assert_eq!(response.status(), status);
        assert_eq!(body(response), expected);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/plain; charset=utf-8"
        );
        assert_browser_headers(response, false);
        assert!(response.headers().get("set-cookie").is_none());
        assert_eq!(
            response
                .headers()
                .get("retry-after")
                .map(|value| value.to_str().unwrap()),
            transient.then_some("5")
        );
        testing_logger::validate(|captured| {
            assert!(
                captured.iter().any(|line| {
                    line.body
                        .contains(&format!("config store extraction failed: {reason}"))
                }),
                "the logged error detail must identify the failure category"
            );
            for line in captured {
                for secret in [CONFIG_CANARY, STORE_ERROR_CANARY] {
                    assert!(
                        !line.body.contains(secret),
                        "synthetic secret leaked into error detail"
                    );
                }
            }
        });
    }

    fn encoded_envelope(data: Value) -> String {
        serde_json::to_string(&BlobEnvelope::new(data, "1970-01-01T00:00:00Z".to_owned())).unwrap()
    }

    fn assert_lifecycle(endpoint: Endpoint) {
        const { assert!(!auctioneer_config::RUNTIME_BOOTSTRAP.allow_missing_config) };
        testing_logger::setup();
        let app = TestApp::new(ALLOWED);
        assert_allowed(endpoint, &endpoint.dispatch(&app));

        app.store().set_config(&denied_config(true));
        assert_denied(endpoint, &endpoint.dispatch(&app), true);

        let mut tampered = BlobEnvelope::new(
            json!({"user_sync": {
                "allowed_origins": [ORIGIN],
                "external_url": format!("https://{CONFIG_CANARY}.example"),
            }}),
            "1970-01-01T00:00:00Z".to_owned(),
        );
        tampered.sha256 = WRONG_SHA.to_owned();
        // Deliberately incomplete JSON exercises envelope syntax, not typed config validation.
        let raw_invalid = format!("{{\"secret\":\"{CONFIG_CANARY}\",");
        for (raw, reason) in [
            (raw_invalid, "MalformedEnvelope"),
            (
                serde_json::to_string(&tampered).unwrap(),
                "IntegrityMismatch",
            ),
            (
                encoded_envelope(json!({
                    "user_sync": {"allowed_origins": [ORIGIN]},
                    "auction": {"default_tmax_ms": CONFIG_CANARY},
                })),
                "Deserialization",
            ),
            (
                encoded_envelope(json!({
                    "user_sync": {"allowed_origins": [ORIGIN]},
                    "bidders": {(live_catalog::iframe().name()): {"timeout_ms": 0_i32}},
                })),
                "Validation",
            ),
        ] {
            app.store().set_blob(Some(raw));
            testing_logger::setup();
            assert_config_failure(&endpoint.dispatch(&app), false, reason);
        }

        app.store().set_unavailable();
        testing_logger::setup();
        assert_config_failure(&endpoint.dispatch(&app), true, "BackendUnavailable");

        app.store().set_blob(None);
        testing_logger::setup();
        assert_config_failure(&endpoint.dispatch(&app), false, "MissingBlob");

        app.store().set_config(ALLOWED);
        assert_allowed(endpoint, &endpoint.dispatch(&app));
        assert_eq!(app.store().reads(), 9);
    }

    #[test]
    fn cookie_sync_refuses_untrusted_config_until_a_valid_replacement_recovers() {
        assert_lifecycle(Endpoint::CookieSync);
    }

    #[test]
    fn setuid_refuses_untrusted_config_until_a_valid_replacement_recovers() {
        assert_lifecycle(Endpoint::Setuid);
    }

    #[test]
    fn getuids_refuses_untrusted_config_until_a_valid_replacement_recovers() {
        assert_lifecycle(Endpoint::Getuids);
    }

    fn assert_snapshot_rechecked(endpoint: Endpoint) {
        testing_logger::setup();
        let app = TestApp::new(ALLOWED);
        app.store().replace_after_next_read(&denied_config(false));
        assert_allowed(endpoint, &endpoint.dispatch(&app));
        assert_denied(endpoint, &endpoint.dispatch(&app), false);
        app.store().set_config(ALLOWED);
        assert_allowed(endpoint, &endpoint.dispatch(&app));
        assert_eq!(app.store().reads(), 3);
    }

    #[test]
    fn cookie_sync_keeps_one_snapshot_for_selection_and_cors_then_rechecks_next_request() {
        assert_snapshot_rechecked(Endpoint::CookieSync);
    }

    #[test]
    fn getuids_keeps_one_snapshot_for_cookie_read_and_cors_then_rechecks_next_request() {
        assert_snapshot_rechecked(Endpoint::Getuids);
    }
}
