//! Extracted overlays must not retain unrelated accounts through bidder fanout.

#[cfg(test)]
use auctioneer_runtime::test_support::alloc_track;
#[expect(
    dead_code,
    reason = "the shared fixture also supports sync and store-failure tests"
)]
mod usersync_app;
use usersync_app as app;

#[cfg(test)]
use auctioneer_bidder::test_support::serving_catalog;

#[global_allocator]
static ALLOCATOR: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
mod bounded_store;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use edgezero_core::response_egress_framing::ResponseEgressFramingMode;
    #[test]
    fn passthrough_metadata_is_borrowed_and_cached_without_allocating() {
        use std::{hint::black_box, ptr};
        let info = passthrough_info();
        assert!(ptr::eq(
            info,
            auctioneer_config::bidder(&info.name).unwrap()
        ));
        let before = alloc_track::reset_peak();
        let cached = black_box(passthrough_info());
        let measured = alloc_track::peak_since(before);
        assert_eq!(measured, 0);
        assert!(ptr::eq(cached, info));
    }

    use super::{alloc_track, app};
    use crate::serving_catalog::passthrough_info;
    use auctioneer_bidder::{BidderHttpResponse, DebugBodyEligibility};
    use auctioneer_core::{
        config::AuctioneerAppConfig,
        handlers::{TestFanoutHandle, auction::TestAuctionBudget},
        host_primitives::{
            TestClockHandle, TestMonotonicClockHandle, TestRngHandle, gates::boot_gates,
        },
    };
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
    };
    use auctioneer_runtime::{AuctionBudget, Clock as _, FakeClock, SeededRng, count_json_nodes};
    use bytes::Bytes;
    use edgezero_core::{
        blob_envelope::BlobEnvelope,
        body::Body,
        http::{Method, Request, Response, StatusCode},
        time::MonotonicClock,
    };
    use futures::{future::poll_fn, stream};
    use serde_json::{Value, json};
    use std::{
        future::Future as _,
        io::{self, Write as _},
        mem::size_of,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        task::{Context, Poll, Waker},
        time::Duration,
    };

    const SELECTED: &str = r#"
