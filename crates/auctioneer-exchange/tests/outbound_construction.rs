#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

mod support;

#[cfg(test)]
mod tests {
    use crate::fake_bidder::FakeBidder;
    use crate::support::response_budgets;
    use async_trait::async_trait;
    use auctioneer_bidder::{
        Bidder, BidderError, BidderHttpResponse, BidderRegistry, BidderRequestInfo, BidderResponse,
        DebugBodySchema, OutboundRequestMeta, OutboundRequestSink,
    };
    use auctioneer_config_data::types::{AccountConfig, BidderInfo, HostConfig};
    use auctioneer_exchange::construction_costs;
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::split::group_by_bidder;
    use auctioneer_exchange::{
        AccountSource, AuctionContext, AuctionDeadline, AuctionError, AuctionPlan, BidderCall,
        BidderCallResult, BidderFanout, FanoutInternalError, ResolvedAccount, resolve_aliases,
        run_auction,
    };
    use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::BidResponse};
    use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare};
    use auctioneer_privacy::{gcl::Gcl, gvl::GvlStore};
    use auctioneer_runtime::{
        AuctionBudget, ChargeKind, Clock as _, FakeClock, ResourceError, SeededRng,
    };
    use futures::executor::block_on;
    use serde_json::{Value, json};
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Default)]
    struct RecordingFanout(Mutex<Vec<String>>);

    #[async_trait(?Send)]
    impl BidderFanout for RecordingFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            Ok(calls
                .into_iter()
                .map(|call| {
                    self.0.lock().unwrap().push(call.request.url().to_owned());
                    BidderCallResult {
                        result: Ok(BidderHttpResponse::new(204, vec![], vec![])),
                        elapsed: Duration::ZERO,
                    }
                })
                .collect())
        }
    }

    fn run(host: &HostConfig, registry: &BidderRegistry, request: Value) -> (Value, Vec<String>) {
        let (response, sent) = run_response(host, registry, request);
        (response.ext.unwrap(), sent)
    }

    fn run_response(
        host: &HostConfig,
        registry: &BidderRegistry,
        request: Value,
    ) -> (BidResponse, Vec<String>) {
        let (response, sent) = run_with_funding(host, registry, request, None);
        (response.unwrap(), sent)
    }

    #[derive(Clone, Copy)]
    enum Funding {
        Exact,
        OneShort,
        SeparateOnly,
        WrongPhase,
    }

    fn run_with_funding(
        host: &HostConfig,
        registry: &BidderRegistry,
        request: Value,
        funding: Option<Funding>,
    ) -> (Result<BidResponse, AuctionError>, Vec<String>) {
        let normalized = prepare(
            serde_json::from_value(request).unwrap(),
            &RequestHeaders::default(),
            &PrepareLimits {
                max_impressions: 100,
                ..PrepareLimits::default()
            },
        )
        .unwrap();
        let account = AccountConfig::default();
        let resolved = ResolvedAccount {
            id: "account".into(),
            source: AccountSource::DefaultFallback,
            config: &account,
        };
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let rng = SeededRng::new(42);
        let fanout = RecordingFanout::default();
        let mut ctx = AuctionContext::with_request_start(
            clock.now_monotonic(),
            host,
            &resolved,
            registry,
            &fanout,
            &clock,
            &rng,
            GvlStore::baked(),
            Gcl::baked(),
            clock.now(),
            false,
        );
        let plan = AuctionPlan::new(
            resolve_aliases(&normalized, registry, &BTreeSet::new(), &host.aliases).unwrap(),
            BTreeSet::new(),
            response_budgets(ctx.bidders, ctx.host),
            BTreeMap::new(),
        );
        let grouped = group_by_bidder(
            &normalized,
            &plan.aliases.map,
            registry,
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .unwrap();
        let names = grouped.bidder_imps().keys().cloned().collect::<Vec<_>>();
        let construction = construction_costs();
        let retained = u64::try_from(names.len())
            .unwrap()
            .checked_mul(construction.retained_per_scope_bytes)
            .unwrap();
        let projection = grouped
            .projection_charge(&names)
            .unwrap()
            .checked_add(retained)
            .unwrap()
            .checked_add(construction.reusable_scratch_bytes)
            .unwrap();
        let diagnostics = u64::try_from(names.len())
            .unwrap()
            .checked_mul(98_304)
            .unwrap()
            .checked_add(4_096)
            .unwrap();
        let combined = projection.checked_add(diagnostics).unwrap();
        let ledger = AuctionBudget::new(combined);
        let receipt = funding.map(|kind| {
            let bytes = match kind {
                Funding::Exact | Funding::WrongPhase => combined,
                Funding::OneShort => combined.checked_sub(1).unwrap(),
                Funding::SeparateOnly => projection.max(diagnostics),
            };
            let phase = if matches!(kind, Funding::WrongPhase) {
                ChargeKind::Response
            } else {
                ChargeKind::Projection
            };
            ledger.try_reserve(phase, bytes).unwrap()
        });
        ctx.projection_reservation = receipt.as_ref();
        let mut audit = DisclosureAudit::default();
        let response = block_on(run_auction(&normalized, &ctx, &plan, &mut audit))
            .and_then(auctioneer_exchange::AuctionOutcome::into_bid_response);
        (response, fanout.0.into_inner().unwrap())
    }

    fn host() -> HostConfig {
        let mut host = auctioneer_config_data::host_default().clone();
        host.gdpr.enabled = false;
        host
    }

    fn registry() -> BidderRegistry {
        registry_with_first(Box::new(FakeBidder::default()), true)
    }

    fn registry_with_first(first: Box<dyn Bidder>, first_uses_https: bool) -> BidderRegistry {
        let mut registry = BidderRegistry::new();
        for (name, bidder, uses_https) in [
            ("fake_first", first, first_uses_https),
            ("fake_second", Box::new(FakeBidder::default()), true),
        ] {
            registry
                .register(
                    Box::leak(Box::new(BidderInfo {
                        name: name.into(),
                        endpoint: Some("https://example.test/bid".into()),
                        uses_https,
                        ..BidderInfo::default()
                    })),
                    bidder,
                )
                .expect("unique test bidder");
        }
        registry
    }

    fn request(padding: usize) -> Value {
        json!({"id": "auction", "site": {"id": "site"}, "payload": "x".repeat(padding),
            "imp": [{"id": "imp-1", "banner": {"w": 300_i32, "h": 250_i32}, "ext": {"bidder": {"fake_first": {}, "fake_second": {}}}}],
            "ext": {"prebid": {"returnallbidstatus": true}}})
    }

    #[derive(Default)]
    struct PartialBidder {
        fatal: bool,
        calls: bool,
        entered: Arc<Mutex<u32>>,
        additional_rows: usize,
        sink_failures: usize,
    }

    impl Bidder for PartialBidder {
        fn debug_body_schema(&self) -> DebugBodySchema {
            DebugBodySchema::Unsupported
        }

        fn make_requests(
            &self,
            request: &BidRequest,
            info: &BidderRequestInfo<'_>,
            sink: &mut OutboundRequestSink<'_>,
        ) -> Result<(), Vec<BidderError>> {
            if let Ok(mut count) = self.entered.lock() {
                *count = count.checked_add(1).unwrap_or(u32::MAX);
            }
            for _ in 0..=self.additional_rows {
                sink.diagnostic(
                    auctioneer_bidder::ConstructionDiagnostic::InvalidImpressionParameters,
                );
            }
            for _ in 0..self.sink_failures {
                let _refused =
                    sink.push_request(info, |meta| meta.set_url("not a URL"), |_body| Ok(()));
            }
            if self.calls {
                FakeBidder::default().make_requests(request, info, sink)?;
            }
            if self.fatal {
                Err(vec![BidderError::bad_input("bidder-fatal failure")])
            } else {
                Ok(())
            }
        }

        fn make_bids(
            &self,
            request: &BidRequest,
            sent: &OutboundRequestMeta,
            response: &BidderHttpResponse,
        ) -> Result<BidderResponse, Vec<BidderError>> {
            FakeBidder::default().make_bids(request, sent, response)
        }
    }

    #[test]
    fn partial_diagnostics_survive_accepted_siblings_zero_calls_and_fatal_refunds() {
        for (calls, fatal, expected_calls) in [(true, false, 2), (false, false, 1), (true, true, 1)]
        {
            let bidders = registry_with_first(
                Box::new(PartialBidder {
                    fatal,
                    calls,
                    entered: Arc::default(),
                    ..PartialBidder::default()
                }),
                true,
            );
            let mut config = host();
            config.resource_limits.max_outbound_requests = 2;
            let (ext, sent) = run(&config, &bidders, request(0));
            assert_eq!(sent.len(), expected_calls);
            let rows = ext["errors"]["fake_first"].as_array().unwrap();
            assert_eq!(rows.len(), if fatal { 2 } else { 1 });
            assert_eq!(
                rows.last().unwrap(),
                &json!({"code": 2_u32, "message": "invalid impression parameters"})
            );
            assert!(ext["errors"].get("fake_second").is_none());
        }
    }

    #[test]
    fn zero_call_alias_diagnostics_require_the_combined_projection_receipt_before_construction() {
        for funding in [
            Funding::Exact,
            Funding::OneShort,
            Funding::SeparateOnly,
            Funding::WrongPhase,
        ] {
            let entered = Arc::new(Mutex::new(0));
            let mut bidders = BidderRegistry::new();
            bidders
                .register(
                    Box::leak(Box::new(BidderInfo {
                        name: "fake_first".into(),
                        endpoint: Some("https://example.test/bid".into()),
                        uses_https: true,
                        ..BidderInfo::default()
                    })),
                    Box::new(PartialBidder {
                        fatal: false,
                        calls: false,
                        entered: Arc::clone(&entered),
                        ..PartialBidder::default()
                    }),
                )
                .expect("unique test bidder");
            let req = json!({"id": "auction", "site": {"id":"site"},
                "imp": [{"id":"imp-1", "banner":{"w":300_i32,"h":250_i32}, "ext":{"bidder":{"fake_alias_a":{},"fake_alias_b":{}}}}],
                "ext":{"prebid":{"aliases":{"fake_alias_a":"fake_first","fake_alias_b":"fake_first"}}}});
            let (result, sent) = run_with_funding(&host(), &bidders, req, Some(funding));
            assert!(sent.is_empty());
            if matches!(funding, Funding::Exact) {
                let response = result.unwrap();
                assert_eq!(*entered.lock().unwrap(), 2);
                let ext = response.ext.unwrap();
                assert_eq!(ext["errors"].as_object().unwrap().len(), 2);
                for alias in ["fake_alias_a", "fake_alias_b"] {
                    assert_eq!(
                        ext["errors"][alias],
                        json!([{"code":2_u32,"message":"invalid impression parameters"}])
                    );
                }
            } else {
                assert!(matches!(
                    result,
                    Err(AuctionError::Resource(ResourceError::Exhausted {
                        kind: ChargeKind::Projection
                    }))
                ));
                assert_eq!(*entered.lock().unwrap(), 0);
            }
        }
    }

    #[test]
    fn diagnostic_and_sink_error_windows_remain_independent_on_the_wire() {
        let bidders = registry_with_first(
            Box::new(PartialBidder {
                additional_rows: 32,
                sink_failures: 16,
                ..PartialBidder::default()
            }),
            true,
        );
        let (ext, sent) = run(&host(), &bidders, request(0));
        assert_eq!(sent.len(), 1);
        let rows = ext["errors"]["fake_first"].as_array().unwrap();
        assert_eq!(rows.len(), 48);
        assert_eq!(rows.iter().filter(|row| row["code"] == 5_u32).count(), 16);
        assert_eq!(rows.iter().filter(|row| row["code"] == 2_u32).count(), 32);
        assert_eq!(
            rows.last().unwrap()["message"],
            "additional construction diagnostics suppressed"
        );
    }

    type Entered = Arc<Mutex<usize>>;
    type Contexts = Arc<Mutex<Vec<Vec<String>>>>;

    struct Scripted {
        lengths: Vec<usize>,
        semantic_error: bool,
        entered: Entered,
        contexts: Contexts,
    }

    impl Bidder for Scripted {
        fn debug_body_schema(&self) -> DebugBodySchema {
            DebugBodySchema::Unsupported
        }

        fn make_requests(
            &self,
            _: &BidRequest,
            info: &BidderRequestInfo<'_>,
            sink: &mut OutboundRequestSink<'_>,
        ) -> Result<(), Vec<BidderError>> {
            for (index, len) in self.lengths.iter().enumerate() {
                let _outcome = sink.push_request(
                    info,
                    |meta| {
                        let mut entered = self.entered.lock().unwrap();
                        *entered = entered.checked_add(1).expect("fixture counter fits usize");
                        drop(entered);
                        meta.set_method("POST")?;
                        meta.set_url(info.endpoint_template)?;
                        meta.header("X-Context", "retained")?;
                        meta.imp_id(&format!("imp-{index}"))?;
                        Ok(())
                    },
                    |writer| writer.write_all(&vec![b'x'; *len]),
                );
            }
            if self.semantic_error {
                Err(vec![BidderError::bad_input("bad params")])
            } else {
                Ok(())
            }
        }
        #[expect(
            clippy::panic_in_result_fn,
            clippy::unwrap_in_result,
            reason = "test spy asserts retained metadata and treats poisoned recording state as a test failure"
        )]
        fn make_bids(
            &self,
            _: &BidRequest,
            sent: &OutboundRequestMeta,
            _: &BidderHttpResponse,
        ) -> Result<BidderResponse, Vec<BidderError>> {
            assert!(
                sent.headers()
                    .any(|(key, value)| key == "X-Context" && value == "retained")
            );
            self.contexts
                .lock()
                .unwrap()
                .push(sent.covered_imp_ids().map(str::to_owned).collect());
            Ok(BidderResponse {
                bids: vec![],
                currency: "USD".into(),
                errors: vec![],
                auction_configs: Vec::new(),
            })
        }
    }

    fn scripted_registry(
        lengths: Vec<usize>,
        semantic_error: bool,
    ) -> (BidderRegistry, Entered, Contexts) {
        let entered = Arc::new(Mutex::new(0_usize));
        let contexts = Arc::new(Mutex::new(Vec::new()));
        let registry = registry_with_first(
            Box::new(Scripted {
                lengths,
                semantic_error,
                entered: Arc::clone(&entered),
                contexts: Arc::clone(&contexts),
            }),
            false,
        );
        (registry, entered, contexts)
    }

    #[test]
    fn bidder_reads_request_context_in_make_bids() {
        let (registry, _, contexts) = scripted_registry(vec![2], false);
        let (_, sent) = run(&host(), &registry, request(0));
        assert_eq!(sent.len(), 2);
        assert_eq!(*contexts.lock().unwrap(), [vec!["imp-0"]]);
    }

    #[test]
    fn every_count_rejection_is_counted_without_running_metadata_or_emitting_code5() {
        let (registry, entered, _) = scripted_registry(vec![2; 30], false);
        let mut host = host();
        host.resource_limits.max_outbound_requests = 1;
        let (ext, sent) = run(&host, &registry, request(0));
        assert_eq!(sent.len(), 1);
        assert_eq!(*entered.lock().unwrap(), 1);
        assert_eq!(
            ext["warnings"]["fake_first"],
            json!([{"code": 10_999_i32, "message": "29 outbound request attempt(s) rejected by max_outbound_requests"}])
        );
        assert!(ext["errors"]["fake_first"].is_null());
    }

    #[test]
    fn semantic_failure_discards_calls_but_keeps_sink_errors_and_refunds_for_sibling() {
        let (registry, entered, _) = scripted_registry(vec![2048, 2, 2, 2], true);
        let mut host = host();
        host.resource_limits.max_outbound_raw_bytes = 1024;
        host.resource_limits.max_outbound_requests = 1;
        let (ext, sent) = run(&host, &registry, request(0));
        assert_eq!(sent.len(), 1, "second bidder reuses refunded reservation");
        assert_eq!(*entered.lock().unwrap(), 2);
        assert_eq!(
            ext["errors"]["fake_first"],
            json!([
                {"code": 2_i32, "message": "bad params"},
                {"code": 5_i32, "message": "outbound request construction failed"}
            ])
        );
        assert_eq!(
            ext["warnings"]["fake_first"],
            json!([{"code": 10_999_i32, "message": "2 outbound request attempt(s) rejected by max_outbound_requests"}])
        );
        assert!(ext["errors"]["fake_second"].is_null());
    }

    #[test]
    fn rejected_encoded_middle_slot_preserves_sibling_positions_and_contexts() {
        let (registry, _, contexts) = scripted_registry(vec![2, 2048, 2], false);
        let mut host = host();
        host.resource_limits.max_outbound_request_bytes = 1024;
        let (ext, sent) = run(&host, &registry, request(0));
        assert_eq!(sent.len(), 3);
        assert_eq!(*contexts.lock().unwrap(), [vec!["imp-0"], vec!["imp-2"]]);
        assert_eq!(
            ext["errors"]["fake_first"],
            json!([{"code": 5_i32, "message": "bidder request too large"}])
        );
        assert_eq!(
            ext["prebid"]["seatnonbid"],
            json!([{"seat": "fake_first", "nonbid": [{"impid": "imp-1", "statuscode": 100_i32}]}])
        );
    }

    #[test]
    fn raw_oversize_is_fixed_code5_and_never_dispatched() {
        let mut host = host();
        host.resource_limits.max_outbound_raw_bytes = 1024;
        let (ext, sent) = run(&host, &registry(), request(2048));
        assert!(sent.is_empty());
        for name in ["fake_first", "fake_second"] {
            assert_eq!(
                ext["errors"][name],
                json!([{"code": 5_i32, "message": "outbound request construction failed"}])
            );
        }
    }

    #[test]
    fn count_cap_reports_exact_attempt_warning_without_error_or_seatnonbid() {
        let mut host = host();
        host.resource_limits.max_outbound_requests = 1;
        let (ext, sent) = run(&host, &registry(), request(0));
        assert_eq!(sent.len(), 1);
        assert_eq!(
            ext["warnings"]["fake_second"],
            json!([{"code": 10_999_i32, "message": "1 outbound request attempt(s) rejected by max_outbound_requests"}])
        );
        assert!(ext["errors"]["fake_second"].is_null());
        assert!(
            ext["prebid"]["seatnonbid"]
                .as_array()
                .is_none_or(|rows| rows.iter().all(|row| row["seat"] != "fake_second"))
        );
    }

    #[test]
    fn encoded_oversize_is_code5_with_positional_seat100() {
        let mut host = host();
        host.resource_limits.max_outbound_request_bytes = 1024;
        let (response, sent) = run_response(&host, &registry(), request(2048));
        assert_eq!(response.nbr, Some(7_i32));
        let ext = response.ext.unwrap();
        assert!(sent.is_empty());
        for name in ["fake_first", "fake_second"] {
            assert!(
                ext["responsetimemillis"][name].is_null(),
                "unsent bidder has no wire timing"
            );
            assert_eq!(
                ext["errors"][name],
                json!([{"code": 5_i32, "message": "bidder request too large"}])
            );
        }
        let rows = ext["prebid"]["seatnonbid"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert!(
            rows.iter()
                .all(|row| row["nonbid"] == json!([{"impid": "imp-1", "statuscode": 100_i32}]))
        );
    }
}
