//! Competing faults at generated-App application boundaries after admission.

#[expect(
    dead_code,
    reason = "the shared App fixture also serves lifecycle-only integration targets"
)]
mod usersync_app;
use auctioneer_core::test_support::usersync_keys as keys;
use usersync_app as app;
#[cfg(test)]
mod support;

#[cfg(test)]
mod bounded_store;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use super::{
        app::{self, TestApp},
        keys, support,
    };
    use auctioneer_core::handlers::TestFanoutHandle;
    use auctioneer_core::usersync::{cookie::UidCookie, limits::CookieLimits};
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
    };
    use edgezero_core::{
        app::App,
        blob_envelope::BlobEnvelope,
        body::Body,
        error::EdgeError,
        http::{Method, Response, StatusCode, request_builder},
        ingress::{IngressDispatchOutcome, IngressFraming, IngressHeadAccounting},
        time::{MonotonicClock, MonotonicInstant},
    };
    use futures::{executor::block_on, stream};
    use serde_json::json;
    use std::{
        mem,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        task::Poll,
        time::Duration,
    };

    struct NoDispatch;

    #[async_trait::async_trait(?Send)]
    impl BidderFanout for NoDispatch {
        async fn send_all(
            &self,
            _calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            panic!("competing faults must refuse before bidder dispatch");
        }
    }

    const POLICY: &str = "[host_cookie.optout_cookie]\nname='optout'\nvalue='yes'\n[accounts.disabled]\ndisabled=true\n";

    const CPU_POLICY: &str = "[user_sync]\nallowed_origins=['https://publisher.example']\n[host_cookie.optout_cookie]\nname='optout'\nvalue='yes'\n";

    fn clocked_sync_request(
        method: Method,
        uri: &str,
        wire: &[u8],
        expire_at: usize,
        body_timeout: bool,
    ) -> (Response, usize, usize) {
        let mut generated =
            App::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default()).unwrap();
        let base = MonotonicInstant::now();
        let expired = base.checked_add(Duration::from_mins(1)).unwrap();
        let samples = Arc::new(AtomicUsize::new(0));
        let clock_samples = Arc::clone(&samples);
        let timed_out = Arc::new(AtomicBool::new(false));
        let clock_timeout = Arc::clone(&timed_out);
        generated.set_monotonic_clock(MonotonicClock::new(move || {
            let sample = clock_samples
                .fetch_add(1, Ordering::SeqCst)
                .saturating_add(1);
            if sample >= expire_at || clock_timeout.load(Ordering::SeqCst) {
                expired
            } else {
                base
            }
        }));
        let polls = Arc::new(AtomicUsize::new(0));
        let body_polls = Arc::clone(&polls);
        let mut chunk = Some(bytes::Bytes::copy_from_slice(wire));
        let body = Body::from_stream(stream::poll_fn(move |_cx| {
            body_polls.fetch_add(1, Ordering::SeqCst);
            if body_timeout {
                timed_out.store(true, Ordering::SeqCst);
                return Poll::Ready(Some(Err(EdgeError::RequestTimeout {
                    message: "synthetic-ingress-timeout".into(),
                })));
            }
            Poll::Ready(chunk.take().map(Ok))
        }));
        let request = request_builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .header("cookie", "optout=yes")
            .header("origin", "https://publisher.example")
            .header("access-control-request-method", "POST")
            .body(body)
            .unwrap();
        let configured_request = support::ctx_with_config(CPU_POLICY, request)
            .into_request()
            .unwrap();
        let outcome = block_on(generated.dispatch_ingress(
            configured_request,
            base,
            IngressHeadAccounting::HostManaged,
            IngressFraming::HostManaged,
        ))
        .unwrap();
        let IngressDispatchOutcome::Response(mut envelope) = outcome else {
            panic!("admitted sync request must produce a response");
        };
        (
            mem::replace(envelope.response_mut(), Response::new(Body::empty())),
            samples.load(Ordering::SeqCst),
            polls.load(Ordering::SeqCst),
        )
    }

    #[test]
    fn every_sync_cpu_checkpoint_can_replace_business_response_without_retry_after() {
        let _serial = TestApp::new(CPU_POLICY);
        for (method, uri, wire, expected_status, expected_checkpoints) in [
            (
                Method::POST,
                "/cookie_sync",
                serde_json::to_vec(&json!({"gdpr":0_i32})).unwrap(),
                StatusCode::UNAUTHORIZED,
                4,
            ),
            (
                Method::POST,
                "/cookie_sync",
                // Literal exemption: the parse checkpoint must receive incomplete JSON.
                b"{".to_vec(),
                StatusCode::BAD_REQUEST,
                4,
            ),
            (
                Method::OPTIONS,
                "/cookie_sync",
                Vec::new(),
                StatusCode::NO_CONTENT,
                3,
            ),
            (
                Method::GET,
                "/setuid?bidder=fake_fixture_bidder&uid=u&gdpr=0",
                Vec::new(),
                StatusCode::UNAUTHORIZED,
                3,
            ),
            (
                Method::GET,
                "/getuids?x=%GG",
                Vec::new(),
                StatusCode::BAD_REQUEST,
                3,
            ),
        ] {
            let (baseline, sample_count, _) =
                clocked_sync_request(method.clone(), uri, &wire, usize::MAX, false);
            assert_eq!(baseline.status(), expected_status);
            let mut cpu_samples = Vec::new();
            // Sample one anchors the config-fetch deadline. Expiring there
            // moves that anchor, rather than targeting a route checkpoint.
            for sample in 2..=sample_count {
                let (response, _, polls) =
                    clocked_sync_request(method.clone(), uri, &wire, sample, false);
                if response.status() == StatusCode::SERVICE_UNAVAILABLE
                    && app::body(&response) == b"overloaded\n"
                {
                    assert!(response.headers().get("retry-after").is_none());
                    assert!(response.headers().get("set-cookie").is_none());
                    assert_eq!(
                        response.headers()["access-control-allow-origin"],
                        "https://publisher.example"
                    );
                    cpu_samples.push((sample, polls));
                } else if response.status() == StatusCode::SERVICE_UNAVAILABLE {
                    assert_eq!(response.headers()["retry-after"], "5");
                    assert_eq!(
                        app::body(&response),
                        b"Config store unavailable, retry after 5s"
                    );
                } else {
                    assert!(
                        response.status() == expected_status
                            || response.status() == StatusCode::REQUEST_TIMEOUT
                    );
                }
            }
            assert_eq!(
                cpu_samples.len(),
                expected_checkpoints,
                "{method} {uri}: {cpu_samples:?}"
            );
            assert_eq!(
                cpu_samples[0].1, 0,
                "host-resolution refusal must not poll input"
            );
            assert!(
                cpu_samples.last().unwrap().1 > 0,
                "post-decision checkpoint must replace the baseline response"
            );
        }
    }

    #[test]
    fn admitted_body_timeout_stays_408_when_route_cpu_expires_during_read() {
        let _serial = TestApp::new(CPU_POLICY);
        for (method, uri) in [
            (Method::POST, "/cookie_sync"),
            (
                Method::GET,
                "/setuid?bidder=fake_fixture_bidder&uid=u&gdpr=0",
            ),
            (Method::GET, "/getuids"),
        ] {
            let (response, _, polls) = clocked_sync_request(method, uri, b"", usize::MAX, true);
            assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
            assert_eq!(app::body(&response), b"request timeout\n");
            assert!(response.headers().get("retry-after").is_none());
            assert!(response.headers().get("set-cookie").is_none());
            assert!(polls > 0);
        }
    }

    #[test]
    fn auction_syntax_nodes_and_disabled_account_precede_typed_field_validation() {
        let app =
            TestApp::new("[auction]\nmax_parsed_nodes=20\n[accounts.disabled]\ndisabled=true\n");
        let cases = [
            // Literal exemption: preserve the incomplete imp array's syntax fault.
            (br#"{"site":{"publisher":{"id":"disabled"}},"imp":["#.to_vec(), StatusCode::BAD_REQUEST, "Invalid request"),
            (serde_json::to_vec(&json!({"site":{"publisher":{"id":"disabled"}},"extra":vec![0_i32;14],"regs":{"gpp_sid":"invalid"}})).unwrap(), StatusCode::BAD_REQUEST, "nodes"),
            (serde_json::to_vec(&json!({"site":{"publisher":{"id":"disabled"}}})).unwrap(), StatusCode::SERVICE_UNAVAILABLE, "disabled"),
            (serde_json::to_vec(&json!({"site":{"publisher":{"id":"disabled"}},"ext":{"prebid":{"channel":{"name":"invalid"}}}})).unwrap(), StatusCode::SERVICE_UNAVAILABLE, "disabled"),
            (serde_json::to_vec(&json!({"id":"r","site":{"publisher":{"id":"disabled"}},"regs":{"gpp_sid":"invalid"}})).unwrap(), StatusCode::SERVICE_UNAVAILABLE, "disabled"),
            // Literal exemption: account refusal must precede conversion of opaque 1e400.
            (br#"{"site":{"publisher":{"id":"disabled"}},"imp":[{"id":"i","banner":{"ext":{"opaque":1e400}}}]}"#.to_vec(), StatusCode::SERVICE_UNAVAILABLE, "disabled"),
            (serde_json::to_vec(&json!({"id":"r","site":{"publisher":{"id":"clean"}}})).unwrap(), StatusCode::BAD_REQUEST, "imp"),
        ];
        for (wire, status, message) in &cases {
            let response = app.dispatch_with(
                Method::POST,
                "/openrtb2/auction",
                &[],
                Body::from_bytes(wire.clone()),
                |request| {
                    request
                        .extensions_mut()
                        .insert(TestFanoutHandle(Arc::new(NoDispatch)));
                },
            );
            assert_eq!(
                response.status(),
                *status,
                "{}",
                String::from_utf8_lossy(app::body(&response))
            );
            assert!(
                String::from_utf8_lossy(app::body(&response)).contains(message),
                "{}",
                String::from_utf8_lossy(app::body(&response))
            );
        }
        assert_eq!(app.store().reads(), cases.len());
    }

    #[test]
    fn auction_byte_and_depth_bounds_refuse_before_disabled_account_discovery() {
        let app = TestApp::new("[accounts.disabled]\ndisabled=true\n");
        let too_deep = format!(
            "{{\"site\":{{\"publisher\":{{\"id\":\"disabled\"}}}},\"opaque\":{}0{}}}",
            "[".repeat(128),
            "]".repeat(128)
        );
        let too_large = serde_json::to_vec(
            &json!({"site":{"publisher":{"id":"disabled"}},"opaque":"x".repeat(0x0004_0000)}),
        )
        .unwrap();
        for wire in [too_deep.into_bytes(), too_large] {
            let response = app.dispatch_with(
                Method::POST,
                "/openrtb2/auction",
                &[],
                Body::from_bytes(wire),
                |request| {
                    request
                        .extensions_mut()
                        .insert(TestFanoutHandle(Arc::new(NoDispatch)));
                },
            );
            assert_eq!(
                response.status(),
                StatusCode::BAD_REQUEST,
                "{}",
                String::from_utf8_lossy(app::body(&response))
            );
        }
        assert_eq!(app.store().reads(), 2);
    }

    #[test]
    fn unrelated_invalid_account_in_blob_precedes_body_disabled_account_and_optout() {
        let app = TestApp::new(POLICY);
        let invalid = BlobEnvelope::new(
            json!({"accounts":{"unrelated":{"targeting_prefix":""}, "disabled":{"disabled":true}}}),
            "1970-01-01T00:00:00Z".into(),
        );
        app.store()
            .set_blob(Some(serde_json::to_string(&invalid).unwrap()));
        for (uri, wire) in [
            (
                "/openrtb2/auction",
                // Literal exemption: malformed request syntax competes with bad config.
                br#"{"site":{"publisher":{"id":"disabled"}},"imp":["#.to_vec(),
            ),
            (
                "/cookie_sync",
                serde_json::to_vec(&json!({"account":"disabled","gdpr":1_i32})).unwrap(),
            ),
        ] {
            let response = app.dispatch(
                Method::POST,
                uri,
                &[
                    ("cookie", "optout=yes"),
                    ("content-type", "application/json"),
                ],
                Body::from_bytes(wire),
            );
            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(app::body(&response), b"Malformed host configuration");
            assert!(response.headers().get("retry-after").is_none());
        }
        assert_eq!(app.store().reads(), 2);
        assert_eq!(app.secrets().reads(), 0);
    }

    #[test]
    fn host_marker_and_disabled_account_refuse_before_keyring_but_uid_optout_requires_authentication()
     {
        let app = TestApp::new(POLICY);
        let uid = keys::encode_cookie(&UidCookie::opted_out(), &CookieLimits::default()).unwrap();
        let signed = format!("uids={uid}");
        app.secrets().unavailable();
        for (cookie, account, status, expected_reads, message) in [
            (
                "optout=yes",
                "disabled",
                StatusCode::UNAUTHORIZED,
                0,
                "User has opted out",
            ),
            (
                signed.as_str(),
                "disabled",
                StatusCode::BAD_REQUEST,
                0,
                "account is disabled",
            ),
            (
                signed.as_str(),
                "allowed",
                StatusCode::SERVICE_UNAVAILABLE,
                1,
                "user sync signing unavailable",
            ),
            (
                "",
                "disabled",
                StatusCode::BAD_REQUEST,
                1,
                "account is disabled",
            ),
        ] {
            let wire = serde_json::to_vec(&json!({"account":account,"gdpr":1_i32,"filterSettings":{"iframe":{"filter":"invalid","bidders":["fake_fixture_bidder"]}}})).unwrap();
            let response = app.dispatch(
                Method::POST,
                "/cookie_sync",
                &[("content-type", "application/json"), ("cookie", cookie)],
                Body::from_bytes(wire),
            );
            assert_eq!(response.status(), status);
            assert!(String::from_utf8_lossy(app::body(&response)).contains(message));
            assert!(response.headers().get("retry-after").is_none());
            assert!(response.headers().get("set-cookie").is_none());
            assert_eq!(app.secrets().reads(), expected_reads);
        }
        app.secrets().set(Some(keys::SECRET_JSON.as_bytes()));
        for (index, wire) in [
            json!({"account":"allowed","gdpr":1_i32}),
            json!({"account":"allowed","gdpr":0_i32,"filterSettings":{"iframe":{"filter":"invalid","bidders":["fake_fixture_bidder"]}}}),
        ].into_iter().enumerate() {
            let response = app.dispatch(Method::POST, "/cookie_sync", &[("content-type","application/json"),("cookie",&signed)], Body::from_bytes(serde_json::to_vec(&wire).unwrap()));
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            assert_eq!(app::body(&response), b"User has opted out\n");
            assert_eq!(app.secrets().reads(), 2 + index);
        }
        assert_eq!(app.store().reads(), 6);
    }
}
