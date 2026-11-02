//! Routed and cross-endpoint user-sync flows.

#[cfg(test)]
use auctioneer_core::test_support::live_catalog;

#[expect(
    dead_code,
    reason = "shared support exposes helpers used by other test targets"
)]
mod usersync_app;

#[cfg(test)]
use auctioneer_core::test_support::usersync_keys;

#[cfg(test)]
mod bounded_store;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use super::usersync_keys::decode_cookie;
    use super::{live_catalog, usersync_app};
    use auctioneer_core::{
        config::{AuctioneerAppConfig, UserSyncOverride},
        handlers::sync_http::serialized_header_bytes,
        usersync::{cookie::MAX_SET_COOKIE_FIELD_BYTES, limits::CookieLimits},
    };
    use edgezero_core::body::Body;
    use http::{HeaderMap, HeaderValue, Method, StatusCode, header::COOKIE};
    use std::collections::BTreeMap;
    use usersync_app::body;

    #[test]
    fn setuid_write_is_read_back_by_getuids_without_a_second_write() {
        let app = usersync_app::TestApp::new("");
        let response = app.dispatch(
            Method::GET,
            &format!(
                "/setuid?bidder={}&uid=partner-123&f=i&gdpr=0",
                live_catalog::redirect().name()
            ),
            &[],
            Body::empty(),
        );
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body(&response).len(), 86);
        let set_cookie = response
            .headers()
            .get("set-cookie")
            .unwrap()
            .to_str()
            .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            HeaderValue::from_str(set_cookie.split(';').next().unwrap()).unwrap(),
        );
        let readback = app.dispatch(
            Method::GET,
            "/getuids",
            &[("cookie", headers[COOKIE].to_str().unwrap())],
            Body::empty(),
        );
        assert_eq!(readback.status(), StatusCode::OK);
        assert!(body(&readback).ends_with(b"\n"));
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(body(&readback)).unwrap(),
            serde_json::json!({"buyeruids": {(live_catalog::redirect().family()): "partner-123"}})
        );
        assert!(!readback.headers().contains_key("set-cookie"));
    }

    #[test]
    fn setuid_near_limit_cookie_does_not_consume_other_header_budget() {
        let app = usersync_app::TestApp::new(
            "[user_sync]\nallowed_origins = ['https://publisher.example']\n[host_cookie]\nmax_cookie_size_bytes = 0\n",
        );
        assert_eq!(MAX_SET_COOKIE_FIELD_BYTES, 4096);
        let dispatch = |uid_length: usize| {
            app.dispatch(
                Method::GET,
                &format!(
                    "/setuid?bidder={}&uid={}&f=b&gdpr=0",
                    live_catalog::redirect().name(),
                    "a".repeat(uid_length)
                ),
                &[("origin", "https://publisher.example")],
                Body::empty(),
            )
        };
        let (mut fits, mut exceeds) = (1_usize, MAX_SET_COOKIE_FIELD_BYTES);
        assert_eq!(dispatch(fits).status(), StatusCode::OK);
        assert_eq!(dispatch(exceeds).status(), StatusCode::BAD_REQUEST);
        while exceeds - fits > 1 {
            let middle = fits.midpoint(exceeds);
            let probe = dispatch(middle);
            match probe.status() {
                StatusCode::OK => fits = middle,
                StatusCode::BAD_REQUEST => {
                    assert_eq!(body(&probe), b"cookie resource limit exceeded");
                    assert!(!probe.headers().contains_key("set-cookie"));
                    exceeds = middle;
                }
                status => panic!("unexpected boundary status {status}"),
            }
        }
        let response = dispatch(fits);

        assert_eq!(response.status(), StatusCode::OK);
        let set_cookie = response.headers()["set-cookie"].as_bytes();
        assert!(set_cookie.len() > MAX_SET_COOKIE_FIELD_BYTES - 4);
        assert!(set_cookie.len() <= MAX_SET_COOKIE_FIELD_BYTES);
        let all_headers = serialized_header_bytes(response.headers()).unwrap();
        let other_headers = all_headers - ("set-cookie: \r\n".len() + set_cookie.len());
        assert!(all_headers > MAX_SET_COOKIE_FIELD_BYTES);
        assert!(other_headers <= 16 * 1024);
        assert_eq!(
            response.headers()["access-control-allow-origin"],
            "https://publisher.example"
        );
        assert_eq!(response.headers()["cache-control"], "no-store");
        let refused = dispatch(exceeds);
        assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body(&refused), b"cookie resource limit exceeded");
        assert!(!refused.headers().contains_key("set-cookie"));
        assert_eq!(
            refused.headers()["access-control-allow-origin"],
            "https://publisher.example"
        );
    }

    #[test]
    fn cookie_sync_callback_preserves_normalized_bidder_identity_not_family() {
        let role = live_catalog::redirect();
        let uppercase = role.name().to_ascii_uppercase();
        for (requested, alias, expected) in [
            (role.name(), None, role.name()),
            (uppercase.as_str(), None, role.name()),
            (
                "fake_CallbackAlias",
                Some("fake_CallbackAlias"),
                "fake_callbackalias",
            ),
        ] {
            for callback_token in ["SyncerKey", "BidderName"] {
                let mut config = AuctioneerAppConfig::default();
                if let Some(alias_name) = alias {
                    config.aliases = Some(BTreeMap::from([(
                        alias_name.to_owned(),
                        role.name().to_owned(),
                    )]));
                }
                assert!(
                    !config
                        .aliases
                        .as_ref()
                        .is_some_and(|aliases| aliases.contains_key(role.family()))
                );
                if callback_token == "BidderName" {
                    config.user_sync = Some(UserSyncOverride {
                        redirect_url: Some("{{.ExternalURL}}/setuid?bidder={{.BidderName}}&gdpr={{.GDPR}}&f={{.SyncType}}&uid={{.UserMacro}}".to_owned()),
                        ..UserSyncOverride::default()
                    });
                }
                let app = usersync_app::TestApp::new(&toml::to_string(&config).unwrap());
                let raw = serde_json::to_vec(&serde_json::json!({
                    "bidders": [requested],
                    "gdpr": 0_i32,
                    "coopSync": false,
                }))
                .unwrap();
                let response = app.dispatch(
                    Method::POST,
                    "/cookie_sync",
                    &[("content-type", "application/json")],
                    Body::from_bytes(raw),
                );
                assert_eq!(response.status(), StatusCode::OK);
                let payload: serde_json::Value = serde_json::from_slice(body(&response)).unwrap();
                let statuses = payload["bidder_status"].as_array().unwrap();
                assert_eq!(statuses.len(), 1);
                assert_eq!(statuses[0]["bidder"], requested);
                let request_target = role.callback_uri(
                    statuses[0]["usersync"]["url"].as_str().unwrap(),
                    "partner-123",
                );
                let callback_url =
                    url::Url::parse(&format!("https://callback.example{request_target}")).unwrap();
                let bidder = callback_url
                    .query_pairs()
                    .find(|(key, _)| key == "bidder")
                    .unwrap()
                    .1;
                assert_eq!(bidder, expected, "{requested}: {callback_token}");

                let setuid_response =
                    app.dispatch(Method::GET, &request_target, &[], Body::empty());
                assert_eq!(setuid_response.status(), StatusCode::OK);
                let written = cookie::Cookie::parse(
                    setuid_response.headers()["set-cookie"].to_str().unwrap(),
                )
                .unwrap();
                let decoded = decode_cookie(written.value(), &CookieLimits::default()).unwrap();
                assert_eq!(decoded.cookie.uid(role.family()), Some("partner-123"));
                assert!(decoded.cookie.uid(expected).is_none());
            }
        }
    }

    #[test]
    fn cookie_sync_routed_missing_consent_uses_numeric_sid_scope() {
        let app = usersync_app::TestApp::new(
            "[user_sync]\nallowed_origins = ['https://publisher.example']\n",
        );
        for sid in ["02", "00002", "6,02", "02,02"] {
            let raw = serde_json::to_vec(&serde_json::json!({
                "gdpr": 0_i32,
                "gpp_sid": sid,
            }))
            .unwrap();
            let response = app.dispatch(
                Method::POST,
                "/cookie_sync",
                &[
                    ("content-type", "application/json"),
                    ("origin", "https://publisher.example"),
                ],
                Body::from_bytes(raw),
            );
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{sid}");
            assert_eq!(
                body(&response),
                b"gdpr_consent is required if gdpr=1\n",
                "{sid}"
            );
            assert_eq!(response.headers()["cache-control"], "no-store");
            assert_eq!(
                response.headers()["access-control-allow-origin"],
                "https://publisher.example"
            );
            assert!(!response.headers().contains_key("set-cookie"));
        }
    }

    #[test]
    fn cookie_sync_routed_numeric_sid_scope_preserves_raw_callback_macro() {
        let app = usersync_app::TestApp::new("[gdpr]\nenabled = false\n");
        for (sid, expected_gdpr) in [("02", "1"), ("6,02", "1"), ("-1", "0")] {
            let raw = serde_json::to_vec(&serde_json::json!({
                "bidders": [live_catalog::redirect().name()],
                "gdpr": 0_i32,
                "gdpr_consent": "opaque-consent",
                "gpp_sid": sid,
                "coopSync": false,
            }))
            .unwrap();
            let response = app.dispatch(
                Method::POST,
                "/cookie_sync",
                &[("content-type", "application/json")],
                Body::from_bytes(raw),
            );
            assert_eq!(response.status(), StatusCode::OK, "{sid}");
            let payload: serde_json::Value = serde_json::from_slice(body(&response)).unwrap();
            let statuses = payload["bidder_status"].as_array().unwrap();
            assert_eq!(statuses.len(), 1);
            let callback_url =
                live_catalog::redirect().callback(statuses[0]["usersync"]["url"].as_str().unwrap());
            let values: BTreeMap<_, _> = callback_url.query_pairs().collect();
            assert_eq!(values["gdpr"], expected_gdpr, "{sid}");
            assert_eq!(values["gpp_sid"], sid);
            assert_eq!(values["gdpr_consent"], "opaque-consent");
        }
    }
}
