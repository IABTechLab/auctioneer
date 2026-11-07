//! Real HTTP admission and portable retained-result capacity conformance.

mod support;

#[cfg(test)]
mod fanout_limits;
use fanout_limits as envelope;

#[cfg(test)]
use auctioneer_bidder::test_support::serving_catalog;

#[cfg(test)]
mod tests {
    use super::{envelope, serving_catalog, support};

    use auctioneer_bidder::{BidderHttpResponse, EntryPoint};
    use auctioneer_config_data::types::BidderImplementationBinding;
    use auctioneer_core::auction::audit::construction_costs as core_construction_costs;
    use auctioneer_core::auction::{
        AuctionFailure, AuctionFailureReason, AuctionFunding, AuctionInput, AuctionResourceInputs,
        AuctionResourceProfile, AuctionServices, CompletedAuction, CompletedResponse,
        SchemaAllowance, prepare_auction,
    };
    use auctioneer_core::config::AuctioneerAppConfig;
    use auctioneer_core::handlers::{TestFanoutHandle, auction::TestAuctionBudget};
    use auctioneer_core::host_primitives::{
        TestClockHandle, TestMonotonicClockHandle, TestRngHandle, admission_gate, boot_gates,
        install_boot_envelope, resolve_budget_inputs, static_egress_gate,
    };
    use auctioneer_core::test_support::bidder_fixture;
    use auctioneer_exchange::{
        AuctionDeadline, AuctionError, BidderCall, BidderCallResult, BidderFanout,
        FanoutInternalError,
    };
    use auctioneer_openrtb::{RequestHeaders, types::bid_request::BidRequest};
    use auctioneer_privacy::{UnrestrictedHostRestrictions, gcl::Gcl, gvl::GvlStore};
    use auctioneer_runtime::{
        AdmissionGate, AuctionBudget, CatalogResourceCosts, Clock as _, FakeClock, SeededRng,
        TargetPlatformConstants, bounded_json,
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
        Arc, Weak,
        atomic::{AtomicUsize, Ordering},
    };
    use std::task::{Context, Poll};
    use std::time::Duration;

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
            "schema_version = 1\n[host.aliases]\nfake_bb={:?}\n[host.gdpr]\nenabled = false\n",
            serving_catalog::passthrough_info().name,
        );
        // Only install the extraction binding here: dispatch_ingress below
        // acquires and retains the original admission and terminal owners.
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

    struct RetentionFanout {
        calls: AtomicUsize,
        fail: bool,
    }

    #[async_trait::async_trait(?Send)]
    impl BidderFanout for RetentionFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            assert!(!calls.is_empty());
            self.calls.fetch_add(calls.len(), Ordering::SeqCst);
            if self.fail {
                return Err(FanoutInternalError::Internal(
                    "fake retained failure".into(),
                ));
            }
            Ok(calls
                .into_iter()
                .map(|_call| BidderCallResult {
                    result: Ok(BidderHttpResponse::new(
                        200,
                        Vec::new(),
                        serde_json::to_vec(&json!({
                            "id": "fake_response", "cur": "USD", "seatbid": [{
                                "seat": "fake_upstream", "bid": [{
                                    "id": "fake_bid", "impid": "fake_imp", "price": 1.5_f64,
                                    "adm": "fake_creative", "crid": "fake_creative_id",
                                    "w": 300_u32, "h": 250_u32, "mtype": 1_u8
                                }]
                            }]
                        }))
                        .unwrap(),
                    )),
                    elapsed: Duration::from_millis(1),
                })
                .collect())
        }
    }

    struct RetentionFixture {
        profile: AuctionResourceProfile,
        settings: AuctioneerAppConfig,
        bindings: Vec<BidderImplementationBinding>,
        clock: FakeClock,
        rng: SeededRng,
        fanout: RetentionFanout,
    }

    impl RetentionFixture {
        fn new(fail: bool) -> Self {
            let inputs = resolve_budget_inputs(&envelope::limits()).unwrap();
            let profile = AuctionResourceProfile::new(
                AuctionResourceInputs {
                    limits: inputs.limits,
                    target: target(),
                    outbound_meta_bytes: auctioneer_bidder::outbound_meta_bytes(100),
                    construction_costs: core_construction_costs(),
                    product_default_bytes: inputs.baked_config_bytes,
                    catalog_costs: CatalogResourceCosts::UNCHANGED,
                },
                SchemaAllowance::Disabled,
            )
            .unwrap();
            let registry = bidder_fixture::registry();
            Self {
                profile,
                settings: serde_json::from_value(json!({
                    "schema_version": 1_u8, "host": {"gdpr": {"enabled": false}}
                }))
                .unwrap(),
                bindings: registry
                    .infos()
                    .map(|info| BidderImplementationBinding {
                        bidder: info.name.as_str(),
                        implementation: registry.implementation(info.name.as_str()).unwrap(),
                    })
                    .collect(),
                clock: FakeClock::new(time::OffsetDateTime::UNIX_EPOCH),
                rng: SeededRng::new(42),
                fanout: RetentionFanout {
                    calls: AtomicUsize::new(0),
                    fail,
                },
            }
        }

        fn services(&self) -> AuctionServices<'_> {
            AuctionServices {
                resources: &self.profile,
                product_host: auctioneer_config_data::product_host_default(),
                product_account: auctioneer_config_data::product_account_default(),
                bidders: bidder_fixture::registry(),
                bidder_bindings: &self.bindings,
                fanout: &self.fanout,
                clock: &self.clock,
                rng: &self.rng,
                gvl_store: GvlStore::baked(),
                gcl: Gcl::baked(),
                restrictions: &UnrestrictedHostRestrictions,
                credentials: None,
                request_start: self.clock.now_monotonic(),
                outer_deadline: None,
                now: self.clock.now(),
                debug_requested: false,
                entry_point: EntryPoint::Auction,
            }
        }

        fn execute(
            &self,
            gate: &Arc<AdmissionGate>,
        ) -> (
            Result<CompletedAuction, AuctionFailure>,
            Weak<AuctionBudget>,
        ) {
            let total = self.profile.derived().auction_charges.total().unwrap();
            let ledger = Arc::new(AuctionBudget::new(total));
            let weak = Arc::downgrade(&ledger);
            assert_eq!(ledger.in_use(), 0);
            let permit = gate.try_acquire().expect("accepted population fits");
            let occupied = gate.in_use();
            let funding = AuctionFunding::prepay(&self.profile, &ledger, permit).unwrap();
            assert_eq!(gate.in_use(), occupied);
            assert_eq!(ledger.in_use(), total);
            let input: BidRequest = serde_json::from_value(json!({
                "id": "fake_request", "site": {"id": "fake_site"}, "imp": [{
                    "id": "fake_imp", "banner": {"w": 300_u32, "h": 250_u32},
                    "ext": {"prebid": {"bidder": {"fake_aaa": {}}}}
                }]
            }))
            .unwrap();
            let before = serde_json::to_value(&input).unwrap();
            let prepared = prepare_auction(
                &self.settings,
                AuctionInput::Typed {
                    request: &input,
                    headers: RequestHeaders::default(),
                },
                self.services(),
                funding,
            )
            .unwrap();
            assert_eq!(serde_json::to_value(&input).unwrap(), before);
            drop(input);
            let mut running = Box::pin(prepared.run());
            let result = block_on(running.as_mut());
            drop(running);
            drop(ledger);
            (result, weak)
        }
    }

    fn assert_typed_data(completed: &CompletedAuction) {
        let outcome = completed.outcome();
        assert_eq!(outcome.request_id(), "fake_request");
        assert_eq!(outcome.impression_ids(), ["fake_imp"]);
        assert_eq!(outcome.bidders().len(), 1);
        let bundle = outcome.bidders().get("fake_aaa").unwrap();
        assert!(bundle.dispatched);
        assert_eq!(bundle.bids.len(), 1);
        let selected = outcome.selected_bid(0).expect("nonempty selected bid");
        assert_eq!(selected.requested_bidder.as_str(), "fake_aaa");
        assert_eq!(selected.implementation, Some("fake_protocol"));
        assert_eq!(selected.bid.bid.id, "fake_bid");
        assert_eq!(selected.bid.bid.impid, "fake_imp");
        assert_eq!(selected.bid.bid.adm.as_deref(), Some("fake_creative"));
        assert_eq!(selected.bid.source_seat.as_deref(), Some("fake_upstream"));
    }

    fn assert_wire_data(completed: &CompletedResponse) {
        let response = completed.response();
        assert_eq!(response.id, "fake_request");
        assert_eq!(response.seatbid.len(), 1);
        let seat = &response.seatbid[0];
        assert_eq!(seat.seat, "fake_aaa");
        assert_eq!(seat.bid.len(), 1);
        assert_eq!(seat.bid[0].id, "fake_bid");
        assert_eq!(seat.bid[0].impid, "fake_imp");
        assert_eq!(seat.bid[0].adm.as_deref(), Some("fake_creative"));
    }

    fn assert_failure_data(failure: &AuctionFailure) {
        let AuctionFailureReason::Engine(AuctionError::Internal(message)) = failure.reason() else {
            panic!("expected the dispatched fanout's heap-owning failure");
        };
        assert_eq!(message, "fake retained failure");
        assert!(!message.is_empty());
        assert!(message.capacity() > 0);
    }

    fn assert_retained_at_capacity<T>(
        fail: bool,
        retain: impl Fn(Result<CompletedAuction, AuctionFailure>) -> T,
        assert_data: impl Fn(&T),
    ) {
        let fixture = RetentionFixture::new(fail);
        let capacity = fixture.profile.inputs().limits.max_concurrent_auctions;
        assert_eq!(capacity, 2, "preserve the existing accepted population");
        let total = fixture.profile.derived().auction_charges.total().unwrap();
        assert!(total > 0);
        // This local gate supplies already-acquired permits, not a serving pool.
        let gate = AdmissionGate::new(capacity);
        let weak_gate = Arc::downgrade(&gate);
        let mut retained = Vec::new();
        let mut ledgers = Vec::new();
        for _ in 0..capacity {
            let (result, ledger) = fixture.execute(&gate);
            let owner = retain(result);
            assert_data(&owner);
            assert_eq!(ledger.upgrade().unwrap().in_use(), total);
            retained.push(owner);
            ledgers.push(ledger);
        }
        assert_eq!(fixture.fanout.calls.load(Ordering::SeqCst), 2);
        drop(fixture);
        drop(gate);

        // Only returned owners remain; weak observations cannot fund retention.
        for (owner, ledger) in retained.iter().zip(&ledgers) {
            assert_data(owner);
            assert_eq!(ledger.upgrade().unwrap().in_use(), total);
        }
        assert_eq!(weak_gate.upgrade().unwrap().in_use(), capacity);
        assert!(weak_gate.upgrade().unwrap().try_acquire().is_none());
        drop(retained.remove(0));
        assert!(ledgers[0].upgrade().is_none());
        assert_eq!(
            weak_gate.upgrade().unwrap().in_use(),
            capacity.checked_sub(1).unwrap()
        );
        let replacement = weak_gate
            .upgrade()
            .unwrap()
            .try_acquire()
            .expect("destroying one owner restores one slot");
        assert_eq!(weak_gate.upgrade().unwrap().in_use(), capacity);
        assert!(weak_gate.upgrade().unwrap().try_acquire().is_none());
        assert_eq!(retained.len(), 1);
        assert_data(&retained[0]);
        assert_eq!(ledgers[1].upgrade().unwrap().in_use(), total);
        drop(retained);
        for ledger in ledgers {
            assert!(ledger.upgrade().is_none());
        }
        let retained_gate = weak_gate.upgrade().unwrap();
        assert_eq!(
            retained_gate.in_use(),
            1,
            "only the replacement permit survives"
        );
        drop(replacement);
        assert_eq!(retained_gate.in_use(), 0);
        let mut permits = Vec::new();
        for _ in 0..capacity {
            permits.push(retained_gate.try_acquire().unwrap());
        }
        assert!(retained_gate.try_acquire().is_none());
        drop(permits);
        assert_eq!(retained_gate.in_use(), 0);
        drop(retained_gate);
        assert!(weak_gate.upgrade().is_none());
    }

    #[test]
    fn nonempty_typed_completion_retains_original_capacity_and_ledger() {
        assert_retained_at_capacity(false, Result::unwrap, assert_typed_data);
    }

    #[test]
    fn consuming_wire_projection_retains_original_capacity_and_ledger() {
        assert_retained_at_capacity(
            false,
            |result| {
                let typed = result.unwrap();
                assert_typed_data(&typed);
                let creative = typed
                    .outcome()
                    .selected_bid(0)
                    .unwrap()
                    .bid
                    .bid
                    .adm
                    .as_ref()
                    .unwrap()
                    .as_ptr();
                let projected = typed.into_wire_response().unwrap();
                assert_wire_data(&projected);
                assert_eq!(
                    projected.response().seatbid[0].bid[0]
                        .adm
                        .as_ref()
                        .unwrap()
                        .as_ptr(),
                    creative
                );
                projected
            },
            assert_wire_data,
        );
    }

    #[test]
    fn heap_owning_failure_retains_original_capacity_and_ledger() {
        assert_retained_at_capacity(true, Result::unwrap_err, assert_failure_data);
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
