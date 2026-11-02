//! Debug signals and cache policy through the generated application's real route.

mod usersync_app;
use usersync_app as app;

#[cfg(test)]
use auctioneer_bidder::test_support::serving_catalog;

#[cfg(test)]
mod bounded_store;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use super::{app, serving_catalog};
    use auctioneer_bidder::{BidderHttpResponse, DebugBodyEligibility};
    use auctioneer_core::handlers::{TestFanoutHandle, auction::TestAuctionBudget};
    use auctioneer_core::host_primitives::{TestMonotonicClockHandle, TestRngHandle};
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
    };
    use auctioneer_runtime::{AuctionBudget, Clock as _, FakeClock, SeededRng};
    use edgezero_core::time::MonotonicClock;
    use edgezero_core::{
        body::Body,
        http::{Method, Response, StatusCode},
    };
    use serde_json::{Value, json};
    use std::{
        env,
        process::Command,
        sync::{Arc, Mutex},
        time::Duration,
    };

    struct NoCalls;

    fn config(overrides: &str) -> String {
        format!(
            "[aliases]\nfake_bb={:?}\n{overrides}",
            serving_catalog::passthrough_info().name,
        )
    }

    struct BiddingFanout {
        sent: Mutex<Vec<Vec<u8>>>,
    }

    #[async_trait::async_trait(?Send)]
    impl BidderFanout for BiddingFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            assert_eq!(calls.len(), 1);
            self.sent
                .lock()
                .unwrap()
                .push(calls[0].request.body().to_vec());
            Ok(vec![BidderCallResult {
                result: Ok(BidderHttpResponse::from_transport(
                    200,
                    vec![("content-type".into(), "application/json".into())],
                    serde_json::to_vec(&json!({
                        "id":"response-secret", "cur":"USD",
                        "seatbid":[{"seat":"fake_bb","bid":[{
                            "id":"bid-secret","impid":"i","price":1.5_f64,"mtype":1_i32,
                            "adm":"<div>creative</div>", "w":300_i32,"h":250_i32
                        }]}]
                    }))
                    .unwrap(),
                    DebugBodyEligibility::SingleTextValue,
                )),
                elapsed: Duration::from_micros(17_345),
            }])
        }
    }

    #[test]
    fn actual_app_capture_preserves_the_winner_targeting_and_outbound_bytes() {
        let app = app::TestApp::new(&config("[accounts.publisher]\ndebug_allow=true\n"));
        let fanout = Arc::new(BiddingFanout {
            sent: Mutex::new(Vec::new()),
        });
        let mut ordinary = None;
        for debug in [false, true] {
            let mut body = request();
            body["user"] = json!({"id":"user-secret"});
            let response =
                app.dispatch_with(
                    Method::POST,
                    if debug {
                        "/openrtb2/auction?debug=1"
                    } else {
                        "/openrtb2/auction"
                    },
                    &[],
                    Body::from_bytes(serde_json::to_vec(&body).unwrap()),
                    |request| {
                        request
                            .extensions_mut()
                            .insert(TestFanoutHandle(Arc::<BiddingFanout>::clone(&fanout)));
                        request
                            .extensions_mut()
                            .insert(TestRngHandle(Arc::new(SeededRng::new(42))));
                        let clock = FakeClock::new(app::TEST_NOW);
                        request.extensions_mut().insert(TestMonotonicClockHandle(
                            MonotonicClock::new(move || clock.now_monotonic()),
                        ));
                    },
                );
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "{}",
                String::from_utf8_lossy(app::body(&response))
            );
            let mut parsed: Value = serde_json::from_slice(app::body(&response)).unwrap();
            assert_eq!(parsed["seatbid"][0]["bid"][0]["price"], 1.5_f64);
            assert_eq!(
                parsed["seatbid"][0]["bid"][0]["ext"]["prebid"]["targeting"]["hb_pb"],
                "1.50"
            );
            if debug {
                assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
                let captured = parsed["ext"]
                    .as_object_mut()
                    .unwrap()
                    .remove("debug")
                    .unwrap();
                let row = &captured["httpcalls"]["fake_bb"][0];
                assert_eq!(row["elapsed_ms"], 17_i32);
                assert_eq!(row["request"]["body_status"], "captured");
                assert_eq!(row["response"]["body_status"], "captured");
                let request_body: Value =
                    serde_json::from_str(row["request"]["body"].as_str().unwrap()).unwrap();
                assert_eq!(request_body["user"], "[REDACTED]");
                let response_body: Value =
                    serde_json::from_str(row["response"]["body"].as_str().unwrap()).unwrap();
                assert_eq!(response_body["seatbid"][0]["bid"][0]["price"], 1.5_f64);
                for secret in [
                    "user-secret",
                    "response-secret",
                    "bid-secret",
                    "<div>creative</div>",
                ] {
                    assert!(!captured.to_string().contains(secret));
                }
                assert_eq!(Some(&parsed), ordinary.as_ref());
            } else {
                ordinary = Some(parsed);
            }
        }
        let sent = fanout.sent.lock().unwrap();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0], sent[1]);
    }

    #[async_trait::async_trait(?Send)]
    impl BidderFanout for NoCalls {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            assert!(
                calls.is_empty(),
                "empty-auction fixture cannot emit a bidder call"
            );
            Ok(Vec::new())
        }
    }

    fn dispatch(app: &app::TestApp, uri: &str, headers: &[(&str, &str)], body: &Value) -> Response {
        app.dispatch_with(
            Method::POST,
            uri,
            headers,
            Body::from_bytes(serde_json::to_vec(body).unwrap()),
            |request| {
                request
                    .extensions_mut()
                    .insert(TestFanoutHandle(Arc::new(NoCalls)));
                let clock = FakeClock::new(app::TEST_NOW);
                request
                    .extensions_mut()
                    .insert(TestMonotonicClockHandle(MonotonicClock::new(move || {
                        clock.now_monotonic()
                    })));
            },
        )
    }

    #[test]
    fn actual_app_drops_debug_atomically_at_the_final_response_boundary() {
        const CHILD_CAP: &str = "AUCTIONEER__TEST__FINAL_RESPONSE_CAP";
        const CHILD_BASE: &str = "AUCTIONEER__TEST__BASE_RESPONSE_BYTES";
        let config =
            config("[bidders.fake_bb]\nsample_rate=0.0\n[accounts.publisher]\ndebug_allow=true\n");
        if let Ok(raw) = env::var(CHILD_CAP) {
            let cap: u64 = raw.parse().unwrap();
            let base: usize = env::var(CHILD_BASE).unwrap().parse().unwrap();
            let app = app::TestApp::with_final_response_cap(&config, cap);
            let response = dispatch(&app, "/openrtb2/auction?debug=1", &[], &request());
            assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
            if cap < u64::try_from(base).unwrap() {
                assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
                assert!(!app::body(&response).starts_with(b"{"));
            } else {
                assert_eq!(response.status(), StatusCode::OK);
                assert_eq!(app::body(&response).len(), base);
                let parsed: Value = serde_json::from_slice(app::body(&response)).unwrap();
                assert!(parsed["ext"].get("debug").is_none());
                assert_eq!(parsed["id"], "r");
            }
            return;
        }
        let app = app::TestApp::new(&config);
        let response = dispatch(&app, "/openrtb2/auction?debug=1", &[], &request());
        assert_eq!(response.status(), StatusCode::OK);
        let full = app::body(&response).len();
        let mut base: Value = serde_json::from_slice(app::body(&response)).unwrap();
        assert!(
            base["ext"]
                .as_object_mut()
                .unwrap()
                .remove("debug")
                .is_some()
        );
        let base_bytes = serde_json::to_vec(&base).unwrap().len();
        assert!(full > base_bytes + 1);
        for cap in [base_bytes - 1, base_bytes, full - 1] {
            let output = Command::new(env::current_exe().unwrap())
                .args([
                    "--exact",
                    "tests::actual_app_drops_debug_atomically_at_the_final_response_boundary",
                    "--nocapture",
                ])
                .env(CHILD_CAP, cap.to_string())
                .env(CHILD_BASE, base_bytes.to_string())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    fn request() -> Value {
        json!({"id":"r","imp":[{"id":"i","banner":{"w":300_i32,"h":250_i32},"ext":{"bidder":{"fake_bb":{}}}}],"site":{"publisher":{"id":"publisher"}}})
    }

    #[test]
    fn prepayment_refusals_are_uncacheable_without_resolving_configuration() {
        let app = app::TestApp::new("");
        for uri in ["/openrtb2/auction", "/openrtb2/auction?%64ebug=%31"] {
            let response = app.dispatch_with(
                Method::POST,
                uri,
                &[],
                Body::from_bytes(b"not-json".to_vec()),
                |request| {
                    request
                        .extensions_mut()
                        .insert(TestAuctionBudget(Arc::new(AuctionBudget::new(0))));
                },
            );
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
            assert_eq!(app.store().reads(), 0);
            assert_eq!(app::body(&response), b"overloaded\n");
        }
    }

    #[test]
    fn all_recognized_signal_sources_keep_empty_authorized_debug_uncacheable() {
        let app = app::TestApp::new(&config(
            "[bidders.fake_bb]\nsample_rate=0.0\n[accounts.publisher]\ndebug_allow = true\n",
        ));
        for signal in 0_u8..4 {
            let mut body = request();
            let (uri, headers) = match signal {
                0 => ("/openrtb2/auction?debug=1", Vec::new()),
                1 => ("/openrtb2/auction", vec![("X-Auctioneer-Debug", " \t1\t ")]),
                2 => {
                    body["test"] = json!(1_i32);
                    ("/openrtb2/auction", Vec::new())
                }
                _ => {
                    body["ext"] = json!({"prebid":{"debug":true}});
                    ("/openrtb2/auction", Vec::new())
                }
            };
            let response = dispatch(&app, uri, &headers, &body);
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "{}",
                String::from_utf8_lossy(app::body(&response))
            );
            assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
            let parsed: Value = serde_json::from_slice(app::body(&response)).unwrap();
            assert_eq!(
                parsed["ext"]["debug"],
                json!({"httpcalls":{},"truncated":false,"dropped_calls":0_i32})
            );
        }
    }

    #[test]
    fn account_denial_keeps_its_warning_and_no_store_but_never_debug() {
        let app = app::TestApp::new(&config(
            "[bidders.fake_bb]\nsample_rate=0.0\n[accounts.publisher]\ndebug_allow = false\n",
        ));
        let response = dispatch(&app, "/openrtb2/auction?debug=1", &[], &request());
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        let parsed: Value = serde_json::from_slice(app::body(&response)).unwrap();
        assert!(parsed["ext"].get("debug").is_none());
        assert_eq!(
            parsed["ext"]["warnings"]["prebid"],
            json!([{"code":10002_i32,"message":"debug disabled for account"}])
        );
    }

    #[test]
    fn recognized_http_debug_survives_outer_configuration_errors() {
        let app = app::TestApp::new("");
        app.store().set_unavailable();
        let outage_response = dispatch(&app, "/openrtb2/auction?debug=1", &[], &request());
        assert_eq!(outage_response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            outage_response.headers().get("cache-control").unwrap(),
            "no-store"
        );
        assert_eq!(app.store().reads(), 1);

        app.store().set_blob(Some("not-an-envelope".into()));
        let invalid_response = app.dispatch(
            Method::POST,
            "/openrtb2/auction?debug=1",
            &[],
            Body::empty(),
        );
        assert_eq!(invalid_response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            invalid_response.headers().get("cache-control").unwrap(),
            "no-store"
        );
        app.store().set_config(&config(
            "[bidders.fake_bb]\nsample_rate=0.0\n[accounts.publisher]\ndebug_allow=true\n",
        ));
        app.store().replace_after_next_read(&config(
            "[bidders.fake_bb]\nsample_rate=0.0\n[accounts.publisher]\ndebug_allow=false\n",
        ));
        for allowed in [true, false] {
            let response = dispatch(&app, "/openrtb2/auction?debug=1", &[], &request());
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
            let parsed: Value = serde_json::from_slice(app::body(&response)).unwrap();
            assert_eq!(parsed["ext"].get("debug").is_some(), allowed);
        }
        assert_eq!(app.store().reads(), 4);
    }

    #[test]
    fn normalized_body_signal_survives_a_later_handler_owned_privacy_error() {
        let app = app::TestApp::new("");
        let mut body = request();
        body["test"] = json!(1_i32);
        body["regs"] = json!({"gdpr":2_i32});
        let response = dispatch(&app, "/openrtb2/auction", &[], &body);
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
    }

    #[test]
    fn unrequested_or_malformed_query_does_not_create_debug_or_cache_policy() {
        let app = app::TestApp::new(&config(
            "[bidders.fake_bb]\nsample_rate=0.0\n[accounts.publisher]\ndebug_allow = true\n",
        ));
        for uri in [
            "/openrtb2/auction",
            "/openrtb2/auction?debug=1&other=%FF",
            "/openrtb2/auction?debug=1+",
        ] {
            let response = dispatch(&app, uri, &[], &request());
            assert_eq!(response.status(), StatusCode::OK);
            assert!(response.headers().get("cache-control").is_none());
            let parsed: Value = serde_json::from_slice(app::body(&response)).unwrap();
            assert!(parsed["ext"].get("debug").is_none());
        }
    }
}
