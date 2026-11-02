//! Generated-App acceptance of the issued-cookie integrity boundary.

#[cfg(test)]
use auctioneer_core::test_support::live_catalog;

#[expect(
    dead_code,
    reason = "shared App support offers helpers used by other targets"
)]
mod usersync_app;

use auctioneer_core::test_support::usersync_keys;

#[cfg(test)]
mod bounded_store;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use auctioneer_core::usersync::limits::CookieLimits;
    use base64::{
        Engine as _,
        engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
    };
    use edgezero_core::{
        body::Body,
        http::{Method, StatusCode},
    };
    use hmac::{Hmac, Mac as _};
    use serde_json::json;
    use sha2::Sha256;
    use usersync_app::{TestApp, body};

    use super::{live_catalog, usersync_app, usersync_keys};

    #[test]
    fn unsigned_uid_state_is_rejected_by_every_http_reader() {
        let app = TestApp::new("");
        let payload = json!({
            "tempUIDs": {(live_catalog::redirect().family()): {"uid": "caller-chosen-uid", "expires": "2099-01-01T00:00:00Z"}}
        });
        let cookie = format!(
            "uids={}",
            URL_SAFE.encode(serde_json::to_vec(&payload).unwrap())
        );
        for (method, uri, request_body) in [
            (
                Method::POST,
                "/cookie_sync",
                serde_json::to_vec(
                    &json!({"gdpr": 0_i32, "bidders": [live_catalog::redirect().name()]}),
                )
                .unwrap(),
            ),
            (Method::GET, "/getuids", Vec::new()),
            (
                Method::GET,
                &live_catalog::redirect().setuid("gdpr=0&uid=new-uid"),
                Vec::new(),
            ),
        ] {
            let response = app.dispatch(
                method,
                uri,
                &[("cookie", &cookie)],
                Body::from_bytes(request_body),
            );
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
            let expected: &[u8] = if uri == "/cookie_sync" {
                b"invalid UID cookie\n"
            } else {
                b"invalid UID cookie"
            };
            assert_eq!(body(&response), expected, "{uri}");
            assert!(!response.headers().contains_key("set-cookie"), "{uri}");
        }
    }

    #[test]
    fn unavailable_keys_refuse_state_without_polling_earlier_denials() {
        let app = TestApp::new(
            "[host_cookie.optout_cookie]\nname='optout'\nvalue='yes'\n[accounts.closed]\ndisabled=true\n",
        );
        app.secrets().set(None);
        let denied = app.dispatch(
            Method::GET,
            &live_catalog::redirect().setuid("account=closed"),
            &[],
            Body::empty(),
        );
        assert_eq!(denied.status(), StatusCode::BAD_REQUEST);
        assert_eq!(app.secrets().reads(), 0);
        let opted_out = app.dispatch(
            Method::POST,
            "/cookie_sync",
            &[("cookie", "optout=yes")],
            Body::from_bytes(b"{}".to_vec()),
        );
        assert_eq!(opted_out.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(app.secrets().reads(), 0);
        let readback = app.dispatch(
            Method::GET,
            "/getuids",
            &[("cookie", "optout=yes")],
            Body::empty(),
        );
        assert_eq!(readback.status(), StatusCode::OK);
        assert_eq!(body(&readback), b"{}\n");
        assert_eq!(app.secrets().reads(), 0);
        for (method, uri, payload) in [
            (Method::GET, "/getuids", Vec::new()),
            (
                Method::GET,
                &live_catalog::redirect().setuid("gdpr=0&uid=new"),
                Vec::new(),
            ),
            (Method::POST, "/cookie_sync", b"{\"gdpr\":0}".to_vec()),
        ] {
            let response = app.dispatch(method, uri, &[], Body::from_bytes(payload));
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{uri}");
            assert!(!response.headers().contains_key("set-cookie"));
        }
        assert_eq!(app.secrets().reads(), 3);
        let preflight = app.dispatch(Method::OPTIONS, "/cookie_sync", &[], Body::empty());
        assert_eq!(preflight.status(), StatusCode::FORBIDDEN);
        assert_eq!(app.secrets().reads(), 3);
    }

    #[test]
    fn rotation_uses_one_current_secret_snapshot_per_request() {
        let app = TestApp::new("");
        let callback = app.dispatch(
            Method::GET,
            &live_catalog::redirect().setuid("gdpr=0&uid=owned-hint"),
            &[],
            Body::empty(),
        );
        assert_eq!(callback.status(), StatusCode::OK);
        assert_eq!(app.secrets().reads(), 1);
        let header = callback.headers()["set-cookie"].to_str().unwrap();
        let pair = header.split(';').next().unwrap();
        assert!(pair.starts_with("uids=v1.test."));
        let original = pair.strip_prefix("uids=").unwrap();
        assert_eq!(
            usersync_keys::decode_cookie(original, &CookieLimits::default())
                .unwrap()
                .cookie
                .uid(live_catalog::redirect().family()),
            Some("owned-hint")
        );

        let replacement = URL_SAFE.encode([2_u8; 32]).trim_end_matches('=').to_owned();
        let retained = serde_json::to_vec(&json!({"active":"new", "keys":[
            {"id":"test", "key":"AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE"},
            {"id":"new", "key":replacement}
        ]}))
        .unwrap();
        app.secrets().set(Some(&retained));
        let retained_read =
            app.dispatch(Method::GET, "/getuids", &[("cookie", pair)], Body::empty());
        assert_eq!(retained_read.status(), StatusCode::OK);
        let next = app.dispatch(
            Method::GET,
            &live_catalog::redirect().setuid("gdpr=0&uid=new-hint"),
            &[("cookie", pair)],
            Body::empty(),
        );
        assert_eq!(next.status(), StatusCode::OK);
        assert!(
            next.headers()["set-cookie"]
                .to_str()
                .unwrap()
                .starts_with("uids=v1.new.")
        );

        let retired =
            serde_json::to_vec(&json!({"active":"new", "keys":[{"id":"new", "key":replacement}]}))
                .unwrap();
        app.secrets().set(Some(&retired));
        let response = app.dispatch(Method::GET, "/getuids", &[("cookie", pair)], Body::empty());
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body(&response), b"invalid UID cookie");
        assert_eq!(app.secrets().reads(), 4);
        assert!(!response.headers().contains_key("set-cookie"));
    }

    #[test]
    fn malformed_or_oversized_keyrings_never_become_unsigned_writes() {
        let app = TestApp::new("");
        for keyring in [b"private-keyring-canary".to_vec(), vec![b' '; 4_097]] {
            app.secrets().set(Some(&keyring));
            let response = app.dispatch(
                Method::GET,
                &live_catalog::redirect().setuid("gdpr=0&uid=new"),
                &[],
                Body::empty(),
            );
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(body(&response), b"user sync signing unavailable");
            assert!(!response.headers().contains_key("set-cookie"));
            assert!(!String::from_utf8_lossy(body(&response)).contains("private-keyring-canary"));
        }
        app.secrets().unavailable();
        let response = app.dispatch(Method::GET, "/getuids", &[], Body::empty());
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    fn assert_keyring_unavailable_to_every_http_reader(secret: &serde_json::Value) {
        let app = TestApp::new("");
        app.secrets()
            .set(Some(&serde_json::to_vec(secret).unwrap()));
        for (method, uri, payload) in [
            (Method::GET, "/getuids", Vec::new()),
            (
                Method::GET,
                &live_catalog::redirect().setuid("gdpr=0&uid=new"),
                Vec::new(),
            ),
            (Method::POST, "/cookie_sync", b"{\"gdpr\":0}".to_vec()),
        ] {
            let reads = app.secrets().reads();
            let response = app.dispatch(method, uri, &[], Body::from_bytes(payload));
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{uri}");
            let expected: &[u8] = if uri == "/cookie_sync" {
                b"user sync signing unavailable\n"
            } else {
                b"user sync signing unavailable"
            };
            assert_eq!(body(&response), expected, "{uri}");
            assert!(!response.headers().contains_key("set-cookie"), "{uri}");
            assert_eq!(
                app.secrets().reads(),
                reads.checked_add(1).unwrap(),
                "{uri}"
            );
        }
    }

    #[test]
    fn positional_keyring_roots_are_unavailable_to_every_http_reader() {
        let key = URL_SAFE_NO_PAD.encode([1_u8; 32]);
        assert_keyring_unavailable_to_every_http_reader(
            &json!(["test", [{"id":"test", "key":key}]]),
        );
    }

    #[test]
    fn positional_key_records_are_unavailable_to_every_http_reader() {
        let key = URL_SAFE_NO_PAD.encode([1_u8; 32]);
        assert_keyring_unavailable_to_every_http_reader(
            &json!({"active":"test", "keys":[["test", key]]}),
        );
    }

    #[test]
    fn every_http_reader_rejects_an_authentic_mac_for_an_unsupported_version() {
        let app = TestApp::new("");
        let payload = URL_SAFE.encode(
            serde_json::to_vec(&json!({
                "tempUIDs":{(live_catalog::redirect().family()):{"uid":"synthetic-user","expires":"2099-01-01T00:00:00Z"}}
            }))
            .unwrap(),
        );
        for version in ["v1", "v2"] {
            let prefix = format!("{version}.test.{payload}");
            let mut mac = Hmac::<Sha256>::new_from_slice(&[1_u8; 32]).unwrap();
            mac.update(b"auctioneer.uids.v1\0");
            mac.update(prefix.as_bytes());
            let header = format!(
                "uids={prefix}.{}",
                URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
            );
            for (method, uri, request_body) in [
                (Method::GET, "/getuids", Vec::new()),
                (
                    Method::GET,
                    &live_catalog::redirect().setuid("gdpr=0&uid=new"),
                    Vec::new(),
                ),
                (Method::POST, "/cookie_sync", b"{\"gdpr\":0}".to_vec()),
            ] {
                let response = app.dispatch(
                    method,
                    uri,
                    &[("cookie", &header)],
                    Body::from_bytes(request_body),
                );
                if version == "v1" {
                    assert_eq!(response.status(), StatusCode::OK, "{uri}");
                } else {
                    assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
                    let expected: &[u8] = if uri == "/cookie_sync" {
                        b"invalid UID cookie\n"
                    } else {
                        b"invalid UID cookie"
                    };
                    assert_eq!(body(&response), expected, "{uri}");
                    assert!(!response.headers().contains_key("set-cookie"), "{uri}");
                }
            }
        }
        assert_eq!(app.secrets().reads(), 6);
    }

    #[test]
    fn every_http_reader_rejects_tampered_fields_without_a_cookie_write() {
        let app = TestApp::new("");
        let issued = app.dispatch(
            Method::GET,
            &live_catalog::redirect().setuid("gdpr=0&uid=original"),
            &[],
            Body::empty(),
        );
        assert_eq!(issued.status(), StatusCode::OK);
        let signed = issued.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .strip_prefix("uids=")
            .unwrap();
        let fields = signed.split('.').collect::<Vec<_>>();
        for (index, replacement) in [
            (0, "v2"),
            (1, "unknown"),
            (2, "e30="),
            (3, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
        ] {
            let mut altered = fields.clone();
            altered[index] = replacement;
            let header = format!("uids={}", altered.join("."));
            for (method, uri, payload) in [
                (Method::GET, "/getuids", Vec::new()),
                (
                    Method::GET,
                    &live_catalog::redirect().setuid("gdpr=0&uid=next"),
                    Vec::new(),
                ),
                (
                    Method::POST,
                    "/cookie_sync",
                    serde_json::to_vec(&json!({"gdpr":0_i32})).unwrap(),
                ),
            ] {
                let response = app.dispatch(
                    method,
                    uri,
                    &[("cookie", &header)],
                    Body::from_bytes(payload),
                );
                assert_eq!(
                    response.status(),
                    StatusCode::BAD_REQUEST,
                    "{uri}, field {index}"
                );
                assert!(!response.headers().contains_key("set-cookie"));
            }
        }
        assert_eq!(app.secrets().reads(), 13);
    }

    #[test]
    fn deletion_matches_cookie_scope_and_empty_readback_needs_no_legacy_state() {
        let app = TestApp::new("[host_cookie]\ndomain='publisher.example'\n");
        let issued = app.dispatch(
            Method::GET,
            &live_catalog::redirect().setuid("gdpr=0&uid=original"),
            &[("user-agent", "Chrome/130.0")],
            Body::empty(),
        );
        assert_eq!(issued.status(), StatusCode::OK);
        let pair = issued.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap();
        let removed = app.dispatch(
            Method::GET,
            &live_catalog::redirect().setuid("gdpr=0&uid="),
            &[("cookie", pair), ("user-agent", "Chrome/130.0")],
            Body::empty(),
        );
        assert_eq!(removed.status(), StatusCode::OK);
        let value = removed.headers()["set-cookie"].to_str().unwrap();
        for part in [
            "uids=",
            "Domain=publisher.example",
            "Path=/",
            "Max-Age=0",
            "Expires=Thu, 01 Jan 1970 00:00:00 GMT",
            "Secure",
            "SameSite=None",
        ] {
            assert!(
                value.split("; ").any(|field| field == part),
                "missing {part}: {value}"
            );
        }
        assert!(value.len() <= 4096);
        let empty = app.dispatch(Method::GET, "/getuids", &[], Body::empty());
        assert_eq!(empty.status(), StatusCode::OK);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(body(&empty)).unwrap(),
            json!({})
        );
    }
}