[auction]
max_transport_bytes = 4096
max_decoded_bytes = 4096
[accounts.publisher]
debug_allow = true
targeting_prefix = "owned"
max_transport_bytes = 2048
max_decoded_bytes = 3072
[accounts.publisher.privacy.allowactivities.transmitUfpd]
default = true
rules = [{allow = false, condition = {componentName = ["fake_retention_alias"]}}]
"#;
    const URI: &str = "/openrtb2/auction?debug=1";

    #[derive(Default)]
    struct HeldFanout {
        entered: AtomicUsize,
        released: AtomicBool,
        body_polled: AtomicUsize,
        body_released: AtomicBool,
    }

    #[async_trait::async_trait(?Send)]
    impl BidderFanout for HeldFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].request.url(), passthrough_info().endpoint);
            assert_eq!(calls[0].max_transport_bytes, 2048);
            assert_eq!(calls[0].max_decoded_bytes, 3072);
            assert!(calls[0].request.covered_imp_ids().eq(["i"]));
            let sent: Value = serde_json::from_slice(calls[0].request.body()).unwrap();
            assert!(sent["user"].get("id").is_none());
            assert_eq!(sent["site"]["publisher"]["id"], "publisher");
            drop(sent);
            self.entered.fetch_add(1, Ordering::SeqCst);
            poll_fn(|_cx| {
                if self.released.load(Ordering::SeqCst) {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            })
            .await;
            Ok(vec![BidderCallResult {
                result: Ok(BidderHttpResponse::from_transport(
                    200,
                    Vec::new(),
                    serde_json::to_vec(&json!({
                        "id": "upstream", "cur": "USD", "seatbid": [{"seat": "fake_bidder", "bid": [{
                            "id": "bid", "impid": "i", "price": 1.0_f64,
                            "mtype": 1_u32, "adm": "<div>ad</div>"
                        }]}]
                    }))
                    .unwrap(),
                    DebugBodyEligibility::SingleTextValue,
                )),
                elapsed: Duration::ZERO,
            }])
        }
    }

    fn source(vendors: usize) -> String {
        let mut config: Value =
            serde_json::to_value(toml::from_str::<toml::Value>(SELECTED).unwrap()).unwrap();
        config["aliases"] = json!({"fake_retention_alias": passthrough_info().name});
        config["accounts"]["unselected"] = json!({
            "gdpr": {"basic_enforcement_vendors": vec!["fake_bidder"; vendors]}
        });
        toml::to_string(&config).unwrap()
    }

    fn fixture_shape(source: &str, vendors: usize) -> (usize, u32) {
        let config: AuctioneerAppConfig = toml::from_str(source).unwrap();
        config.validate_effective_defaults().unwrap();
        assert_eq!(
            config.accounts["unselected"]
                .gdpr
                .as_ref()
                .unwrap()
                .basic_enforcement_vendors
                .as_ref()
                .unwrap()
                .len(),
            vendors
        );
        let envelope = BlobEnvelope::new(
            serde_json::to_value(config).unwrap(),
            "1970-01-01T00:00:00Z".into(),
        );
        let bytes = serde_json::to_vec(&envelope).unwrap();
        let limits = &boot_gates().unwrap().limits;
        assert!(u64::try_from(bytes.len()).unwrap() <= limits.max_runtime_config_bytes);
        assert!(u64::try_from(bytes.len()).unwrap() <= limits.max_runtime_config_blob_bytes);
        // Counting keys too is stricter than the SDK's value-only ceiling.
        let nodes = count_json_nodes(&bytes, limits.max_config_json_nodes).unwrap();
        (bytes.len(), nodes)
    }

    fn body(control: Arc<HeldFanout>) -> Body {
        let bytes = serde_json::to_vec(&json!({
            "id": "retention", "tmax": 1000_u32,
            "site": {"publisher": {"id": "publisher"}},
            "user": {"id": "private"},
            "imp": [{"id": "i", "banner": {"w": 300_u32, "h": 250_u32},
                "ext": {"bidder": {"fake_retention_alias": {}}}}]
        }))
        .unwrap();
        let mut chunk = Some(Bytes::from(bytes));
        Body::stream(stream::poll_fn(move |_cx| {
            control.body_polled.fetch_add(1, Ordering::SeqCst);
            if control.body_released.load(Ordering::SeqCst) {
                Poll::Ready(chunk.take())
            } else {
                Poll::Pending
            }
        }))
    }

    fn configure(request: &mut Request, fanout: Arc<HeldFanout>, budget: Arc<AuctionBudget>) {
        let clock = Arc::new(FakeClock::new(app::TEST_NOW));
        request.extensions_mut().insert(TestFanoutHandle(fanout));
        request.extensions_mut().insert(TestAuctionBudget(budget));
        request
            .extensions_mut()
            .insert(TestClockHandle(Arc::<FakeClock>::clone(&clock)));
        request
            .extensions_mut()
            .insert(TestMonotonicClockHandle(MonotonicClock::new(move || {
                clock.now_monotonic()
            })));
        request
            .extensions_mut()
            .insert(TestRngHandle(Arc::new(SeededRng::new(42))));
    }

    fn gates(admission: u32, egress: u32, terminal: u32) {
        let installed = boot_gates().unwrap();
        assert_eq!(installed.admission.in_use(), admission);
        assert_eq!(installed.egress.in_use(), egress);
        assert_eq!(installed.static_egress.in_use(), terminal);
        assert_eq!(installed.streams.in_flight(), 0);
        assert_eq!(installed.streams.free(), installed.streams.capacity());
        assert_eq!(installed.aux.in_use(), 0);
    }

    fn parsed(response: &Response) -> Value {
        assert_eq!(response.status(), StatusCode::OK);
        let value: Value = serde_json::from_slice(app::body(response)).unwrap();
        assert_eq!(value["id"], "retention");
        assert_eq!(
            value["seatbid"].as_array().map(Vec::len),
            Some(1),
            "{value}"
        );
        assert_eq!(value["seatbid"][0]["seat"], "fake_retention_alias");
        assert_eq!(value["seatbid"][0]["bid"][0]["id"], "bid");
        assert!(
            value["seatbid"][0]["bid"][0]["ext"]["prebid"]["targeting"]
                .get("owned_pb")
                .is_some()
        );
        assert!(value["ext"]["debug"]["httpcalls"]["fake_retention_alias"].is_array());
        value
    }

    fn warm_response(app: &app::TestApp) -> Value {
        let fanout = Arc::new(HeldFanout::default());
        let budget = Arc::new(AuctionBudget::new(
            boot_gates().unwrap().derived.per_auction_peak,
        ));
        let mut dispatch = Box::pin(app.dispatch_envelope_with(
            Method::POST,
            URI,
            &[],
            body(Arc::clone(&fanout)),
            |request| configure(request, Arc::clone(&fanout), Arc::clone(&budget)),
        ));
        let mut context = Context::from_waker(Waker::noop());
        assert!(dispatch.as_mut().poll(&mut context).is_pending());
        fanout.body_released.store(true, Ordering::SeqCst);
        assert!(dispatch.as_mut().poll(&mut context).is_pending());
        fanout.released.store(true, Ordering::SeqCst);
        let Poll::Ready(mut envelope) = dispatch.as_mut().poll(&mut context) else {
            panic!("released warmup fanout must finish without another suspension");
        };
        drop(dispatch);
        let value = parsed(envelope.response_mut());
        let (prepared, _policy, mut attempt, clock) = envelope
            .begin(ResponseEgressFramingMode::ExplicitBufferedLength)
            .unwrap_or_else(|_| panic!("the warmup response must prepare for terminal egress"));
        assert!(attempt.begin_writing());
        assert!(attempt.complete(clock.now()));
        drop((prepared, attempt, clock));
        assert_eq!(app.store().reads(), 1);
        assert_eq!(fanout.entered.load(Ordering::SeqCst), 1);
        assert_eq!(budget.in_use(), 0);
        gates(0, 0, 0);
        value
    }

    struct Measurement {
        held: i64,
        config_peak: u64,
    }

    fn measure(app: &app::TestApp, vendors: usize, cancel: bool, expected: &Value) -> Measurement {
        let source = source(vendors);
        app.store().set_config(&source);
        let (blob_bytes, nodes) = fixture_shape(&source, vendors);
        let fanout = Arc::new(HeldFanout::default());
        let total = boot_gates().unwrap().derived.per_auction_peak;
        let budget = Arc::new(AuctionBudget::new(total));
        gates(0, 0, 0);
        let reads_before = app.store().reads();
        let before = alloc_track::reset_peak();
        let mut dispatch = Box::pin(app.dispatch_envelope_with(
            Method::POST,
            URI,
            &[],
            body(Arc::clone(&fanout)),
            |request| configure(request, Arc::clone(&fanout), Arc::clone(&budget)),
        ));
        let mut context = Context::from_waker(Waker::noop());
        assert!(dispatch.as_mut().poll(&mut context).is_pending());
        let config_peak = alloc_track::peak_since(before);
        assert_eq!(fanout.body_polled.load(Ordering::SeqCst), 1);
        assert_eq!(fanout.entered.load(Ordering::SeqCst), 0);
        assert_eq!(app.store().reads().checked_sub(reads_before), Some(1));
        assert_eq!(budget.in_use(), total);
        gates(1, 0, 1);
        fanout.body_released.store(true, Ordering::SeqCst);
        assert!(dispatch.as_mut().poll(&mut context).is_pending());
        let held = alloc_track::live().checked_sub(before).unwrap();
        let peak = alloc_track::peak_since(before);
        assert_eq!(fanout.entered.load(Ordering::SeqCst), 1);
        assert_eq!(app.store().reads().checked_sub(reads_before), Some(1));
        assert_eq!(budget.in_use(), total);
        gates(1, 0, 1);
        if cancel {
            drop(dispatch);
        } else {
            fanout.released.store(true, Ordering::SeqCst);
            let Poll::Ready(mut envelope) = dispatch.as_mut().poll(&mut context) else {
                panic!("released fanout must finish without another suspension");
            };
            drop(dispatch);
            assert_eq!(&parsed(envelope.response_mut()), expected);
            assert_eq!(budget.in_use(), 0);
            gates(0, 1, 1);
            let (prepared, _policy, mut attempt, clock) = envelope
                .begin(ResponseEgressFramingMode::ExplicitBufferedLength)
                .unwrap_or_else(|_| {
                    panic!("the successful auction response must prepare for terminal egress")
                });
            assert!(attempt.begin_writing());
            assert!(attempt.complete(clock.now()));
            drop((prepared, attempt, clock));
        }
        assert_eq!(app.store().reads().checked_sub(reads_before), Some(1));
        assert_eq!(budget.in_use(), 0);
        gates(0, 0, 0);
        let cleaned = alloc_track::live().checked_sub(before).unwrap();
        assert_eq!(
            cleaned, 0,
            "all dispatch-owned allocations must be released"
        );
        writeln!(
            io::stderr(),
            "vendors={vendors} cancel={cancel} blob={blob_bytes} nodes={nodes} config_peak={config_peak} held={held} peak={peak} cleaned={cleaned}"
        )
        .unwrap();
        Measurement { held, config_peak }
    }

    #[test]
    fn unselected_account_overlay_is_released_before_fanout() {
        let app = app::TestApp::new(&source(0));
        // Warm this same App before setting per-dispatch allocation baselines.
        let expected = warm_response(&app);
        let rows = [false, true]
            .map(|cancel| [0, 256, 4096].map(|vendors| measure(&app, vendors, cancel, &expected)));
        assert_eq!(app.store().reads(), 7, "one warmup plus six measured reads");
        for [empty, small, large] in rows {
            let small_storage = u64::try_from(256 * size_of::<String>()).unwrap();
            let growth_storage = u64::try_from((4096 - 256) * size_of::<String>()).unwrap();
            assert!(small.config_peak >= empty.config_peak.checked_add(small_storage).unwrap());
            assert!(large.config_peak >= small.config_peak.checked_add(growth_storage).unwrap());
            for row in [small, large] {
                assert_eq!(
                    row.held, empty.held,
                    "unselected typed overlay must not survive fanout"
                );
            }
        }
    }
}
