//! Real admission with two independently charged, suspended auctions.

mod support;

#[cfg(test)]
mod fanout_limits;
use fanout_limits as envelope;

#[cfg(test)]
use auctioneer_bidder::test_support::serving_catalog;

#[cfg(test)]
mod tests {
    use super::{envelope, serving_catalog, support};

    use auctioneer_core::handlers::request_disclosure_audit::construction_costs as core_construction_costs;
    use auctioneer_core::handlers::{TestFanoutHandle, auction::TestAuctionBudget};
    use auctioneer_core::host_primitives::{
        TestClockHandle, TestMonotonicClockHandle, TestRngHandle, admission_gate, boot_gates,
        install_boot_envelope, static_egress_gate,
    };
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
    };
    use auctioneer_runtime::{
        AuctionBudget, Clock as _, FakeClock, SeededRng, TargetPlatformConstants, bounded_json,
    };
    use edgezero_core::app::App;
    use edgezero_core::body::Body;
    use edgezero_core::ingress::{IngressDispatchOutcome, IngressFraming, IngressHeadAccounting};
    use edgezero_core::time::{MonotonicClock, MonotonicInstant};
    use futures::{executor::block_on, task::noop_waker};
    use serde_json::{Value, json};
    use std::future::{Future as _, pending};
    use std::mem;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::task::{Context, Poll};

    struct Parked(Arc<AtomicUsize>);

    #[async_trait::async_trait(?Send)]
    impl BidderFanout for Parked {
        async fn send_all(
            &self,
            _: Vec<BidderCall>,
            _: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            pending().await
        }
    }

    fn request(
        body: &Value,
        ledger: &Arc<AuctionBudget>,
        entered: &Arc<AtomicUsize>,
    ) -> http::Request<Body> {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let mut request = http::Request::builder()
            .method("POST")
            .uri("/openrtb2/auction")
            .body(Body::from_bytes(serde_json::to_vec(body).unwrap()))
            .unwrap();
        request
            .extensions_mut()
            .insert(TestFanoutHandle(Arc::new(Parked(Arc::clone(entered)))));
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
        request
            .extensions_mut()
            .insert(TestAuctionBudget(Arc::clone(ledger)));
        let config = format!(
            "[aliases]\nfake_bb={:?}\n[gdpr]\nenabled = false\n",
            serving_catalog::passthrough_info().name,
        );
        support::ctx_with_config(&config, request)
            .into_request()
            .unwrap()
    }

    fn target() -> TargetPlatformConstants {
        TargetPlatformConstants {
            platform_concurrent_ceiling: 384,
            header_entry_overhead_bytes: 64,
            parser_header_ceiling_bytes: 21_632,
            parser_header_knob_bytes: 21_633,
            max_request_target_bytes: 768,
            h1_max_buf_size_bytes: 24_576_u32,
            h1_max_headers: 128,
            parser_max_header_entries: 676,
            pseudoheader_allowance_bytes: 1024,
            parser_transient_bytes: 0x4000,
            egress_handoff_factor: 2,
        }
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the ordered boot fixture keeps construction charges, independent ledger release, and amplification refusal together"
    )]
    fn independent_live_ledgers_and_amplification_refusal() {
        let _info = serving_catalog::passthrough_info();
        let limits = envelope::limits();
        install_boot_envelope(
            &limits,
            &target(),
            auctioneer_bidder::outbound_meta_bytes(100),
            core_construction_costs(),
            auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        let app =
            App::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default()).unwrap();
        let total = boot_gates().unwrap().derived.per_auction_peak;
        let entered = Arc::new(AtomicUsize::new(0));
        let body = json!({"id":"r", "tmax":1000_i32, "site":{"id":"s"}, "imp":[{
            "id":"i", "banner":{"w":300_i32,"h":250_i32}, "ext":{"bidder":{"fake_bb":{}}}
        }]});
        let ledgers = [
            Arc::new(AuctionBudget::new(total)),
            Arc::new(AuctionBudget::new(total)),
        ];
        let mut futures = ledgers.each_ref().map(|ledger| {
            Box::pin(app.dispatch_ingress(
                request(&body, ledger, &entered),
                MonotonicInstant::now(),
                IngressHeadAccounting::HostManaged,
                IngressFraming::HostManaged,
            ))
        });
        let waker = noop_waker();
        let mut context = Context::from_waker(&waker);
        for pending in &mut futures {
            assert!(matches!(pending.as_mut().poll(&mut context), Poll::Pending));
        }
        assert_eq!(entered.load(Ordering::SeqCst), 2);
        assert_eq!(admission_gate().unwrap().in_use(), 2);
        for ledger in &ledgers {
            assert_eq!(ledger.in_use(), total);
        }
        let [first, second] = futures;
        drop(first);
        assert_eq!(ledgers[0].in_use(), 0);
        assert_eq!(ledgers[1].in_use(), total);
        assert_eq!(admission_gate().unwrap().in_use(), 1);
        drop(second);
        assert_eq!(ledgers[1].in_use(), 0);
        assert_eq!(admission_gate().unwrap().in_use(), 0);
        assert_eq!(static_egress_gate().unwrap().in_use(), 0);

        // Test injection may reduce the allowance, never inflate the boot row.
        let inflated = Arc::new(AuctionBudget::new(total + 1));
        let mut pending = Box::pin(app.dispatch_ingress(
            request(&body, &inflated, &entered),
            MonotonicInstant::now(),
            IngressHeadAccounting::HostManaged,
            IngressFraming::HostManaged,
        ));
        assert!(matches!(pending.as_mut().poll(&mut context), Poll::Pending));
        assert_eq!(entered.load(Ordering::SeqCst), 3);
        assert_eq!(inflated.in_use(), 0);
        drop(pending);

        let mut amplified = body.clone();
        amplified["imp"] = json!(
            (0_u32..100)
                .map(|index| {
                    let mut imp = body["imp"][0].clone();
                    imp["id"] = json!(index.to_string());
                    imp
                })
                .collect::<Vec<_>>()
        );
        amplified["ext"] = json!({"prebid":{"bidderparams":{"fake_bb":{
            "p":vec![json!({"":[]});20_000]
        }}}});
        let amplified_bytes = serde_json::to_vec(&amplified).unwrap();
        assert!(amplified_bytes.len() < usize::try_from(limits.inbound_request_bytes).unwrap());
        bounded_json::count_json_nodes(&amplified_bytes, limits.max_parsed_nodes).unwrap();
        // Ingress fits, but repeating the supplement across 100 imps exceeds
        // the materialization node cap before the projection receipt is checked.
        let ledger = Arc::new(AuctionBudget::new(total));
        let outcome = block_on(app.dispatch_ingress(
            request(&amplified, &ledger, &entered),
            MonotonicInstant::now(),
            IngressHeadAccounting::HostManaged,
            IngressFraming::HostManaged,
        ))
        .unwrap();
        let IngressDispatchOutcome::Response(mut envelope) = outcome else {
            panic!("expected refusal");
        };
        assert_eq!(
            envelope.response_mut().status(),
            http::StatusCode::BAD_REQUEST
        );
        let refusal_body = mem::take(envelope.response_mut().body_mut());
        assert_eq!(
            block_on(refusal_body.into_bytes_bounded(256))
                .unwrap()
                .as_ref(),
            b"Invalid request: bidder-scoped supplement exceeds the materialization budget\n"
        );
        assert_eq!(
            entered.load(Ordering::SeqCst),
            3,
            "amplification must refuse before fanout"
        );
        assert_eq!(ledger.in_use(), 0);
        assert_eq!(admission_gate().unwrap().in_use(), 0);
        drop(envelope);
        assert_eq!(static_egress_gate().unwrap().in_use(), 0);
    }
}
