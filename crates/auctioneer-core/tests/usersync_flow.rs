#[cfg(test)]
use auctioneer_core::test_support::live_catalog;

#[expect(
    dead_code,
    reason = "the flow binary does not change its fixture store"
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
    use super::{
        live_catalog,
        usersync_app::{TEST_NOW, TestApp, body},
        usersync_keys::decode_cookie,
    };
    use auctioneer_core::usersync::limits::CookieLimits;
    use cookie::Cookie;
    use edgezero_core::{
        body::Body,
        http::{Method, Response, StatusCode},
    };
    use serde_json::{Value, json};

    const PUBLISHER_ORIGIN: &str = "https://publisher.example";

    fn assert_browser_headers(response: &Response, allowed: bool) {
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        if allowed {
            assert_eq!(
                response
                    .headers()
                    .get("access-control-allow-origin")
                    .unwrap(),
                PUBLISHER_ORIGIN
            );
            assert_eq!(
                response
                    .headers()
                    .get("access-control-allow-credentials")
                    .unwrap(),
                "true"
            );
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
            assert!(
                response
                    .headers()
                    .get("access-control-allow-origin")
                    .is_none()
            );
            assert!(
                response
                    .headers()
                    .get("access-control-allow-credentials")
                    .is_none()
            );
        }
    }

    fn redirect_callback_uri(selected: &Value, account: &str, uid: &str) -> String {
        let role = live_catalog::redirect();
        let rendered = selected["usersync"]["url"].as_str().unwrap();
        let callback = role.callback(rendered);
        assert_eq!(
            callback
                .query_pairs()
                .find(|(key, _)| key == "account")
                .unwrap()
                .1,
            account
        );
        assert_eq!(
            callback
                .query_pairs()
                .find(|(key, _)| key == "uid")
                .unwrap()
                .1,
            role.user_macro()
        );
        role.callback_uri(rendered, uid)
    }

    fn assert_buyeruid(response: &Response, family: &str, uid: &str) {
        assert!(body(response).ends_with(b"\n"));
        assert_eq!(
            serde_json::from_slice::<Value>(body(response)).unwrap(),
            json!({"buyeruids": {(family): uid}})
        );
    }

    fn carried_cookie(response: &Response, family: &str, uid: &str) -> String {
        let set_cookie = response
            .headers()
            .get("set-cookie")
            .unwrap()
            .to_str()
            .unwrap();
        let parsed = Cookie::parse(set_cookie.to_owned()).unwrap();
        assert_eq!(parsed.name(), "uids");
        assert_eq!(parsed.path(), Some("/"));
        assert!(parsed.expires().is_some());
        assert!(parsed.value().starts_with("v1.test."));
        assert!(set_cookie.len() <= 4096);
        let decoded = decode_cookie(parsed.value(), &CookieLimits::default()).unwrap();
        assert_eq!(decoded.cookie.uid(family), Some(uid));
        let expiry = decoded.cookie.entries()[family].expires();
        assert!(expiry > TEST_NOW);
        assert!(
            expiry
                < TEST_NOW
                    .checked_add(time::Duration::days(15))
                    .expect("date range")
        );
        format!("{}={}", parsed.name(), parsed.value())
    }

    #[test]
    fn redirect_browser_round_trip_carries_cookie_and_named_account() {
        let role = live_catalog::redirect();
        let app = TestApp::new(&format!(
            "[user_sync]\nallowed_origins = ['https://publisher.example']\n[aliases]\n'{}' = '{}'\n[accounts.publisher]\n",
            role.family(),
            role.name()
        ));
        let preflight = app.dispatch(
            Method::OPTIONS,
            "/cookie_sync",
            &[
                ("origin", PUBLISHER_ORIGIN),
                ("access-control-request-method", "POST"),
                ("access-control-request-headers", "content-type"),
            ],
            Body::empty(),
        );
        assert_eq!(preflight.status(), StatusCode::NO_CONTENT);
        assert!(body(&preflight).is_empty());
        assert_browser_headers(&preflight, true);
        assert_eq!(
            preflight
                .headers()
                .get("access-control-allow-methods")
                .unwrap(),
            "POST"
        );
        assert!(preflight.headers().get("set-cookie").is_none());

        let post = app.dispatch(
            Method::POST,
            "/cookie_sync",
            &[
                ("origin", PUBLISHER_ORIGIN),
                ("content-type", "application/json"),
            ],
            Body::from_bytes(
                serde_json::to_vec(&json!({
                    "account": "publisher",
                    "bidders": [role.name()],
                    "gdpr": 0_i32,
                    "coopSync": false,
                }))
                .unwrap(),
            ),
        );
        assert_eq!(
            post.status(),
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(body(&post))
        );
        assert_browser_headers(&post, true);
        assert_eq!(
            post.headers().get("content-type").unwrap(),
            "application/json; charset=utf-8"
        );
        assert!(body(&post).ends_with(b"\n"));
        assert!(post.headers().get("set-cookie").is_none());
        let json: Value = serde_json::from_slice(body(&post)).unwrap();
        assert_eq!(json["status"], "no_cookie");
        let selected = json["bidder_status"].as_array().unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0]["bidder"], role.name());
        assert_eq!(selected[0]["usersync"]["type"], "redirect");
        let callback_uri = redirect_callback_uri(&selected[0], "publisher", "buyer-123");

        let setuid = app.dispatch(Method::GET, &callback_uri, &[], Body::empty());
        assert_eq!(
            setuid.status(),
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(body(&setuid))
        );
        assert_browser_headers(&setuid, false);
        assert_eq!(setuid.headers().get("content-type").unwrap(), "image/png");
        assert_eq!(
            body(&setuid),
            include_bytes!("../../auctioneer-bidder/tests/conformance/fixtures/usersync/pixel.png")
        );
        let cookie_header = carried_cookie(&setuid, role.family(), "buyer-123");
        let getuids = app.dispatch(
            Method::GET,
            "/getuids",
            &[("origin", PUBLISHER_ORIGIN), ("cookie", &cookie_header)],
            Body::empty(),
        );
        assert_eq!(getuids.status(), StatusCode::OK);
        assert_browser_headers(&getuids, true);
        assert_eq!(
            getuids.headers().get("content-type").unwrap(),
            "text/plain; charset=utf-8"
        );
        assert_buyeruid(&getuids, role.family(), "buyer-123");
        assert!(getuids.headers().get("set-cookie").is_none());
        assert_eq!(
            app.store().reads(),
            4,
            "each endpoint should read one config snapshot"
        );
    }

    #[test]
    fn emitted_redirect_callback_round_trips_without_family_alias() {
        let role = live_catalog::redirect();
        let app = TestApp::new("[accounts.publisher]\n");
        let post = app.dispatch(
            Method::POST,
            "/cookie_sync",
            &[("content-type", "application/json")],
            Body::from_bytes(
                serde_json::to_vec(&json!({
                    "account": "publisher",
                    "bidders": [role.name()],
                    "gdpr": 0_i32,
                    "coopSync": false,
                }))
                .unwrap(),
            ),
        );
        assert_eq!(post.status(), StatusCode::OK);
        let json: Value = serde_json::from_slice(body(&post)).unwrap();
        let selected = json["bidder_status"].as_array().unwrap();
        assert_eq!(selected.len(), 1);
        let callback_path = redirect_callback_uri(&selected[0], "publisher", "no-alias-user");
        assert!(
            callback_path.contains(&format!("bidder={}&", role.name())),
            "{callback_path}"
        );

        let callback = app.dispatch(Method::GET, &callback_path, &[], Body::empty());
        assert_eq!(callback.status(), StatusCode::OK);
        let cookie_header = carried_cookie(&callback, role.family(), "no-alias-user");
        let getuids = app.dispatch(
            Method::GET,
            "/getuids",
            &[("cookie", &cookie_header)],
            Body::empty(),
        );
        assert_eq!(getuids.status(), StatusCode::OK);
        assert_buyeruid(&getuids, role.family(), "no-alias-user");
        assert_eq!(app.store().reads(), 3);
    }

    #[test]
    fn deleting_signed_cookie_expires_matching_scope_before_empty_readback() {
        let role = live_catalog::redirect();
        let app = TestApp::new("[host_cookie]\ndomain = 'sync.example.com'\n");
        let issued = app.dispatch(
            Method::GET,
            &format!("/setuid?bidder={}&uid=delete-me&gdpr=0&f=b", role.name()),
            &[],
            Body::empty(),
        );
        assert_eq!(issued.status(), StatusCode::OK);
        let carried = carried_cookie(&issued, role.family(), "delete-me");
        let original = Cookie::parse(issued.headers()["set-cookie"].to_str().unwrap()).unwrap();
        let cleared = app.dispatch(
            Method::GET,
            &format!("/setuid?bidder={}&uid=&gdpr=0&f=b", role.name()),
            &[("cookie", &carried)],
            Body::empty(),
        );
        assert_eq!(cleared.status(), StatusCode::OK);
        let deletion = Cookie::parse(cleared.headers()["set-cookie"].to_str().unwrap()).unwrap();
        assert_eq!(deletion.name(), original.name());
        assert_eq!(deletion.path(), original.path());
        assert_eq!(deletion.domain(), original.domain());
        assert_eq!(deletion.domain(), Some("sync.example.com"));
        assert!(deletion.value().is_empty());
        assert_eq!(deletion.max_age(), Some(time::Duration::ZERO));
        assert!(deletion.expires_datetime().unwrap() < TEST_NOW);
        assert_eq!(cleared.headers()["cache-control"], "no-store");
        // A browser removes the cookie on Max-Age=0; it does not echo uids=.
        let readback = app.dispatch(Method::GET, "/getuids", &[], Body::empty());
        assert_eq!(readback.status(), StatusCode::OK);
        assert_eq!(body(&readback), b"{}\n");
        assert!(!readback.headers().contains_key("set-cookie"));
    }

    #[test]
    fn iframe_append_callback_round_trip_and_unsyncable_identity_is_omitted() {
        let role = live_catalog::iframe();
        let app = TestApp::new("");
        let post = app.dispatch(
            Method::POST,
            "/cookie_sync",
            &[("content-type", "application/json")],
            Body::from_bytes(
                serde_json::to_vec(&json!({
                    "bidders": [role.name(), live_catalog::unsyncable().name],
                    "gdpr": 0_i32,
                    "coopSync": false,
                }))
                .unwrap(),
            ),
        );
        assert_eq!(post.status(), StatusCode::OK);
        let json: Value = serde_json::from_slice(body(&post)).unwrap();
        let selected = json["bidder_status"].as_array().unwrap();
        assert_eq!(
            selected.len(),
            1,
            "unsyncable identity must not produce a sync URL"
        );
        assert_eq!(selected[0]["bidder"], role.name());
        assert_eq!(selected[0]["usersync"]["type"], "iframe");
        let setuid_path_query = role.callback_uri(
            selected[0]["usersync"]["url"].as_str().unwrap(),
            "buyer-456",
        );

        let setuid = app.dispatch(Method::GET, &setuid_path_query, &[], Body::empty());
        assert_eq!(
            setuid.status(),
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(body(&setuid))
        );
        assert_eq!(setuid.headers().get("content-type").unwrap(), "text/html");
        assert!(body(&setuid).is_empty());
        let carried = carried_cookie(&setuid, role.family(), "buyer-456");
        let getuids = app.dispatch(
            Method::GET,
            "/getuids",
            &[("cookie", &carried)],
            Body::empty(),
        );
        assert_eq!(getuids.status(), StatusCode::OK);
        assert_buyeruid(&getuids, role.family(), "buyer-456");
        assert!(getuids.headers().get("set-cookie").is_none());
    }

    #[test]
    fn default_unconfigured_and_literal_unknown_accounts_keep_distinct_provenance() {
        let role = live_catalog::iframe();
        let app = TestApp::new("[accounts.unknown]\ndisabled = true\n");
        let default = app.dispatch(
            Method::GET,
            &format!("/setuid?bidder={}&uid=default-user&gdpr=0&f=b", role.name()),
            &[],
            Body::empty(),
        );
        assert_eq!(default.status(), StatusCode::OK);
        assert!(default.headers().get("set-cookie").is_some());

        let unknown = app.dispatch(
            Method::GET,
            &format!(
                "/setuid?bidder={}&uid=unknown-user&gdpr=0&f=b&account=unknown",
                role.name()
            ),
            &[],
            Body::empty(),
        );
        assert_eq!(unknown.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            body(&unknown),
            b"account is disabled, please reach out to the prebid server host"
        );
        assert!(unknown.headers().get("set-cookie").is_none());

        let unconfigured = app.dispatch(
            Method::POST,
            "/cookie_sync",
            &[("content-type", "application/json")],
            Body::from_bytes(
                serde_json::to_vec(&json!({
                    "account": "unconfigured",
                    "bidders": [role.name()],
                    "gdpr": 0_i32,
                    "coopSync": false,
                }))
                .unwrap(),
            ),
        );
        assert_eq!(unconfigured.status(), StatusCode::OK);
        let json: Value = serde_json::from_slice(body(&unconfigured)).unwrap();
        let callback_url = role.callback(
            json["bidder_status"][0]["usersync"]["url"]
                .as_str()
                .unwrap(),
        );
        assert_eq!(
            callback_url
                .query_pairs()
                .find(|(key, _)| key == "account")
                .unwrap()
                .1,
            "unconfigured"
        );
    }
}
