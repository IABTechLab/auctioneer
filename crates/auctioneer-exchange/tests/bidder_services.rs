//! Portable service admission, independent of any real bidder implementation.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

mod support;

#[cfg(test)]
mod tests {
    use super::fake_bidder::FakeBidder;
    use super::support::response_budgets;
    use async_trait::async_trait;
    use auctioneer_bidder::{BidderHttpResponse, BidderRegistry, BidderRequestInfo};
    use auctioneer_config_data::types::{AccountConfig, BidderInfo};
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::{
        AccountSource, AuctionContext, AuctionDeadline, AuctionError, AuctionPlan, BidderCall,
        BidderCallResult, BidderFanout, FanoutInternalError, ResolvedAccount, resolve_aliases,
        run_auction,
    };
    use auctioneer_openrtb::{
        PrepareLimits, RequestHeaders, prepare,
        types::{bid_request::BidRequest, bid_response::BidResponse},
    };
    use auctioneer_privacy::{gcl::Gcl, gvl::GvlStore};
    use auctioneer_runtime::bidder_credentials::{
        BidderCredentialSource, BidderCredentials, CredentialUnavailable,
    };
    use auctioneer_runtime::{
        AuctionBudget, ChargeKind, Clock as _, FakeClock, MonotonicInstant, ResourceError,
        SeededRng,
    };
    use futures::{executor::block_on, task::noop_waker};
    use serde_json::{Value, json};
    use std::{
        collections::{BTreeMap, BTreeSet},
        future::{Future, pending},
        pin::pin,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        task::{Context, Poll},
        time::Duration,
    };

    const SPENDER: &str = "fake_0_spender";
    const PROTECTED: &str = "fake_1_credential";

    #[derive(Default)]
    struct Fanout(Mutex<Vec<String>>);

    #[async_trait(?Send)]
    impl BidderFanout for Fanout {
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

    struct Reader {
        calls: AtomicUsize,
        clock: Arc<FakeClock>,
        advance: bool,
        missing: bool,
        pending: bool,
        cancelled: AtomicUsize,
    }

    struct CancelGuard<'a>(&'a AtomicUsize);

    impl Drop for CancelGuard<'_> {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[async_trait(?Send)]
    impl BidderCredentialSource for Reader {
        async fn load(
            &self,
            reference: &str,
            deadline: MonotonicInstant,
        ) -> Result<BidderCredentials, CredentialUnavailable> {
            assert_eq!(reference, "fake_credentials");
            assert_eq!(
                deadline.saturating_duration_since(self.clock.now_monotonic()),
                Duration::from_secs(1)
            );
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.pending {
                let _guard = CancelGuard(&self.cancelled);
                pending::<()>().await;
            }
            if self.advance {
                self.clock.advance(Duration::from_secs(2));
            }
            if self.missing {
                return Err(CredentialUnavailable);
            }
            BidderCredentials::parse(
                &serde_json::to_vec(&json!({
                    "schema_version": 1_i32,
                    "username": "fake_user",
                    "password": "FAKE_SERVICE_PASSWORD",
                    "tracker": "fake_tracker"
                }))
                .unwrap(),
            )
        }
    }

    #[derive(Clone, Copy)]
    struct Scenario {
        denied: bool,
        invalid: bool,
        expired: bool,
        max_calls: usize,
        missing_store: bool,
        missing_value: bool,
        late_value: bool,
        funded: bool,
        projection_bytes: u64,
        resource_refusal: bool,
        pending: bool,
        protected: &'static str,
    }

    impl Default for Scenario {
        fn default() -> Self {
            Self {
                denied: false,
                invalid: false,
                expired: false,
                max_calls: 2,
                missing_store: false,
                missing_value: false,
                late_value: false,
                funded: true,
                projection_bytes: 16 << 20,
                resource_refusal: false,
                pending: false,
                protected: PROTECTED,
            }
        }
    }

    struct Outcome {
        reads: usize,
        constructed: usize,
        credentials_seen: usize,
        sent: Vec<String>,
        response: Value,
        cancelled: usize,
    }

