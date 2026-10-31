//! Mutable-store snapshot acceptance through the generated App, not file reload.

#[expect(
    dead_code,
    reason = "the shared App fixture also serves sync-only integration targets"
)]
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
    use super::app::{self, TestApp};
    use crate::serving_catalog::{ServingBidder, default_active_publications};
    use auctioneer_bidder::{BidderHttpResponse, DebugBodyEligibility};
    use auctioneer_config::types::MediaType;
    use auctioneer_core::{
        handlers::TestFanoutHandle,
        host_primitives::{TestMonotonicClockHandle, gates::boot_gates},
    };
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
    };
    use auctioneer_runtime::{Clock as _, FakeClock};
    use edgezero_core::{
        blob_envelope::BlobEnvelope,
        body::Body,
        http::{Method, Request, Response, StatusCode},
        time::MonotonicClock,
    };
    use futures::{channel::oneshot, executor::block_on};
    use serde_json::{Value, json};
    use std::{
        ptr,
        sync::{Arc, Mutex},
        time::Duration,
    };

    fn auction_bases() -> impl Iterator<Item = ServingBidder> {
        default_active_publications().into_iter().filter(|row| {
            let info = row.info;
            !info.disabled
                && info.uses_https
                && info.debug_allow
                && !info.endpoint.contains("{{")
                && info.endpoint_compression.is_none()
                && info.capabilities.site.contains(&MediaType::Banner)
        })
    }

    fn revision_a(base: &str) -> String {
        format!(
            "[aliases]\nfake_lifecycle_alias={base:?}\n[auction]\nmax_transport_bytes=4096\nmax_decoded_bytes=4096\n[accounts.publisher]\ndebug_allow=true\n"
        )
    }

    fn revision_b(base: &str) -> String {
        format!(
            "[aliases]\nfake_lifecycle_alias={base:?}\n[auction]\nmax_bidders_per_auction=100\nmax_outbound_requests=100\nmax_transport_bytes=1048576\nmax_decoded_bytes=1048576\n[accounts.publisher]\ndebug_allow=false\n"
        )
    }

    struct ControlledFanout {
        entered: Mutex<Option<oneshot::Sender<()>>>,
        release: Mutex<Option<oneshot::Receiver<()>>>,
        caps: Mutex<Vec<(String, u64, u64)>>,
    }

    #[async_trait::async_trait(?Send)]
    impl BidderFanout for ControlledFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            assert_eq!(calls.len(), 1, "the snapshot resolves exactly one alias");
            self.caps.lock().unwrap().push((
                calls[0].request.url().to_owned(),
                calls[0].max_transport_bytes,
                calls[0].max_decoded_bytes,
            ));
            let pending_release = self.release.lock().unwrap().take();
            if let Some(entered) = self.entered.lock().unwrap().take() {
                entered.send(()).unwrap();
            }
            if let Some(release) = pending_release {
                release.await.unwrap();
            }
            Ok(vec![BidderCallResult {
                result: Ok(BidderHttpResponse::from_transport(
                    204,
                    Vec::new(),
                    Vec::new(),
                    DebugBodyEligibility::SingleTextValue,
                )),
                elapsed: Duration::ZERO,
            }])
        }
    }

    fn request(params: &Value) -> Body {
        Body::from_bytes(serde_json::to_vec(&json!({"id":"snapshot", "imp":[{"id":"i","banner":{"w":300_i32,"h":250_i32},"ext":{"bidder":{"fake_lifecycle_alias":params}}}],"site":{"publisher":{"id":"publisher"}}})).unwrap())
    }

    fn configure(request: &mut Request, fanout: Arc<ControlledFanout>) {
        request.extensions_mut().insert(TestFanoutHandle(fanout));
        let clock = FakeClock::new(app::TEST_NOW);
        request
            .extensions_mut()
            .insert(TestMonotonicClockHandle(MonotonicClock::new(move || {
                clock.now_monotonic()
            })));
    }

    fn parsed(response: &Response) -> Value {
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(app::body(response))
        );
        serde_json::from_slice(app::body(response)).unwrap()
    }

    #[test]
    fn suspended_auction_retains_debug_alias_and_resource_snapshot_then_next_request_rechecks() {
        let mut bases = auction_bases();
        let base_a = bases
            .next()
            .expect("snapshot coverage requires a debug-enabled banner base");
        let base_b = bases
            .find(|base| base.info.endpoint != base_a.info.endpoint)
            .expect("snapshot coverage requires a second base with a distinct fixed endpoint");
        let revision_a = revision_a(&base_a.info.name);
        let revision_b = revision_b(&base_b.info.name);
        let app = TestApp::new(&revision_a);
        let gates = boot_gates().unwrap();
        let installed = ptr::from_ref(gates);
        let limits = gates.limits;
        let (entered_tx, entered_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let fanout = Arc::new(ControlledFanout {
            entered: Mutex::new(Some(entered_tx)),
            release: Mutex::new(Some(release_rx)),
            caps: Mutex::new(Vec::new()),
        });
        let (response, ()) = block_on(async {
            futures::join!(
                app.dispatch_with_async(
                    Method::POST,
                    "/openrtb2/auction?debug=1",
                    &[],
                    request(&base_a.params),
                    |request| configure(request, Arc::clone(&fanout))
                ),
                async {
                    entered_rx.await.unwrap();
                    assert_eq!(app.store().reads(), 1);
                    app.store().set_config(&revision_b);
                    release_tx.send(()).unwrap();
                }
            )
        });
        let first = parsed(&response);
        assert_eq!(
            first["ext"]["debug"]["httpcalls"]["fake_lifecycle_alias"][0]["request"]["endpoint_origin"],
            url::Url::parse(&base_a.info.endpoint)
                .unwrap()
                .origin()
                .ascii_serialization()
        );
        assert_eq!(app.store().reads(), 1);
        assert_eq!(
            fanout.caps.lock().unwrap().as_slice(),
            &[(base_a.info.endpoint.clone(), 4096, 4096)]
        );
        let second = parsed(&app.dispatch_with(
            Method::POST,
            "/openrtb2/auction?debug=1",
            &[],
            request(&base_b.params),
            |request| configure(request, Arc::clone(&fanout)),
        ));
        assert!(second["ext"].get("debug").is_none());
        assert_eq!(app.store().reads(), 2);
        assert_eq!(
            fanout.caps.lock().unwrap()[1],
            (
                base_b.info.endpoint.clone(),
                limits.max_transport_bytes,
                limits.max_decoded_bytes
            )
        );
        assert_eq!(ptr::from_ref(boot_gates().unwrap()), installed);
        assert_eq!(boot_gates().unwrap().limits, limits);
    }

    #[test]
    fn auction_refuses_invalid_unavailable_and_deleted_snapshots_until_valid_recovery() {
        let base = auction_bases()
            .next()
            .expect("recovery coverage requires a debug-enabled banner base");
        let revision = revision_a(&base.info.name);
        let app = TestApp::new(&revision);
        let fanout = Arc::new(ControlledFanout {
            entered: Mutex::new(None),
            release: Mutex::new(None),
            caps: Mutex::new(Vec::new()),
        });
        let dispatch = || {
            app.dispatch_with(
                Method::POST,
                "/openrtb2/auction?debug=1",
                &[],
                request(&base.params),
                |request| configure(request, Arc::clone(&fanout)),
            )
        };
        parsed(&dispatch());
        let valid =
            serde_json::to_value(BlobEnvelope::new(json!({}), "1970-01-01T00:00:00Z".into()))
                .unwrap();
        let mut corrupt = valid;
        corrupt["data"] = json!({"status":{"banner":"integrity-canary"}});
        for (index, replacement) in [
            Some("{".into()),
            Some(serde_json::to_string(&corrupt).unwrap()),
            None,
        ]
        .into_iter()
        .enumerate()
        {
            app.store().set_blob(replacement);
            let response = dispatch();
            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(app::body(&response), b"Malformed host configuration");
            assert!(response.headers().get("retry-after").is_none());
            assert_eq!(app.store().reads(), 2 + index * 2);
            app.store().set_config(&revision);
            parsed(&dispatch());
        }
        app.store().set_unavailable();
        let response = dispatch();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()["retry-after"], "5");
        assert!(!String::from_utf8_lossy(app::body(&response)).contains(app::STORE_ERROR_CANARY));
        app.store().set_config(&revision);
        parsed(&dispatch());
        assert_eq!(app.store().reads(), 9);
        assert_eq!(fanout.caps.lock().unwrap().len(), 5);
    }

    #[test]
    fn disabled_publisher_with_opaque_overflow_number_refuses_before_typed_preparation() {
        let app = TestApp::new("[accounts.publisher]\ndisabled=true\n");
        // Literal exemption: json! cannot preserve the exact out-of-range 1e400 token.
        let wire = br#"{"id":"r","site":{"publisher":{"id":"publisher"}},"imp":[{"id":"i","banner":{"ext":{"opaque":1e400}}}]}"#;
        auctioneer_runtime::count_json_nodes(wire, 100).unwrap();
        let fanout = Arc::new(ControlledFanout {
            entered: Mutex::new(None),
            release: Mutex::new(None),
            caps: Mutex::new(Vec::new()),
        });
        let response = app.dispatch_with(
            Method::POST,
            "/openrtb2/auction",
            &[],
            Body::from_bytes(wire.to_vec()),
            |request| configure(request, fanout),
        );
        assert_eq!(
            response.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "{}",
            String::from_utf8_lossy(app::body(&response))
        );
        assert!(String::from_utf8_lossy(app::body(&response)).contains("publisher"));
        assert_eq!(app.store().reads(), 1);
    }
}
