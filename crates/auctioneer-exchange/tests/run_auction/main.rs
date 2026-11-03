//! `run_auction` end-to-end driver tests. Uses a canned in-memory
//! fanout to hold results stable and asserts on the returned
//! `BidResponse` envelope.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

use auctioneer_exchange::test_support as support;

#[cfg(test)]
mod tests {
    // Published owner captures use the host filesystem. The fake-based auction
    // controls below remain portable and are checked on every guest target.
    #[cfg(not(target_arch = "wasm32"))]
    mod provider_conformance;
    use crate::fake_bidder::FakeBidder;
    use crate::support::response_budgets;
    use async_trait::async_trait;
    use futures::executor::block_on;

    use auctioneer_bidder::{BidderHttpResponse, BidderName, BidderRegistry, DebugBodySchema};
    use auctioneer_config_data::types::{AccountConfig, BidderInfo, HostConfig, ResourceLimits};
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::{
        AccountSource, AuctionContext, AuctionDeadline, AuctionPlan, BidderCall, BidderCallResult,
        BidderFanout, FanoutInternalError, ResolvedAccount, ResolvedAliases, run_auction,
    };
    use auctioneer_openrtb::ext::imp::ImpExt;
    use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, Imp};
    use auctioneer_openrtb::types::bid_response::BidResponse;
    use auctioneer_openrtb::types::field::JsonField;
    use auctioneer_openrtb::types::inventory::Site;
    use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare};

    const IMP_CAP_NOT_UNDER_TEST: PrepareLimits = PrepareLimits {
        max_impressions: 100,
        max_copied_header_bytes: 4_096,
    };
    use auctioneer_privacy::gcl::Gcl;
    use auctioneer_privacy::gvl::GvlStore;
    use auctioneer_runtime::{Clock as _, FakeClock, SeededRng};
    use serde_json::{Value, json};
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::Duration;

    /// Canned 200 bodies, one per call position.
    ///
    /// The BODIES are stored rather than built responses, because a
    /// `BidderHttpResponse` seals its body and is deliberately not
    /// `Clone` — a fixture handing out one template per call would have
    /// to duplicate sealed state. Building a fresh response per call
    /// from the bytes is what the transport does anyway.
    struct CannedFanout {
        bodies: Vec<Vec<u8>>,
    }

    struct ObservedBidder(Arc<AtomicUsize>);

    impl auctioneer_bidder::Bidder for ObservedBidder {
        fn debug_body_schema(&self) -> DebugBodySchema {
            DebugBodySchema::Unsupported
        }

        fn make_requests(
            &self,
            request: &BidRequest,
            info: &auctioneer_bidder::BidderRequestInfo<'_>,
            sink: &mut auctioneer_bidder::OutboundRequestSink<'_>,
        ) -> Result<(), Vec<auctioneer_bidder::BidderError>> {
            auctioneer_bidder::Bidder::make_requests(&FakeBidder::default(), request, info, sink)
        }
        fn make_bids(
            &self,
            request: &BidRequest,
            sent: &auctioneer_bidder::OutboundRequestMeta,
            response: &BidderHttpResponse,
        ) -> Result<auctioneer_bidder::BidderResponse, Vec<auctioneer_bidder::BidderError>>
        {
            self.0.fetch_add(1, Ordering::SeqCst);
            auctioneer_bidder::Bidder::make_bids(&FakeBidder::default(), request, sent, response)
        }
    }

    #[async_trait(?Send)]
    impl BidderFanout for CannedFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            Ok(calls
                .iter()
                .enumerate()
                .map(|(i, _c)| BidderCallResult {
                    result: Ok(self.bodies.get(i).map_or_else(
                        || BidderHttpResponse::new(204, vec![], vec![]),
                        |body| BidderHttpResponse::new(200, vec![], body.clone()),
                    )),
                    elapsed: Duration::from_millis(50),
                })
                .collect())
        }
    }

    fn host_with_caps(default_ms: i64, max_ms: i64) -> HostConfig {
        HostConfig {
            resource_limits: ResourceLimits {
                default_tmax_ms: default_ms,
                max_tmax_ms: max_ms,
                max_bidder_response_bytes: 0x0004_0000,
                max_bidders_per_auction: 50,
                max_outbound_requests: 100,
                max_outbound_raw_bytes: 0x0004_0000,
                max_outbound_request_bytes: 0x0004_0000,
                max_transport_bytes: 0x0004_0000,
                max_decoded_bytes: 0x0004_0000,
                max_parsed_nodes: 0x0001_0000,
            },
            ..HostConfig::default()
        }
    }

    const RESPONSE_BIDDERS: &[&str] = &["fake_aaa", "fake_bb", "fake_ccc", "fake_bmmmmm"];

    fn auction_with_bidder_response(body: &[u8]) -> BidResponse {
        let mut registry = BidderRegistry::new();
        let endpoint = "https://bidder.example.test/openrtb";
        registry
            .register(
                leak_info("fake_bb", endpoint),
                Box::new(FakeBidder::default()),
            )
            .expect("unique test bidder");
        registry
            .register(
                leak_info("fake_aaa", endpoint),
                Box::new(FakeBidder::default()),
            )
            .expect("unique test bidder");
        registry
            .register(
                leak_info("fake_ccc", endpoint),
                Box::new(FakeBidder::default()),
            )
            .expect("unique test bidder");
        registry
            .register(
                leak_info("fake_bmmmmm", endpoint),
                Box::new(FakeBidder::default()),
            )
            .expect("unique test bidder");
        let params: serde_json::Map<String, Value> = RESPONSE_BIDDERS
            .iter()
            .map(|name| ((*name).to_owned(), json!({})))
            .collect();
        let request = prepare(
            serde_json::from_value(json!({
                "id": "auction-diagnostics", "site": {"id": "site"},
                "imp": [{"id": "imp-1", "banner": {"w": 300_i32, "h": 250_i32},
                    "ext": {"bidder": params}}]
            }))
            .unwrap(),
            &RequestHeaders::default(),
            &IMP_CAP_NOT_UNDER_TEST,
        )
        .unwrap();
        let host = host_with_caps(1000, 2000);
        let config = AccountConfig::default();
        let account = ResolvedAccount {
            id: "publisher".into(),
            source: AccountSource::DefaultFallback,
            config: &config,
        };
        let fanout = CannedFanout {
            bodies: vec![body.to_vec(); RESPONSE_BIDDERS.len()],
        };
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let rng = SeededRng::new(42);
        let ctx = AuctionContext::with_request_start(
            clock.now_monotonic(),
            &host,
            &account,
            &registry,
            &fanout,
            &clock,
            &rng,
            GvlStore::baked(),
            Gcl::baked(),
            clock.now(),
            false,
        );
        let plan = AuctionPlan::new(
            ResolvedAliases::empty(),
            BTreeSet::new(),
            response_budgets(ctx.bidders, ctx.host),
            BTreeMap::new(),
        );
        let mut audit = DisclosureAudit::default();
        let response = block_on(run_auction(&request, &ctx, &plan, &mut audit)).unwrap();
        let timings = response.ext.as_ref().unwrap()["responsetimemillis"]
            .as_object()
            .unwrap();
        assert_eq!(timings.len(), RESPONSE_BIDDERS.len());
        for name in RESPONSE_BIDDERS {
            assert!(timings.contains_key(*name), "{name} must reach dispatch");
        }
        response
    }

    fn bid_response_with_currency(currency: Option<&str>) -> Vec<u8> {
        let mut response = json!({
            "id": "r", "seatbid": [{"seat": "fake_s", "bid": [{
                "id": "b", "impid": "imp-1", "price": 1.5_f64, "w": 300_i32, "h": 250_i32
            }]}]
        });
        if let Some(value) = currency {
            response["cur"] = json!(value);
        }
        serde_json::to_vec(&response).unwrap()
    }

    #[test]
    fn bidder_response_parse_errors_do_not_echo_payloads() {
        for marker in [
            "RESPONSE_SECRET".to_owned(),
            "RESPONSE_SECRET".repeat(5000),
            "RESPONSE_SECRET\n\"\\\u{2603}".to_owned(),
        ] {
            let mut truncated = serde_json::to_vec(&json!({"id": "r", "secret": marker})).unwrap();
            truncated.pop();
            let bodies = [
                serde_json::to_vec(&json!({"id": "r", "seatbid": marker})).unwrap(),
                serde_json::to_vec(&json!({"id": "r", "seatbid": [{
                    "seat": "fake_s", "bid": [{"id": "b", "impid": "imp-1", "price": marker}]
                }]}))
                .unwrap(),
                truncated,
            ];
            for body in bodies {
                let response = auction_with_bidder_response(&body);
                assert!(response.seatbid.is_empty());
                assert!(
                    !serde_json::to_string(&response)
                        .unwrap()
                        .contains("RESPONSE_SECRET")
                );
                let ext = response.ext.as_ref().unwrap();
                for name in RESPONSE_BIDDERS {
                    assert_eq!(
                        ext["errors"][*name],
                        json!([{
                            "code": 14_u32, "message": "invalid bidder response"
                        }]),
                        "{name}"
                    );
                }
                assert!(ext.get("warnings").is_none());
            }
        }
    }

    #[test]
    fn unsupported_currency_warnings_do_not_echo_payloads() {
        for currency in [
            "EUR".to_owned(),
            "RESPONSE_SECRET".repeat(5000),
            "RESPONSE_SECRET\n\"\\\u{2603}".to_owned(),
        ] {
            let response =
                auction_with_bidder_response(&bid_response_with_currency(Some(&currency)));
            assert!(
                response.seatbid.is_empty(),
                "unsupported currency bids must stay dropped"
            );
            assert!(
                !serde_json::to_string(&response)
                    .unwrap()
                    .contains(&currency)
            );
            let ext = response.ext.as_ref().unwrap();
            for name in RESPONSE_BIDDERS {
                assert_eq!(
                    ext["warnings"][*name],
                    json!([{
                        "code": 10_004_u32, "message": "non-USD currency; bids dropped"
                    }]),
                    "{name}"
                );
            }
            assert!(ext.get("errors").is_none());
        }
    }

    #[test]
    fn default_or_usd_currency_keeps_bids_without_response_diagnostics() {
        for currency in [None, Some(""), Some("USD")] {
            let response = auction_with_bidder_response(&bid_response_with_currency(currency));
            assert_eq!(response.cur, "USD");
            assert_eq!(response.seatbid.len(), RESPONSE_BIDDERS.len());
            for seat in &response.seatbid {
                assert_eq!(seat.bid.len(), 1);
                assert_eq!(seat.bid[0].id, "b");
            }
            let ext = response.ext.as_ref().unwrap();
            assert!(ext.get("errors").is_none());
            assert!(ext.get("warnings").is_none());
        }
    }

    #[test]
    fn projection_refuses_an_insufficient_or_wrong_phase_receipt_before_fanout() {
        let mut registry = BidderRegistry::new();
        registry
            .register(
                leak_info("fake_bb", "https://demo.example.test/openrtb"),
                Box::new(FakeBidder::default()),
            )
            .expect("unique test bidder");
        let request = prepare(
            BidRequest {
                id: "projection".into(),
                imp: vec![imp_with_bidder("i", "fake_bb")],
                site: Some(Site::default()),
                ..BidRequest::default()
            },
            &RequestHeaders::default(),
            &IMP_CAP_NOT_UNDER_TEST,
        )
        .unwrap();
        let host = host_with_caps(1000, 2000);
        let config = AccountConfig::default();
        let account = ResolvedAccount {
            id: "a".into(),
            source: AccountSource::DefaultFallback,
            config: &config,
        };
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let rng = SeededRng::new(42);
        let fanout = CannedFanout { bodies: vec![] };
        let disabled = BTreeSet::new();
        let aliases =
            auctioneer_exchange::resolve_aliases(&request, &registry, &disabled, &BTreeMap::new())
                .unwrap();
        let plan = AuctionPlan::new(
            aliases,
            disabled,
            response_budgets(&registry, &host),
            BTreeMap::new(),
        );
        for (kind, bytes) in [
            (auctioneer_runtime::ChargeKind::Projection, 0),
            (auctioneer_runtime::ChargeKind::Privacy, 1 << 30_u8),
        ] {
            let budget = auctioneer_runtime::AuctionBudget::new(bytes);
            let receipt = budget.try_reserve(kind, bytes).unwrap();
            let mut ctx = AuctionContext::with_request_start(
                clock.now_monotonic(),
                &host,
                &account,
                &registry,
                &fanout,
                &clock,
                &rng,
                GvlStore::baked(),
                Gcl::baked(),
                clock.now(),
                false,
            );
            ctx.projection_reservation = Some(&receipt);
            let mut audit = DisclosureAudit::default();
            assert!(matches!(
                block_on(run_auction(&request, &ctx, &plan, &mut audit)),
                Err(auctioneer_exchange::AuctionError::Resource(
                    auctioneer_runtime::ResourceError::Exhausted {
                        kind: auctioneer_runtime::ChargeKind::Projection
                    }
                ))
            ));
        }
    }

    #[test]
    fn oversized_request_aliases_have_bounded_identity_refusals() {
        use auctioneer_config_data::types::MAX_BIDDER_NAME_BYTES;

        let mut registry = BidderRegistry::new();
        registry
            .register(
                leak_info("fake_bb", "https://demo.example.test/openrtb"),
                Box::new(FakeBidder::default()),
            )
            .expect("unique test bidder");
        for alias in [
            "a".repeat(MAX_BIDDER_NAME_BYTES + 1),
            "a".repeat(32 * 1024),
            "\u{00e9}".repeat(MAX_BIDDER_NAME_BYTES.wrapping_div(2) + 1),
        ] {
            let request = prepare(
                serde_json::from_value(json!({
                    "id": "identity-limit", "site": {"id": "s"},
                    "imp": [{"id": "i", "banner": {"w": 300_i32, "h": 250_i32},
                        "ext": {"bidder": {alias.clone(): {}}}}],
                    "ext": {"prebid": {"aliases": {alias.clone(): "fake_bb"}}}
                }))
                .unwrap(),
                &RequestHeaders::default(),
                &IMP_CAP_NOT_UNDER_TEST,
            )
            .unwrap();
            let error = auctioneer_exchange::resolve_aliases(
                &request,
                &registry,
                &BTreeSet::new(),
                &BTreeMap::new(),
            )
            .unwrap_err();
            assert_eq!(error.code(), 2);
            assert!(matches!(
                &error,
                auctioneer_exchange::AuctionError::BadInput(message)
                    if message == &format!(
                        "alias name exceeds the {MAX_BIDDER_NAME_BYTES}-byte bidder identity limit"
                    ) && !message.contains(&alias)
            ));
        }
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "phase refusal and parser-count controls share the same source-sized fixture"
    )]
    fn response_receipt_rejects_targeting_amplification_before_assembly() {
        use auctioneer_runtime::bounded_json::NODE_RETAINED_BYTES;
        use auctioneer_runtime::budget::{
            ASSEMBLY_FACTOR, HEADER_MAP_CHARGE, RESPONSE_ASSEMBLY_FACTOR, RESPONSE_PARSE_FACTOR,
        };
        use auctioneer_runtime::debug_budget::debug_peak_bytes;
        use auctioneer_runtime::{AuctionBudget, ChargeKind, ResourceError, serialized_json_bytes};
        use serde_json::json;
        for (boundary_alias, prefix_len, parses) in [
            (false, 32 * 1024, true),
            (true, 0, true),
            (false, 4000, false),
        ] {
            let name = if boundary_alias {
                "fake_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned()
            } else {
                "fake_bb".into()
            };
            let prefix = if boundary_alias {
                "hb".to_owned()
            } else {
                "p".repeat(prefix_len)
            };
            let mut registry = BidderRegistry::new();
            let parsed = Arc::new(AtomicUsize::new(0));
            registry
                .register(
                    leak_info("fake_bb", "https://demo.example.test/openrtb"),
                    Box::new(ObservedBidder(Arc::clone(&parsed))),
                )
                .expect("unique test bidder");
            let mut raw = json!({"id":"r", "site":{"id":"s"}, "imp":[{
                "id":"i", "banner":{"w":300_i32,"h":250_i32}, "ext":{"bidder":{name.clone():{}}}
            }], "ext":{"prebid":{"targeting":{"prefix":prefix}}}});
            if boundary_alias {
                raw["ext"]["prebid"]["aliases"] = json!({name.clone():"fake_bb"});
            }
            let req = prepare(
                serde_json::from_value(raw).unwrap(),
                &RequestHeaders::default(),
                &IMP_CAP_NOT_UNDER_TEST,
            )
            .unwrap();
            let aliases = auctioneer_exchange::resolve_aliases(
                &req,
                &registry,
                &BTreeSet::new(),
                &BTreeMap::new(),
            )
            .unwrap();
            let host = host_with_caps(1000, 2000);
            let plan = AuctionPlan::new(
                aliases,
                BTreeSet::new(),
                response_budgets(&registry, &host),
                BTreeMap::new(),
            );
            let account_config = AccountConfig::default();
            let account = ResolvedAccount {
                id: "a".into(),
                source: AccountSource::DefaultFallback,
                config: &account_config,
            };
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            let rng = SeededRng::new(42);
            let fanout = CannedFanout { bodies:vec![serde_json::to_vec(&json!({"id":"r","cur":"USD","seatbid":[{"seat":"fake_bb","bid":
                    (0_u32..32).map(|n| json!({"id":n.to_string(),"impid":"i","price":1_i32})).collect::<Vec<_>>()
                }]})).unwrap()] };
            let bidder_bytes = serialized_json_bytes(&name).unwrap();
            // Response ID/global fields and one candidate identity, each with
            // 16 nodes. Prefix/FPD bytes are not request-derived metadata.
            let metadata_bytes = u64::try_from(req.id.len()).unwrap()
                + 128
                + 2 * u64::try_from(name.len()).unwrap()
                + 128;
            let bookkeeping = 4 * (metadata_bytes + 32 * NODE_RETAINED_BYTES);
            let decoded = u64::try_from(fanout.bodies[0].len()).unwrap()
                * (1 + ASSEMBLY_FACTOR + RESPONSE_PARSE_FACTOR + RESPONSE_ASSEMBLY_FACTOR)
                + HEADER_MAP_CHARGE;
            let targeting = 32
                * (serialized_json_bytes(&prefix).unwrap() + bidder_bytes + 64)
                * RESPONSE_ASSEMBLY_FACTOR;
            // Exact pre-targeting funding reaches make_bids; one byte less
            // rejects decoded retention before the adapter parser is invoked.
            let debug = debug_peak_bytes(100).unwrap();
            let capacity = bookkeeping + decoded + debug - u64::from(!parses);
            assert!(capacity >= bookkeeping + debug);
            assert!(capacity < bookkeeping + decoded + debug + targeting);
            let budget = AuctionBudget::new(capacity);
            let receipt = budget
                .try_reserve(ChargeKind::Response, budget.limit())
                .unwrap();
            let mut ctx = AuctionContext::with_request_start(
                clock.now_monotonic(),
                &host,
                &account,
                &registry,
                &fanout,
                &clock,
                &rng,
                GvlStore::baked(),
                Gcl::baked(),
                clock.now(),
                false,
            );
            let mut control_audit = DisclosureAudit::default();
            let control = block_on(run_auction(&req, &ctx, &plan, &mut control_audit)).unwrap();
            assert_eq!(
                control
                    .seatbid
                    .iter()
                    .map(|seat| seat.bid.len())
                    .sum::<usize>(),
                32
            );
            parsed.store(0, Ordering::SeqCst);
            ctx.response_reservation = Some(&receipt);
            let mut audit = DisclosureAudit::default();
            assert!(
                matches!(
                    block_on(run_auction(&req, &ctx, &plan, &mut audit)),
                    Err(auctioneer_exchange::AuctionError::Resource(
                        ResourceError::Exhausted {
                            kind: ChargeKind::Response
                        }
                    ))
                ),
                "boundary_alias={boundary_alias}, prefix_len={prefix_len}"
            );
            assert_eq!(
                parsed.load(Ordering::SeqCst),
                usize::from(parses),
                "prefix_len={prefix_len}"
            );
        }
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "privacy visibility, expiry and receipt controls share the same denial population"
    )]
    fn response_receipt_covers_denial_rows_before_capping_and_expiry() {
        use auctioneer_runtime::bounded_json::NODE_RETAINED_BYTES;
        use auctioneer_runtime::debug_budget::debug_peak_bytes;
        use auctioneer_runtime::{AuctionBudget, ChargeKind, ResourceError};
        use serde_json::{Map, Value, json};
        let mut registry = BidderRegistry::new();
        registry
            .register(
                leak_info("fake_bb", "https://demo.example.test/openrtb"),
                Box::new(FakeBidder::default()),
            )
            .expect("unique test bidder");
        let bidders: Map<String, Value> = (0_u32..128)
            .map(|n| (format!("fake_a{n}"), json!({})))
            .collect();
        let aliases: Map<String, Value> = bidders
            .keys()
            .map(|name| (name.clone(), json!("fake_bb")))
            .collect();
        let mut config = AccountConfig::default();
        config.privacy.allowactivities =
            serde_json::from_value(json!({"fetchBids":{"default":false}})).unwrap();
        let account = ResolvedAccount {
            id: "a".into(),
            source: AccountSource::DefaultFallback,
            config: &config,
        };
        let mut host = host_with_caps(1000, 2000);
        host.resource_limits.max_bidders_per_auction = 1;
        let rng = SeededRng::new(42);
        let fanout = CannedFanout { bodies: vec![] };
        for expired in [false, true] {
            for visible in [false, true] {
                let raw = json!({"id":"r", "site":{"id":"s"}, "imp":[{
                    "id":"i".repeat(1024), "banner":{"w":300_i32,"h":250_i32}, "ext":{"bidder":bidders}
                }], "ext":{"prebid":{"aliases":aliases,"returnallbidstatus":visible}}});
                let req = prepare(
                    serde_json::from_value(raw).unwrap(),
                    &RequestHeaders::default(),
                    &IMP_CAP_NOT_UNDER_TEST,
                )
                .unwrap();
                let resolved = auctioneer_exchange::resolve_aliases(
                    &req,
                    &registry,
                    &BTreeSet::new(),
                    &BTreeMap::new(),
                )
                .unwrap();
                let plan = AuctionPlan::new(
                    resolved,
                    BTreeSet::new(),
                    response_budgets(&registry, &host),
                    BTreeMap::new(),
                );
                let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
                let start = clock.now_monotonic();
                if expired {
                    clock.advance(Duration::from_secs(3));
                }
                let identity_bytes = bidders
                    .keys()
                    .map(|name| 2 * u64::try_from(name.len()).unwrap() + 128)
                    .sum::<u64>();
                let count = u64::try_from(bidders.len()).unwrap();
                let nonbid_bytes = if visible {
                    count * (u64::try_from(req.imp[0].id.len()).unwrap() + 32)
                } else {
                    0
                };
                let metadata_bytes =
                    u64::try_from(req.id.len()).unwrap() + 128 + identity_bytes + nonbid_bytes;
                let metadata_nodes = 16 + count * (16 + 8 * u64::from(visible));
                let bookkeeping = 4 * (metadata_bytes + metadata_nodes * NODE_RETAINED_BYTES);
                let capacity = bookkeeping + debug_peak_bytes(100).unwrap();
                let budget = AuctionBudget::new(capacity);
                let full_receipt = budget.try_reserve(ChargeKind::Response, capacity).unwrap();
                let mut ctx = AuctionContext::with_request_start(
                    start,
                    &host,
                    &account,
                    &registry,
                    &fanout,
                    &clock,
                    &rng,
                    GvlStore::baked(),
                    Gcl::baked(),
                    clock.now(),
                    false,
                );
                ctx.response_reservation = Some(&full_receipt);
                let mut control_audit = DisclosureAudit::default();
                let control = block_on(run_auction(&req, &ctx, &plan, &mut control_audit)).unwrap();
                let ext = control.ext.as_ref().unwrap();
                assert_eq!(ext["warnings"].as_object().unwrap().len(), bidders.len());
                if visible {
                    assert_eq!(
                        ext["prebid"]["seatnonbid"].as_array().unwrap().len(),
                        bidders.len(),
                    );
                } else {
                    assert!(ext["prebid"].get("seatnonbid").is_none());
                }
                let short_budget = AuctionBudget::new(capacity.checked_sub(1).unwrap());
                let receipt = short_budget
                    .try_reserve(ChargeKind::Response, short_budget.limit())
                    .unwrap();
                ctx.response_reservation = Some(&receipt);
                let mut audit = DisclosureAudit::default();
                assert!(
                    matches!(
                        block_on(run_auction(&req, &ctx, &plan, &mut audit)),
                        Err(auctioneer_exchange::AuctionError::Resource(
                            ResourceError::Exhausted {
                                kind: ChargeKind::Response
                            }
                        ))
                    ),
                    "expired={expired}, visible={visible}"
                );
                drop(full_receipt);
                drop(receipt);
                assert_eq!(budget.in_use(), 0);
                assert_eq!(short_budget.in_use(), 0);
            }
        }
    }

    fn host_with_bidder_cap(cap: usize) -> HostConfig {
        HostConfig {
            resource_limits: ResourceLimits {
                default_tmax_ms: 750,
                max_tmax_ms: 1500,
                max_bidder_response_bytes: 0x0004_0000,
                max_bidders_per_auction: cap,
                max_outbound_requests: 100,
                max_outbound_raw_bytes: 0x0004_0000,
                max_outbound_request_bytes: 0x0004_0000,
                max_transport_bytes: 0x0004_0000,
                max_decoded_bytes: 0x0004_0000,
                max_parsed_nodes: 0x0001_0000,
            },
            ..HostConfig::default()
        }
    }

    fn leak_info(name: &'static str, endpoint: &'static str) -> &'static BidderInfo {
        Box::leak(Box::new(BidderInfo {
            name: name.into(),
            endpoint: endpoint.into(),
            uses_https: true,
            ..BidderInfo::default()
        }))
    }

    fn imp_with_bidder(id: &str, bidder: &str) -> Imp {
        imp_with_bidders(id, &[bidder])
    }

    fn imp_with_bidders(id: &str, bidders: &[&str]) -> Imp {
        let mut obj = serde_json::Map::new();
        for bidder in bidders {
            obj.insert((*bidder).to_owned(), serde_json::json!({}));
        }
        Imp {
            id: id.into(),
            banner: Some(Banner {
                w: Some(300),
                h: Some(250),
                ..Banner::default()
            }),
            ext: Some(ImpExt {
                bidder: JsonField::Value(serde_json::Value::Object(obj)),
                ..ImpExt::default()
            }),
            ..Imp::default()
        }
    }

    #[test]
    fn missing_resolved_response_budget_is_an_internal_error() {
        block_on(async {
            let mut registry = BidderRegistry::new();
            registry
                .register(
                    leak_info("fake_bb", "https://demo.example.test/openrtb"),
                    Box::new(FakeBidder::default()),
                )
                .expect("unique test bidder");
            let normalized = prepare(
                BidRequest {
                    id: "missing-budget".to_owned(),
                    imp: vec![imp_with_bidder("imp-1", "fake_bb")],
                    ..BidRequest::default()
                },
                &RequestHeaders::default(),
                &IMP_CAP_NOT_UNDER_TEST,
            )
            .unwrap();
            let host = host_with_caps(750, 1500);
            let acct = AccountConfig::default();
            let resolved = ResolvedAccount {
                id: "unknown".to_owned(),
                source: AccountSource::DefaultFallback,
                config: &acct,
            };
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            let rng = SeededRng::new(42);
            let fanout = CannedFanout { bodies: Vec::new() };
            let ctx = AuctionContext::with_request_start(
                clock.now_monotonic(),
                &host,
                &resolved,
                &registry,
                &fanout,
                &clock,
                &rng,
                GvlStore::baked(),
                Gcl::baked(),
                clock.now(),
                false,
            );
            let plan = AuctionPlan::new(
                ResolvedAliases::empty(),
                BTreeSet::new(),
                BTreeMap::new(),
                BTreeMap::new(),
            );
            let mut audit = DisclosureAudit::default();
            let error = run_auction(&normalized, &ctx, &plan, &mut audit)
                .await
                .expect_err("missing resolution must not use a default cap");
            assert!(
                matches!(&error, auctioneer_exchange::AuctionError::Internal(message) if message == "Internal auction error")
            );
        });
    }

    #[test]
    fn run_auction_returns_bid_response_with_responsetime_and_currency() {
        block_on(async {
            let mut registry = BidderRegistry::new();
            registry
                .register(
                    leak_info("fake_bb", "https://demo.example.test/openrtb"),
                    Box::new(FakeBidder::default()),
                )
                .expect("unique test bidder");

            let req = BidRequest {
                id: "auc-1".into(),
                imp: vec![imp_with_bidder("imp-1", "fake_bb")],
                cur: vec!["USD".into()],
                ..BidRequest::default()
            };
            let headers = RequestHeaders::default();
            let normalized = prepare(req, &headers, &IMP_CAP_NOT_UNDER_TEST)
                .expect("prepare succeeds on minimal request");

            let host = host_with_caps(750, 1500);
            let acct = AccountConfig::default();
            let resolved = ResolvedAccount {
                id: "unknown".into(),
                source: AccountSource::DefaultFallback,
                config: &acct,
            };
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            let rng = SeededRng::new(42);
            // Raw byte-string fixture: this is a wire body, a `&[u8]`/`Vec<u8>`.
            // `serde_json::json!` yields a `Value`, so converting would mean
            // `.to_string().into_bytes()` at the call site for no gain.
            let response_body = br#"{"id":"r","seatbid":[{"seat":"fake_bb","bid":[{"id":"b1","impid":"imp-1","price":1.5,"w":300,"h":250}]}],"cur":"USD"}"#;
            let fanout = CannedFanout {
                bodies: vec![response_body.to_vec()],
            };

            let ctx = AuctionContext::with_request_start(
                clock.now_monotonic(),
                &host,
                &resolved,
                &registry,
                &fanout,
                &clock,
                &rng,
                GvlStore::baked(),
                Gcl::baked(),
                clock.now(),
                false,
            );
            let plan = AuctionPlan::new(
                ResolvedAliases::empty(),
                BTreeSet::new(),
                response_budgets(&registry, &host),
                BTreeMap::new(),
            );

            let mut audit = DisclosureAudit::default();
            let resp = run_auction(&normalized, &ctx, &plan, &mut audit)
                .await
                .expect("run_auction succeeds");
            assert_eq!(resp.id, "auc-1");
            assert_eq!(resp.cur, "USD");
            let raw = serde_json::to_string(resp.ext.as_ref().unwrap()).unwrap();
            assert!(raw.contains("\"responsetimemillis\""));
            assert!(raw.contains("\"tmaxrequest\""));
            assert!(!raw.contains("\"debug\""));
            assert_eq!(resp.seatbid.len(), 1);
            assert_eq!(resp.seatbid[0].seat, "fake_bb");
        });
    }

    #[test]
    fn run_auction_short_circuits_on_expired_budget() {
        block_on(async {
            let mut registry = BidderRegistry::new();
            registry
                .register(
                    leak_info("fake_bb", "https://demo.example.test/openrtb"),
                    Box::new(FakeBidder::default()),
                )
                .expect("unique test bidder");
            let req = BidRequest {
                id: "auc-2".into(),
                imp: vec![imp_with_bidder("imp-1", "fake_bb")],
                cur: vec!["USD".into()],
                tmax: Some(0),
                ..BidRequest::default()
            };
            let normalized =
                prepare(req, &RequestHeaders::default(), &IMP_CAP_NOT_UNDER_TEST).unwrap();
            let host = host_with_caps(1, 1);
            let acct = AccountConfig::default();
            let resolved = ResolvedAccount {
                id: "unknown".into(),
                source: AccountSource::DefaultFallback,
                config: &acct,
            };
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            let rng = SeededRng::new(42);
            let start = clock.now_monotonic();
            clock.advance(Duration::from_millis(5));
            let fanout = CannedFanout { bodies: vec![] };
            let ctx = AuctionContext::with_request_start(
                start,
                &host,
                &resolved,
                &registry,
                &fanout,
                &clock,
                &rng,
                GvlStore::baked(),
                Gcl::baked(),
                clock.now(),
                false,
            );
            let plan = AuctionPlan::new(
                ResolvedAliases::empty(),
                BTreeSet::new(),
                response_budgets(&registry, &host),
                BTreeMap::new(),
            );
            let mut audit = DisclosureAudit::default();
            let resp = run_auction(&normalized, &ctx, &plan, &mut audit)
                .await
                .unwrap();
            let raw = serde_json::to_string(resp.ext.as_ref().unwrap()).unwrap();
            assert!(raw.contains("\"prebid\""));
            assert!(raw.contains("\"code\":12") || raw.contains("\"code\": 12"));
        });
    }

    /// Round-trip regression for the sampler-then-cap ordering: with
    /// `max_bidders_per_auction = 1` and two bidders on one imp, the
    /// split's `BTreeMap<BidderName, _>` iteration keeps `"fake_dropped_bidder"`
    /// ahead of `"fake_kept_bidder"` in registry order. If the cap ran BEFORE
    /// the sampler, it would keep `"fake_dropped_bidder"`'s slot and truncate
    /// `"fake_kept_bidder"` away before sampling ever got a look — so only
    /// running the sampler first lets `"fake_kept_bidder"` reach dispatch.
    #[test]
    fn sampled_out_bidder_frees_a_cap_slot_for_the_next_eligible_bidder() {
        block_on(async {
            let mut registry = BidderRegistry::new();
            registry
                .register(
                    leak_info("fake_dropped_bidder", "https://demo.example.test/dropped"),
                    Box::new(FakeBidder::default()),
                )
                .expect("unique test bidder");
            registry
                .register(
                    leak_info("fake_kept_bidder", "https://demo.example.test/kept"),
                    Box::new(FakeBidder::default()),
                )
                .expect("unique test bidder");

            let req = BidRequest {
                id: "auc-sample-cap".into(),
                imp: vec![imp_with_bidders(
                    "imp-1",
                    &["fake_dropped_bidder", "fake_kept_bidder"],
                )],
                cur: vec!["USD".into()],
                ..BidRequest::default()
            };
            let normalized =
                prepare(req, &RequestHeaders::default(), &IMP_CAP_NOT_UNDER_TEST).unwrap();

            let host = host_with_bidder_cap(1);
            let acct = AccountConfig::default();
            let resolved = ResolvedAccount {
                id: "unknown".into(),
                source: AccountSource::DefaultFallback,
                config: &acct,
            };
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            let rng = SeededRng::new(42);
            let fanout = CannedFanout { bodies: vec![] };

            let ctx = AuctionContext::with_request_start(
                clock.now_monotonic(),
                &host,
                &resolved,
                &registry,
                &fanout,
                &clock,
                &rng,
                GvlStore::baked(),
                Gcl::baked(),
                clock.now(),
                false,
            );
            let mut sample_rates: BTreeMap<BidderName, f64> = BTreeMap::new();
            sample_rates.insert(BidderName::new("fake_dropped_bidder"), 0.0_f64);
            let plan = AuctionPlan::new(
                ResolvedAliases::empty(),
                BTreeSet::new(),
                response_budgets(&registry, &host),
                sample_rates,
            );

            let mut audit = DisclosureAudit::default();
            let resp = run_auction(&normalized, &ctx, &plan, &mut audit)
                .await
                .expect("run_auction succeeds");
            let responsetimemillis = resp
                .ext
                .as_ref()
                .and_then(|ext| ext.get("responsetimemillis"))
                .and_then(serde_json::Value::as_object);
            assert!(
                responsetimemillis.is_some_and(|m| m.contains_key("fake_kept_bidder")),
                "fake_kept_bidder must occupy the cap slot the sampled-out bidder freed: {:?}",
                resp.ext,
            );
            assert!(
                !responsetimemillis.is_some_and(|m| m.contains_key("fake_dropped_bidder")),
                "fake_dropped_bidder was sampled out and must never dispatch: {:?}",
                resp.ext,
            );
        });
    }

    /// Run one auction on a `FakeClock` advanced by `advance` from
    /// `request_start`, and report whether the tmax-expired
    /// short-circuit fired. Asserts the exact deadline composition on
    /// the way through.
    fn expired_budget_short_circuits(advance: Duration) -> bool {
        block_on(async {
            const BUDGET_MS: u64 = 300;
            let mut registry = BidderRegistry::new();
            registry
                .register(
                    leak_info("fake_bb", "https://demo.example.test/fake_bb"),
                    Box::new(FakeBidder::default()),
                )
                .expect("unique test bidder");
            let req = BidRequest {
                id: "auc-deadline-instant".into(),
                imp: vec![imp_with_bidder("imp-1", "fake_bb")],
                cur: vec!["USD".into()],
                tmax: Some(i64::try_from(BUDGET_MS).expect("fits")),
                ..BidRequest::default()
            };
            let normalized = prepare(req, &RequestHeaders::default(), &IMP_CAP_NOT_UNDER_TEST)
                .expect("fixture prepares");
            let host = host_with_caps(750, 1500);
            let acct = AccountConfig::default();
            let resolved = ResolvedAccount {
                id: "unknown".into(),
                source: AccountSource::DefaultFallback,
                config: &acct,
            };
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            let rng = SeededRng::new(42);
            let request_start = clock.now_monotonic();

            // The deadline composes on the instant itself, exactly.
            let resolved_tmax =
                auctioneer_exchange::resolve_tmax(Some(BUDGET_MS), &host, request_start)
                    .expect("a bounded budget composes a representable deadline");
            assert_eq!(
                resolved_tmax.deadline.deadline_at,
                request_start
                    .checked_add(Duration::from_millis(BUDGET_MS))
                    .expect("no overflow"),
                "deadline_at is request_start + effective_budget on the instant, not a projection"
            );

            clock.advance(advance);
            let fanout = CannedFanout { bodies: vec![] };
            let ctx = AuctionContext::with_request_start(
                request_start,
                &host,
                &resolved,
                &registry,
                &fanout,
                &clock,
                &rng,
                GvlStore::baked(),
                Gcl::baked(),
                clock.now(),
                false,
            );
            let plan = AuctionPlan::new(
                ResolvedAliases::empty(),
                BTreeSet::new(),
                response_budgets(&registry, &host),
                BTreeMap::new(),
            );
            let mut audit = DisclosureAudit::default();
            let resp = run_auction(&normalized, &ctx, &plan, &mut audit)
                .await
                .expect("the auction answers");
            let raw = serde_json::to_string(&resp.ext).expect("ext serializes");
            raw.contains("\"code\":12")
        })
    }

    /// The auction deadline is composed directly ON the
    /// `request_start` instant — `request_start + effective_budget` —
    /// with no epoch, projection or conversion in between.
    ///
    /// The three clock positions pin the INCLUSIVE expiry rule the
    /// composed instant has to obey: a clock one millisecond short of
    /// the deadline still runs the auction, a clock exactly on it
    /// short-circuits, and so does a clock past it. Anything that
    /// re-derived the deadline from a projected duration would drift
    /// on at least one of the three.
    #[test]
    fn auction_deadline_composes_on_instant_without_projection() {
        assert!(
            expired_budget_short_circuits(Duration::from_millis(300)),
            "a clock exactly AT the deadline is expired: the rule is deadline_at <= now"
        );
        assert!(
            expired_budget_short_circuits(Duration::from_millis(301)),
            "a clock past the deadline is expired"
        );
        assert!(
            !expired_budget_short_circuits(Duration::from_millis(299)),
            "a clock one millisecond short of the deadline still runs the auction"
        );
    }
}