    fn registry(
        scenario: Scenario,
        executions: &Arc<AtomicUsize>,
        saw_credentials: &Arc<AtomicUsize>,
    ) -> BidderRegistry {
        let constructed = Arc::clone(executions);
        let present = Arc::clone(saw_credentials);
        let observer = move |info: &BidderRequestInfo<'_>| {
            constructed.fetch_add(1, Ordering::SeqCst);
            if let Some(credentials) = info.credentials {
                assert_eq!(credentials.username(), "fake_user");
                assert_eq!(credentials.tracker(), "fake_tracker");
                present.fetch_add(1, Ordering::SeqCst);
            }
        };
        // Registry owns its boxed bidder, so the test observer outlives this drive.
        let callback = Box::leak(Box::new(observer));
        let preflight = Box::leak(Box::new(
            |request: &BidRequest, _info: &BidderRequestInfo<'_>| {
                assert_eq!(request.tmax, Some(1_000));
            },
        ));
        let mut registry = BidderRegistry::new();
        for name in [SPENDER, scenario.protected] {
            registry
                .register(
                    Box::leak(Box::new(BidderInfo {
                        name: name.into(),
                        endpoint: format!("https://bidder.example.test/{name}"),
                        uses_https: true,
                        ..BidderInfo::default()
                    })),
                    Box::new(FakeBidder {
                        credential_reference: (name == scenario.protected)
                            .then_some("fake_credentials"),
                        construction_observer: (name == scenario.protected).then_some(callback),
                        credential_preflight_observer: (name == scenario.protected)
                            .then_some(preflight),
                        semantic_error: name == scenario.protected && scenario.invalid,
                        ..FakeBidder::default()
                    }),
                )
                .expect("unique test bidder");
        }
        registry
    }

    fn complete<F: Future>(future: F, cancel: bool) -> Option<F::Output> {
        if cancel {
            let mut pinned = pin!(future);
            assert!(matches!(
                pinned
                    .as_mut()
                    .poll(&mut Context::from_waker(&noop_waker())),
                Poll::Pending
            ));
            None
        } else {
            Some(block_on(future))
        }
    }

    fn drive(scenario: Scenario) -> Outcome {
        let executions = Arc::new(AtomicUsize::new(0));
        let saw_credentials = Arc::new(AtomicUsize::new(0));
        let registry = registry(scenario, &executions, &saw_credentials);
        let request = prepare(
            serde_json::from_value(json!({
                "id": "fake-service-auction",
                "site": {"id": "fake_site"},
                "regs": {"gdpr": 0_i32},
                "imp": [{"id": "fake_imp", "banner": {"w": 300_i32, "h": 250_i32},
                    "ext": {"prebid": {"bidder": {(SPENDER): {}, (scenario.protected): {}}}}}]
            }))
            .unwrap(),
            &RequestHeaders::default(),
            &PrepareLimits::default(),
        )
        .unwrap();
        let mut host = auctioneer_config_data::host_default().clone();
        host.gdpr.enabled = false;
        host.resource_limits.default_tmax_ms = 1_000;
        host.resource_limits.max_tmax_ms = 1_000;
        host.resource_limits.max_outbound_requests = scenario.max_calls;
        let mut account = AccountConfig::default();
        if scenario.denied {
            account.privacy.allowactivities = serde_json::from_value(json!({
                "fetchBids": {"rules": [{"allow": false, "condition": {"componentName": [scenario.protected]}}]}
            })).unwrap();
        }
        let resolved = ResolvedAccount {
            id: "fake_account".into(),
            source: AccountSource::DefaultFallback,
            config: &account,
        };
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let start = clock.now_monotonic();
        if scenario.expired {
            clock.advance(Duration::from_secs(2));
        }
        let reader = Reader {
            calls: AtomicUsize::new(0),
            clock: Arc::clone(&clock),
            advance: scenario.late_value,
            missing: scenario.missing_value,
            pending: scenario.pending,
            cancelled: AtomicUsize::new(0),
        };
        let fanout = Fanout::default();
        let rng = SeededRng::new(42);
        let mut context = AuctionContext::with_request_start(
            start,
            &host,
            &resolved,
            &registry,
            &fanout,
            clock.as_ref(),
            &rng,
            GvlStore::baked(),
            Gcl::baked(),
            clock.now(),
            false,
        );
        context.credential_source = (!scenario.missing_store).then_some(&reader);
        // Functional admission tests: the separate resource controls own exact byte boundaries.
        let ledger = AuctionBudget::new(16 << 20);
        let funding = scenario.funded.then(|| {
            ledger
                .try_reserve(ChargeKind::Projection, scenario.projection_bytes)
                .unwrap()
        });
        context.projection_reservation = funding.as_ref();
        let plan = AuctionPlan::new(
            resolve_aliases(&request, &registry, &BTreeSet::new(), &host.aliases).unwrap(),
            BTreeSet::new(),
            response_budgets(&registry, &host),
            BTreeMap::new(),
        );
        let mut audit = DisclosureAudit::default();
        let result = complete(
            run_auction(&request, &context, &plan, &mut audit),
            scenario.pending,
        );
        let response = if let Some(completed) = result {
            response(
                completed.and_then(auctioneer_exchange::AuctionOutcome::into_bid_response),
                scenario.resource_refusal,
            )
        } else {
            Value::Null
        };
        Outcome {
            reads: reader.calls.load(Ordering::SeqCst),
            constructed: executions.load(Ordering::SeqCst),
            credentials_seen: saw_credentials.load(Ordering::SeqCst),
            cancelled: reader.cancelled.load(Ordering::SeqCst),
            sent: fanout.0.into_inner().unwrap(),
            response,
        }
    }

    fn response(result: Result<BidResponse, AuctionError>, resource_refusal: bool) -> Value {
        if resource_refusal {
            assert!(matches!(
                result,
                Err(AuctionError::Resource(ResourceError::Exhausted {
                    kind: ChargeKind::Projection
                }))
            ));
            Value::Null
        } else {
            serde_json::to_value(result.unwrap()).unwrap()
        }
    }

    #[test]
    fn dropping_a_suspended_auction_cancels_its_credential_read_without_dispatch() {
        let outcome = drive(Scenario {
            pending: true,
            protected: "fake_0_credential",
            ..Scenario::default()
        });
        assert_eq!(
            (outcome.reads, outcome.cancelled, outcome.constructed),
            (1, 1, 0)
        );
        assert!(outcome.sent.is_empty());
        assert_eq!(outcome.response, Value::Null);
    }

    #[test]
    fn failed_credential_read_refunds_the_only_permit_for_a_later_ordinary_bidder() {
        let outcome = drive(Scenario {
            protected: "fake_0_credential",
            max_calls: 1,
            missing_value: true,
            ..Scenario::default()
        });
        assert_eq!(
            (outcome.reads, outcome.constructed, outcome.cancelled),
            (1, 0, 0)
        );
        assert_eq!(
            outcome.sent,
            [format!("https://bidder.example.test/{SPENDER}")]
        );
        assert_eq!(
            outcome.response["ext"]["errors"]["fake_0_credential"][0]["code"],
            5_i32
        );
    }

    #[test]
    fn admitted_bidder_receives_one_coherent_credential_snapshot() {
        let outcome = drive(Scenario::default());
        assert_eq!(
            (outcome.reads, outcome.constructed, outcome.credentials_seen),
            (1, 1, 1)
        );
        assert_eq!(outcome.sent.len(), 2);
    }

    #[test]
    fn credential_work_is_funded_before_any_store_poll_or_sibling_construction() {
        // This covers the pre-existing two-bidder projection/diagnostic cost,
        // but not that cost plus the independent 128-KiB credential bank.
        let outcome = drive(Scenario {
            projection_bytes: 327_680,
            resource_refusal: true,
            ..Scenario::default()
        });
        assert_eq!(
            (outcome.reads, outcome.constructed, outcome.credentials_seen),
            (0, 0, 0)
        );
        assert!(outcome.sent.is_empty());
        assert_eq!(outcome.response, Value::Null);
    }

    #[test]
    fn privacy_and_parameter_refusals_each_precede_credential_reads() {
        for scenario in [
            Scenario {
                denied: true,
                ..Scenario::default()
            },
            Scenario {
                invalid: true,
                ..Scenario::default()
            },
        ] {
            let outcome = drive(scenario);
            assert_eq!((outcome.reads, outcome.constructed), (0, 0));
            assert_eq!(
                outcome.sent,
                [format!("https://bidder.example.test/{SPENDER}")]
            );
        }
    }

    #[test]
    fn exhausted_call_capacity_does_not_fetch_credentials_or_suppress_an_accepted_sibling() {
        let outcome = drive(Scenario {
            max_calls: 1,
            ..Scenario::default()
        });
        assert_eq!((outcome.reads, outcome.constructed), (0, 0));
        assert_eq!(
            outcome.sent,
            [format!("https://bidder.example.test/{SPENDER}")]
        );
    }

    #[test]
    fn expired_auction_never_reads_credentials_or_dispatches() {
        let outcome = drive(Scenario {
            expired: true,
            ..Scenario::default()
        });
        assert_eq!((outcome.reads, outcome.constructed), (0, 0));
        assert!(outcome.sent.is_empty());
    }

    #[test]
    fn missing_service_value_or_funding_refuses_only_the_credential_bidder() {
        for (scenario, expected_reads) in [
            (
                Scenario {
                    missing_store: true,
                    ..Scenario::default()
                },
                0,
            ),
            (
                Scenario {
                    missing_value: true,
                    ..Scenario::default()
                },
                1,
            ),
            (
                Scenario {
                    funded: false,
                    ..Scenario::default()
                },
                0,
            ),
        ] {
            let outcome = drive(scenario);
            assert_eq!((outcome.reads, outcome.constructed), (expected_reads, 0));
            assert_eq!(
                outcome.sent,
                [format!("https://bidder.example.test/{SPENDER}")]
            );
            assert_eq!(
                outcome.response["ext"]["errors"][PROTECTED][0]["code"],
                5_i32
            );
        }
    }

    #[test]
    fn a_late_credential_result_cannot_restart_the_auction_deadline() {
        let outcome = drive(Scenario {
            late_value: true,
            ..Scenario::default()
        });
        assert_eq!((outcome.reads, outcome.constructed), (1, 0));
        assert!(outcome.sent.is_empty());
        assert_eq!(
            outcome.response["ext"]["errors"]["prebid"][0]["code"],
            12_i32
        );
    }
}
