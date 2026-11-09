//! Response classification must precede adapter parsing on both fanout paths.

#[cfg(test)]
pub use auctioneer_core::test_support::bidder_fixture;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use super::bidder_fixture::fake_bidder::FakeBidder;
    use super::fanout_limits as envelope;

    use auctioneer_bidder::{
        Bidder, BidderError, BidderHttpResponse, BidderRegistry, BidderRequestInfo, BidderResponse,
        DebugBodySchema, OutboundRequestMeta, OutboundRequestSink,
    };
    use auctioneer_config_data::types::{AccountConfig, BidderInfo};
    use auctioneer_core::config::{AuctioneerAppConfig, budgets::resolve_budgets};
    use auctioneer_core::host_primitives::{
        DevFanout, EdgeZeroFanout, EdgeZeroProxyAdapter, RaceTimersStrategy,
    };
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::{
        AccountSource, AuctionContext, AuctionPlan, BidderFanout, ResolvedAccount, ResolvedAliases,
        run_auction,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare};
    use auctioneer_privacy::{gcl::Gcl, gvl::GvlStore};
    use auctioneer_runtime::{
        Clock as _, FakeClock, NoopDecodeCooperator, SeededRng, StreamGate, Timer as _,
    };
    use edgezero_core::error::{BadGatewayReason, EdgeError};
    use edgezero_core::http::{HeaderMap, StatusCode};
    use edgezero_core::outbound::{
        HttpClient, IndependentOutboundProgress, OutboundBatch, OutboundBatchDriverEvent,
        OutboundDispatchRequest, OutboundExchangeOwner, OutboundFuture, OutboundHead,
        OutboundHeaderFidelity, OutboundHeaderInput, OutboundHttpClient, OutboundPending,
        OutboundPreparedBatch, OutboundProgressRuntime, OutboundRequest, OutboundSendMode,
        OutboundTime,
    };
    use edgezero_core::time::MonotonicClock;
    use futures::StreamExt as _;
    use futures::{executor::block_on, stream};
    use serde_json::{Value, json};
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::Error as IoError;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingBidder(Arc<AtomicUsize>);

    impl Bidder for CountingBidder {
        fn debug_body_schema(&self) -> DebugBodySchema {
            DebugBodySchema::Unsupported
        }

        fn make_requests(
            &self,
            request: &BidRequest,
            info: &BidderRequestInfo<'_>,
            sink: &mut OutboundRequestSink<'_>,
        ) -> Result<(), Vec<BidderError>> {
            FakeBidder::default().make_requests(request, info, sink)
        }

        fn make_bids(
            &self,
            request: &BidRequest,
            sent: &OutboundRequestMeta,
            response: &BidderHttpResponse,
        ) -> Result<BidderResponse, Vec<BidderError>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            FakeBidder::default().make_bids(request, sent, response)
        }
    }

    const HOSTILE_DETAIL: &str = "https://user:secret@private.example/rtb?token=secret";

    #[derive(Clone, Copy)]
    enum Outcome {
        Status(u16),
        ServiceUnavailable,
        Internal,
        BadGateway(BadGatewayReason),
    }

    #[derive(Clone)]
    struct ScriptedClient {
        outcome: Outcome,
        clock: Arc<FakeClock>,
        dispatches: Arc<AtomicUsize>,
    }

    impl ScriptedClient {
        #[expect(
            clippy::unwrap_in_result,
            reason = "scripted test responses require valid fixture status codes and serializable JSON"
        )]
        fn head(&self, request: OutboundDispatchRequest) -> Result<OutboundHead, EdgeError> {
            self.dispatches.fetch_add(1, Ordering::Relaxed);
            let status = match self.outcome {
                Outcome::Status(status) => status,
                Outcome::ServiceUnavailable => {
                    return Err(EdgeError::service_unavailable(HOSTILE_DETAIL));
                }
                Outcome::Internal => {
                    return Err(EdgeError::internal(IoError::other(HOSTILE_DETAIL)));
                }
                Outcome::BadGateway(reason) => {
                    return Err(EdgeError::bad_gateway_with_reason(HOSTILE_DETAIL, reason));
                }
            };
            let body = json!({
                "id": "auction", "cur": "USD", "seatbid": [{"seat": "fake_pp", "bid": [{
                    "id": "bid", "impid": "imp-1", "price": 1.0_f64,
                    "w": 300_i32, "h": 250_i32, "adm": "creative"
                }]}]
            });
            let (_parts, context) = request.into_parts();
            OutboundHead::new(
                StatusCode::from_u16(status).unwrap(),
                OutboundHeaderInput::new(
                    "fixture",
                    HeaderMap::new(),
                    OutboundHeaderFidelity::ProviderFieldBoundariesUnknown,
                ),
                Box::pin(stream::iter([Ok(bytes::Bytes::from(
                    serde_json::to_vec(&body).unwrap(),
                ))])),
                context,
                OutboundExchangeOwner::new(|| {}),
            )
        }
    }

    impl OutboundHttpClient for ScriptedClient {
        fn begin(&self, request: OutboundDispatchRequest) -> Result<OutboundPending, EdgeError> {
            let resolved_head = self.head(request)?;
            Ok(OutboundPending::new(
                resolved_head.finish(),
                OutboundExchangeOwner::new(|| {}),
            ))
        }

        fn dispatch_head(&self, request: OutboundDispatchRequest) -> OutboundFuture<OutboundHead> {
            let head = self.head(request);
            Box::pin(async move { head })
        }

        fn progress(&self) -> Rc<dyn OutboundProgressRuntime> {
            Rc::new(IndependentOutboundProgress)
        }

        fn start_batch_until(&self, batch: OutboundPreparedBatch) -> OutboundBatch {
            let (context, slots) = batch.into_slots();
            let completions = slots
                .into_iter()
                .map(|(index, request)| {
                    let pending_head = self.head(request);
                    let clock = context.clock().clone();
                    async move {
                        let outcome = match pending_head {
                            Ok(resolved_head) => resolved_head.finish().await,
                            Err(error) => Err(error),
                        };
                        OutboundBatchDriverEvent::Item {
                            index,
                            completed_at: clock.now(),
                            outcome,
                        }
                    }
                })
                .collect::<Vec<_>>();
            OutboundBatch::from_driver(
                context,
                stream::iter(completions).then(|completion| completion),
                OutboundExchangeOwner::new(|| {}),
            )
        }

        fn time(&self) -> OutboundTime {
            let read_clock = Arc::clone(&self.clock);
            let waits = Arc::clone(&self.clock);
            OutboundTime::new(
                MonotonicClock::new(move || read_clock.now_monotonic()),
                move |deadline, clock| {
                    let waiter = Arc::clone(&waits);
                    Box::pin(async move {
                        if let Some(remaining) = deadline.remaining_at(clock.now()) {
                            waiter.sleep(remaining).await;
                        }
                        Ok(())
                    })
                },
            )
        }

        fn validate_supported_options(
            &self,
            _: &OutboundRequest,
            _: OutboundSendMode,
        ) -> Result<(), EdgeError> {
            Ok(())
        }
    }

    fn auction_result(outcome: Outcome, production: bool) -> (Value, usize, usize) {
        let parsed = Arc::new(AtomicUsize::new(0));
        let mut registry = BidderRegistry::new();
        registry
            .register(
                Box::leak(Box::new(BidderInfo {
                    name: "fake_pp".into(),
                    endpoint: "https://bidder.example/rtb".into(),
                    uses_https: true,
                    ..BidderInfo::default()
                })),
                Box::new(CountingBidder(Arc::clone(&parsed))),
            )
            .expect("unique test bidder");
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let dispatches = Arc::new(AtomicUsize::new(0));
        let client = HttpClient::with_client(ScriptedClient {
            outcome,
            clock: Arc::clone(&clock),
            dispatches: Arc::clone(&dispatches),
        });
        let proxy = EdgeZeroProxyAdapter::new(client.clone());
        let dev = DevFanout {
            client: &proxy,
            clock: clock.as_ref(),
        };
        let limits = envelope::limits();
        let wave = EdgeZeroFanout::with_wave_scheduler(
            client,
            Arc::<FakeClock>::clone(&clock),
            Arc::<FakeClock>::clone(&clock),
            Arc::new(NoopDecodeCooperator),
            Arc::new(RaceTimersStrategy),
            limits,
            StreamGate::new(limits.max_active_body_streams),
        );
        let fanout: &dyn BidderFanout = if production { &wave } else { &dev };
        let normalized = prepare(
            serde_json::from_value(json!({
                "id": "auction", "tmax": 1000_i64, "site": {"id": "site"},
                "imp": [{"id": "imp-1", "banner": {"w": 300_i32, "h": 250_i32},
                    "ext": {"bidder": {"fake_pp": {}}}}],
                "ext": {"prebid": {"returnallbidstatus": true}}
            }))
            .unwrap(),
            &RequestHeaders::default(),
            &PrepareLimits {
                max_impressions: 100,
                ..PrepareLimits::default()
            },
        )
        .unwrap();
        let mut host = auctioneer_config_data::host_default().clone();
        host.gdpr.enabled = false;
        let account = AccountConfig::default();
        let resolved = ResolvedAccount {
            id: "account".into(),
            source: AccountSource::DefaultFallback,
            config: &account,
        };
        let rng = SeededRng::new(42);
        let context = AuctionContext::with_request_start(
            clock.now_monotonic(),
            &host,
            &resolved,
            &registry,
            fanout,
            clock.as_ref(),
            &rng,
            GvlStore::baked(),
            Gcl::baked(),
            clock.now(),
            false,
        );
        let aliases = ResolvedAliases::empty();
        let plan = AuctionPlan::new(
            aliases.clone(),
            BTreeSet::new(),
            resolve_budgets(
                &AuctioneerAppConfig::default(),
                &host,
                &resolved.id,
                resolved.source,
                &registry,
                &aliases,
            ),
            BTreeMap::new(),
        );
        let mut audit = DisclosureAudit::default();
        let response = block_on(run_auction(&normalized, &context, &plan, &mut audit))
            .unwrap()
            .into_bid_response()
            .unwrap();
        (
            serde_json::to_value(response).unwrap(),
            parsed.load(Ordering::Relaxed),
            dispatches.load(Ordering::Relaxed),
        )
    }

    #[test]
    fn redirect_3xx_is_bad_server_response_without_parsing() {
        for production in [false, true] {
            for status in [302_u16, 307, 308] {
                let (response, parsed, dispatches) =
                    auction_result(Outcome::Status(status), production);
                assert_eq!(parsed, 0, "status={status}, production={production}");
                assert_eq!(dispatches, 1);
                assert_eq!(
                    response.pointer("/ext/errors/fake_pp"),
                    Some(&json!([
                        {"code": 4_u32, "message": "bidder server error"}
                    ]))
                );
                assert_eq!(
                    response.pointer("/ext/prebid/seatnonbid"),
                    Some(&json!([
                        {"seat": "fake_pp", "nonbid": [{"impid": "imp-1", "statuscode": 100_u32}]}
                    ]))
                );
                assert!(response.get("seatbid").is_none());
            }
        }
    }

    #[test]
    fn successful_bid_response_exercises_parser_counter() {
        for production in [false, true] {
            let (response, parsed, dispatches) = auction_result(Outcome::Status(200), production);
            assert_eq!(parsed, 1);
            assert_eq!(dispatches, 1);
            assert!(response.pointer("/seatbid/0/bid/0").is_some());
        }
    }

    #[test]
    fn initial_transport_failures_are_code4_status100_without_bid_parsing() {
        for outcome in [
            Outcome::ServiceUnavailable,
            Outcome::Internal,
            Outcome::BadGateway(BadGatewayReason::Transport),
            Outcome::BadGateway(BadGatewayReason::Unspecified),
        ] {
            let (response, parsed, dispatches) = auction_result(outcome, true);
            assert_eq!(parsed, 0);
            assert_eq!(dispatches, 1);
            assert_eq!(
                response.pointer("/ext/errors/fake_pp"),
                Some(&json!([
                    {"code": 4_u32, "message": "outbound send failed"}
                ]))
            );
            assert_eq!(
                response.pointer("/ext/prebid/seatnonbid"),
                Some(&json!([
                    {"seat": "fake_pp", "nonbid": [{"impid": "imp-1", "statuscode": 100_u32}]}
                ]))
            );
            assert!(!response.to_string().contains("secret"));
        }
    }

    #[test]
    fn typed_unreachable_keeps_code999_status103() {
        let (response, parsed, dispatches) =
            auction_result(Outcome::BadGateway(BadGatewayReason::Unreachable), true);
        assert_eq!(parsed, 0);
        assert_eq!(dispatches, 1);
        assert_eq!(
            response.pointer("/ext/errors/fake_pp/0/code"),
            Some(&json!(999_u32))
        );
        assert_eq!(
            response.pointer("/ext/prebid/seatnonbid"),
            Some(&json!([
                {"seat": "fake_pp", "nonbid": [{"impid": "imp-1", "statuscode": 103_u32}]}
            ]))
        );
        assert!(!response.to_string().contains("secret"));
    }
}
