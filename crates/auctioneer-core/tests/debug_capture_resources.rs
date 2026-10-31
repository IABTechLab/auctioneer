//! Generated-App cancellation and terminal-egress ownership with active debug.

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
    use auctioneer_core::{
        handlers::{TestFanoutHandle, auction::TestAuctionBudget},
        host_primitives::{
            TestMonotonicClockHandle, admission_gate, boot_gates, egress_gate, static_egress_gate,
        },
    };
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
    };
    use auctioneer_runtime::{AuctionBudget, Clock as _, FakeClock};
    use edgezero_core::response_egress_framing::ResponseEgressFramingMode;
    use edgezero_core::{
        body::Body,
        http::{Method, Request, StatusCode},
        response_egress::{ResponseEgressEnvelope, ResponseEgressOutcome},
        time::MonotonicClock,
    };
    use futures::{executor::block_on, task::noop_waker};
    use serde_json::{Value, json};
    use std::{
        future::{Future as _, pending},
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        task::{Context, Poll},
        time::Duration,
    };

    const SECRET: &str = "USER_PRIVATE_CANARY";

    fn config() -> String {
        format!(
            "[aliases]\nfake_bb={:?}\n[gdpr]\nenabled=false\n[accounts.publisher]\ndebug_allow=true\n",
            serving_catalog::passthrough_info().name,
        )
    }

    enum Behavior {
        Parked,
        Empty,
        Malformed,
        Fail,
    }

    struct Controlled {
        entered: AtomicUsize,
        behavior: Behavior,
    }

    #[async_trait::async_trait(?Send)]
    impl BidderFanout for Controlled {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            assert_eq!(calls.len(), 1);
            let outbound: Value = serde_json::from_slice(calls[0].request.body()).unwrap();
            assert_eq!(outbound["user"]["id"], SECRET);
            self.entered.fetch_add(1, Ordering::SeqCst);
            match self.behavior {
                Behavior::Parked => pending().await,
                Behavior::Fail => Err(FanoutInternalError::NoOutboundCapability),
                Behavior::Malformed => Ok(vec![BidderCallResult {
                    result: Ok(BidderHttpResponse::from_transport(
                        200,
                        vec![("content-type".into(), "application/json".into())],
                        serde_json::to_vec(&json!({
                            "id":"PARSE_PRIVATE_CANARY","cur":"USD",
                            "seatbid":[{"seat":"fake_bb","bid":[{
                                "id":"bid","impid":"i","price":"PARSE_PRIVATE_CANARY"
                            }]}]
                        }))
                        .unwrap(),
                        DebugBodyEligibility::SingleTextValue,
                    )),
                    elapsed: Duration::from_millis(1),
                }]),
                Behavior::Empty => Ok(vec![BidderCallResult {
                    result: Ok(BidderHttpResponse::from_transport(
                        204,
                        vec![("content-type".into(), "application/json".into())],
                        Vec::new(),
                        DebugBodyEligibility::SingleTextValue,
                    )),
                    elapsed: Duration::from_micros(17_345),
                }]),
            }
        }
    }

    fn body() -> Body {
        Body::from_bytes(serde_json::to_vec(&json!({
            "id":"r","regs":{"gdpr":0_i32},
            "user":{"id":SECRET,"buyeruid":"BUYER_PRIVATE_CANARY"},
            "device":{"ifa":"DEVICE_PRIVATE_CANARY","ua":"UA_PRIVATE_CANARY"},
            "site":{"publisher":{"id":"publisher"},"page":"https://publisher.example/private"},
            "imp":[{"id":"i","banner":{"w":300_i32,"h":250_i32},"ext":{"bidder":{"fake_bb":{}}}}]
        })).unwrap())
    }

    fn configure(request: &mut Request, ledger: Arc<AuctionBudget>, fanout: Arc<Controlled>) {
        request.extensions_mut().insert(TestFanoutHandle(fanout));
        request.extensions_mut().insert(TestAuctionBudget(ledger));
        let clock = FakeClock::new(app::TEST_NOW);
        request
            .extensions_mut()
            .insert(TestMonotonicClockHandle(MonotonicClock::new(move || {
                clock.now_monotonic()
            })));
    }

    fn run(
        app: &app::TestApp,
        ledger: &Arc<AuctionBudget>,
        behavior: Behavior,
    ) -> ResponseEgressEnvelope {
        let fanout = Arc::new(Controlled {
            entered: AtomicUsize::new(0),
            behavior,
        });
        block_on(app.dispatch_envelope_with(
            Method::POST,
            "/openrtb2/auction?debug=1",
            &[],
            body(),
            |request| {
                configure(
                    request,
                    Arc::clone(ledger),
                    Arc::<Controlled>::clone(&fanout),
                );
            },
        ))
    }

    fn idle() {
        assert_eq!(admission_gate().unwrap().in_use(), 0);
        assert_eq!(egress_gate().unwrap().in_use(), 0);
        assert_eq!(static_egress_gate().unwrap().in_use(), 0);
    }

    #[test]
    fn cancelling_two_captured_auctions_releases_each_prepaid_peak_independently() {
        testing_logger::setup();
        let app = app::TestApp::new(&config());
        let total = boot_gates().unwrap().derived.per_auction_peak;
        let debug = boot_gates().unwrap().derived.debug_term;
        assert!(debug > 0 && total > debug);
        let ledgers = [
            Arc::new(AuctionBudget::new(total)),
            Arc::new(AuctionBudget::new(total)),
        ];
        let fanout = Arc::new(Controlled {
            entered: AtomicUsize::new(0),
            behavior: Behavior::Parked,
        });
        let mut calls = ledgers.each_ref().map(|ledger| {
            Box::pin(app.dispatch_envelope_with(
                Method::POST,
                "/openrtb2/auction?debug=1",
                &[],
                body(),
                |request| {
                    configure(
                        request,
                        Arc::clone(ledger),
                        Arc::<Controlled>::clone(&fanout),
                    );
                },
            ))
        });
        let waker = noop_waker();
        let mut context = Context::from_waker(&waker);
        for call in &mut calls {
            assert!(matches!(call.as_mut().poll(&mut context), Poll::Pending));
        }
        assert_eq!(fanout.entered.load(Ordering::SeqCst), 2);
        assert_eq!(admission_gate().unwrap().in_use(), 2);
        assert_eq!(static_egress_gate().unwrap().in_use(), 2);
        assert_eq!(egress_gate().unwrap().in_use(), 0);
        assert_eq!(
            ledgers.each_ref().map(|ledger| ledger.in_use()),
            [total, total]
        );
        let [first, second] = calls;
        drop(first);
        assert_eq!(ledgers.each_ref().map(|ledger| ledger.in_use()), [0, total]);
        assert_eq!(admission_gate().unwrap().in_use(), 1);
        drop(second);
        assert_eq!(ledgers.each_ref().map(|ledger| ledger.in_use()), [0, 0]);
        idle();
        drop(run(&app, &ledgers[0], Behavior::Empty));
        idle();
    }

    #[test]
    fn debug_assembly_releases_work_but_keeps_output_owned_until_every_terminal_outcome() {
        testing_logger::setup();
        let app = app::TestApp::new(&config());
        let ledger = Arc::new(AuctionBudget::new(
            boot_gates().unwrap().derived.per_auction_peak,
        ));
        for outcome in [
            ResponseEgressOutcome::Completed,
            ResponseEgressOutcome::ClientDisconnected,
            ResponseEgressOutcome::ConversionError,
            ResponseEgressOutcome::DeadlineExceeded,
            ResponseEgressOutcome::HostHandoff,
            ResponseEgressOutcome::RequestCancelled,
            ResponseEgressOutcome::SourceError,
            ResponseEgressOutcome::TransportError,
            ResponseEgressOutcome::Unspecified,
        ] {
            let mut envelope = run(&app, &ledger, Behavior::Empty);
            assert_eq!(envelope.response_mut().status(), StatusCode::OK);
            let response = envelope.response_mut();
            assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
            let bytes = response.body().as_bytes().unwrap();
            let parsed: Value = serde_json::from_slice(bytes).unwrap();
            let row = &parsed["ext"]["debug"]["httpcalls"]["fake_bb"][0];
            assert_eq!(row["request"]["body_status"], "captured");
            assert_eq!(row["response"]["body_status"], "empty");
            let projection: Value =
                serde_json::from_str(row["request"]["body"].as_str().unwrap()).unwrap();
            assert_eq!(projection["user"], "[REDACTED]");
            assert_eq!(projection["imp"][0]["banner"]["w"], 300_i32);
            for secret in [
                SECRET,
                "BUYER_PRIVATE_CANARY",
                "DEVICE_PRIVATE_CANARY",
                "UA_PRIVATE_CANARY",
                "/private",
            ] {
                assert!(!String::from_utf8_lossy(bytes).contains(secret));
            }
            assert_eq!(ledger.in_use(), 0);
            assert_eq!(admission_gate().unwrap().in_use(), 0);
            assert_eq!(egress_gate().unwrap().in_use(), 1);
            assert_eq!(static_egress_gate().unwrap().in_use(), 1);
            let (_prepared, _policy, mut attempt, clock) = envelope
                .begin(ResponseEgressFramingMode::ExplicitBufferedLength)
                .unwrap_or_else(|_failure| panic!("valid debug response must begin egress"));
            assert!(attempt.begin_writing());
            assert!(attempt.terminate(outcome, clock.now()));
            assert!(!attempt.terminate(outcome, clock.now()));
            idle();
        }
    }

    #[test]
    fn post_capture_handler_error_and_config_refusals_return_the_work_reservation() {
        testing_logger::setup();
        let app = app::TestApp::new(&config());
        let ledger = Arc::new(AuctionBudget::new(
            boot_gates().unwrap().derived.per_auction_peak,
        ));
        let mut failed = run(&app, &ledger, Behavior::Fail);
        assert_eq!(
            failed.response_mut().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            failed
                .response_mut()
                .headers()
                .get("cache-control")
                .unwrap(),
            "no-store"
        );
        assert_eq!(ledger.in_use(), 0);
        assert_eq!(egress_gate().unwrap().in_use(), 0);
        assert_eq!(static_egress_gate().unwrap().in_use(), 1);
        drop(failed);
        idle();
        app.store().set_unavailable();
        let refused = app.dispatch(
            Method::POST,
            "/openrtb2/auction?debug=1",
            &[],
            Body::empty(),
        );
        assert_eq!(refused.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(!String::from_utf8_lossy(app::body(&refused)).contains(SECRET));
        app.store().set_blob(Some("invalid-envelope".into()));
        let corrupt = app.dispatch(
            Method::POST,
            "/openrtb2/auction?debug=1",
            &[],
            Body::empty(),
        );
        assert_eq!(corrupt.status(), StatusCode::INTERNAL_SERVER_ERROR);
        app.store().set_config(&config());
        app.store().replace_after_next_read(&config());
        drop(run(&app, &ledger, Behavior::Empty));
        assert_eq!(app.store().reads(), 4);
        idle();
        testing_logger::validate(|entries| {
            assert!(
                entries
                    .iter()
                    .any(|entry| entry.body.contains("config store unavailable"))
            );
            for entry in entries {
                for secret in [
                    SECRET,
                    "BUYER_PRIVATE_CANARY",
                    "DEVICE_PRIVATE_CANARY",
                    app::STORE_ERROR_CANARY,
                ] {
                    assert!(!entry.body.contains(secret));
                }
            }
        });
    }

    #[test]
    fn bidder_parse_failure_cannot_reflect_secret_values_into_the_no_bid_response() {
        testing_logger::setup();
        let app = app::TestApp::new(&config());
        let ledger = Arc::new(AuctionBudget::new(
            boot_gates().unwrap().derived.per_auction_peak,
        ));
        let mut envelope = run(&app, &ledger, Behavior::Malformed);
        assert_eq!(envelope.response_mut().status(), StatusCode::OK);
        let bytes = envelope.response_mut().body().as_bytes().unwrap();
        let parsed: Value = serde_json::from_slice(bytes).unwrap();
        assert!(parsed.get("seatbid").is_none());
        let row = &parsed["ext"]["debug"]["httpcalls"]["fake_bb"][0];
        assert_eq!(row["request"]["body_status"], "captured");
        assert_eq!(row["response"]["body_status"], "omitted_type");
        assert_eq!(row["error"], "response_parse");
        assert_eq!(
            parsed["ext"]["errors"]["fake_bb"],
            json!([{
                "code":14_i32,"message":"invalid bidder response"
            }])
        );
        for secret in [
            SECRET,
            "BUYER_PRIVATE_CANARY",
            "DEVICE_PRIVATE_CANARY",
            "UA_PRIVATE_CANARY",
            "PARSE_PRIVATE_CANARY",
        ] {
            assert!(!String::from_utf8_lossy(bytes).contains(secret));
        }
        assert_eq!(ledger.in_use(), 0);
        drop(envelope);
        idle();
    }
}
