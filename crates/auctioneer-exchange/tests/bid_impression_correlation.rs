//! Returned-bid admission across the split, outbound metadata, and targeting.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

mod support;

#[cfg(test)]
mod tests {
    use crate::fake_bidder::{FakeBidder, ResponseCoverage};
    use crate::support;
    use async_trait::async_trait;
    use futures::executor::block_on;

    use auctioneer_bidder::{
        Bidder, BidderError, BidderHttpResponse, BidderRegistry, BidderRequestInfo, BidderResponse,
        DebugBodySchema, OutboundRequestMeta, OutboundRequestSink,
    };
    use auctioneer_config_data::types::{AccountConfig, BidderInfo, HostConfig, ResourceLimits};
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::{
        AccountSource, AuctionContext, AuctionDeadline, AuctionPlan, BidderCall, BidderCallResult,
        BidderFanout, FanoutInternalError, ResolvedAccount, resolve_aliases, run_auction,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_openrtb::types::bid_response::BidResponse;
    use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare};
    use auctioneer_privacy::{gcl::Gcl, gvl::GvlStore};
    use auctioneer_runtime::{Clock as _, FakeClock, SeededRng};
    use serde_json::{Value, json};
    use std::collections::{BTreeMap, BTreeSet};
    use std::time::Duration;

    struct CorrelationBidder {
        /// None delegates to the shared fake; explicit rows exercise absent,
        /// per-call, or incorrectly enlarged metadata independently of its parser.
        call_ids: Option<Vec<Vec<&'static str>>>,
    }

    impl Bidder for CorrelationBidder {
        fn debug_body_schema(&self) -> DebugBodySchema {
            DebugBodySchema::Unsupported
        }

        fn make_requests(
            &self,
            request: &BidRequest,
            info: &BidderRequestInfo<'_>,
            sink: &mut OutboundRequestSink<'_>,
        ) -> Result<(), Vec<BidderError>> {
            let Some(rows) = &self.call_ids else {
                return FakeBidder::default().make_requests(request, info, sink);
            };
            for ids in rows {
                let mut scoped = request.clone();
                if !ids.is_empty() {
                    scoped.imp.retain(|imp| ids.contains(&imp.id.as_str()));
                }
                let _outcome = sink.push_request(
                    info,
                    |meta| {
                        meta.set_method("POST")?;
                        meta.set_url(info.endpoint_template)?;
                        for id in ids {
                            meta.imp_id(id)?;
                        }
                        Ok(())
                    },
                    |body| body.write_json(&scoped),
                );
            }
            Ok(())
        }

        fn make_bids(
            &self,
            request: &BidRequest,
            sent: &OutboundRequestMeta,
            response: &BidderHttpResponse,
        ) -> Result<BidderResponse, Vec<BidderError>> {
            // Return cross-call candidates so the exchange, not its fixture,
            // must enforce call-local metadata independently of scoped parsing.
            FakeBidder {
                response_coverage: ResponseCoverage::ScopedRequest,
                ..FakeBidder::default()
            }
            .make_bids(request, sent, response)
        }
    }

    struct CannedFanout(Vec<Vec<u8>>);

    #[async_trait(?Send)]
    impl BidderFanout for CannedFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            assert_eq!(calls.len(), self.0.len(), "every response answers one call");
            Ok(self
                .0
                .iter()
                .map(|body| BidderCallResult {
                    result: Ok(BidderHttpResponse::new(200, vec![], body.clone())),
                    elapsed: Duration::from_millis(1),
                })
                .collect())
        }
    }

    fn imp(id: &str, bidder: &str) -> Value {
        json!({"id": id, "banner": {"w": 300_i32, "h": 250_i32},
        "ext": {"prebid": {"bidder": {(bidder): {}}}}})
    }

    fn bid(id: &str, imp_id: &str, price: f64) -> Value {
        json!({"id": id, "impid": imp_id, "price": price, "w": 300_i32, "h": 250_i32})
    }

    fn info(name: &'static str) -> &'static BidderInfo {
        Box::leak(Box::new(BidderInfo {
            name: name.into(),
            endpoint: "https://bids.example.test/auction".into(),
            uses_https: true,
            ..BidderInfo::default()
        }))
    }

    fn drive(
        imps: &[Value],
        call_ids: Option<Vec<Vec<&'static str>>>,
        response_bids: Vec<Vec<Value>>,
        request_alias: bool,
        host_alias: bool,
    ) -> BidResponse {
        let mut registry = BidderRegistry::new();
        registry
            .register(info("fake_bb"), Box::new(CorrelationBidder { call_ids }))
            .expect("unique test bidder");
        registry
            .register(info("fake_other"), Box::new(FakeBidder::default()))
            .expect("unique test bidder");
        let request: BidRequest = serde_json::from_value(json!({
            "id": "correlation", "imp": imps, "regs": {"gdpr": 0_i32},
            "ext": {"prebid": {
                "aliases": if request_alias { json!({"fake_alias": "fake_bb"}) } else { json!({}) },
                "targeting": {"includewinners": true, "includebidderkeys": true}
            }}
        }))
        .unwrap();
        let aliases = resolve_aliases(
            &request,
            &registry,
            &BTreeSet::new(),
            &if host_alias {
                BTreeMap::from([("fake_alias".into(), "fake_bb".into())])
            } else {
                BTreeMap::new()
            },
        )
        .unwrap();
        let normalized = prepare(
            request,
            &RequestHeaders::default(),
            &PrepareLimits {
                max_impressions: 100,
                max_copied_header_bytes: 4096,
            },
        )
        .unwrap();
        let host = HostConfig {
            resource_limits: ResourceLimits {
                default_tmax_ms: 750,
                max_tmax_ms: 1500,
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
        };
        let account = AccountConfig::default();
        let resolved = ResolvedAccount {
            id: "publisher".into(),
            source: AccountSource::DefaultFallback,
            config: &account,
        };
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let rng = SeededRng::new(42);
        let fanout = CannedFanout(
            response_bids
                .into_iter()
                .map(|bids| {
                    serde_json::to_vec(&json!({
                    "id": "response", "cur": "USD", "seatbid": [{"seat": "fake_upstream", "bid": bids}]
                }))
                .unwrap()
                })
                .collect(),
        );
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
            aliases,
            BTreeSet::new(),
            support::response_budgets(&registry, &host),
            BTreeMap::new(),
        );
        let mut audit = DisclosureAudit::default();
        block_on(run_auction(&normalized, &ctx, &plan, &mut audit))
            .unwrap()
            .into_bid_response()
            .unwrap()
    }

    fn admitted_ids(response: &BidResponse, seat: &str) -> Vec<String> {
        response
            .seatbid
            .iter()
            .filter(|row| row.seat == seat)
            .flat_map(|row| &row.bid)
            .map(|bid| bid.id.clone())
            .collect()
    }

    #[test]
    fn foreign_and_unknown_impressions_are_dropped_before_winner_targeting() {
        let response = drive(
            &[imp("own", "fake_bb"), imp("foreign", "fake_other")],
            None,
            vec![
                vec![
                    bid("valid", "own", 1.0),
                    bid("foreign", "foreign", 99.0),
                    bid("unknown", "missing", 100.0),
                ],
                vec![bid("legitimate", "foreign", 2.0)],
            ],
            false,
            false,
        );
        assert_eq!(admitted_ids(&response, "fake_bb"), ["valid"]);
        let other = &response
            .seatbid
            .iter()
            .find(|s| s.seat == "fake_other")
            .unwrap()
            .bid[0];
        assert_eq!(
            other
                .ext
                .as_ref()
                .unwrap()
                .prebid
                .as_ref()
                .unwrap()
                .targeting["hb_bidder"],
            "fake_other"
        );
        let wire = serde_json::to_value(&response).unwrap();
        assert_eq!(wire["ext"]["errors"]["fake_bb"][0]["code"], 4_u32);
        assert_eq!(
            wire["ext"]["errors"]["fake_bb"].as_array().unwrap().len(),
            2,
            "each rejected bid contributes its own bounded diagnostic"
        );
        assert_eq!(wire["ext"]["errors"]["fake_bb"][1]["code"], 4_u32);
    }

    #[test]
    fn alias_seats_keep_their_own_impression_participation() {
        for (request_alias, host_alias) in [(true, false), (false, true)] {
            let response = drive(
                &[imp("own", "fake_alias"), imp("foreign", "fake_other")],
                None,
                vec![
                    vec![bid("valid", "own", 1.0), bid("foreign", "foreign", 99.0)],
                    vec![bid("fake_other", "foreign", 2.0)],
                ],
                request_alias,
                host_alias,
            );
            assert_eq!(admitted_ids(&response, "fake_alias"), ["valid"]);
            let wire = serde_json::to_value(&response).unwrap();
            assert_eq!(wire["ext"]["errors"]["fake_alias"][0]["code"], 4_u32);
            assert!(wire["ext"]["errors"].get("fake_bb").is_none());
        }
    }

    #[test]
    fn nonempty_metadata_rejects_bids_for_another_call_of_the_same_bidder() {
        let response = drive(
            &[imp("own-1", "fake_bb"), imp("own-2", "fake_bb")],
            Some(vec![vec!["own-1"], vec!["own-2"]]),
            vec![
                vec![bid("valid-1", "own-1", 1.0), bid("wrong-2", "own-2", 99.0)],
                vec![bid("valid-2", "own-2", 2.0), bid("wrong-1", "own-1", 99.0)],
            ],
            false,
            false,
        );
        assert_eq!(admitted_ids(&response, "fake_bb"), ["valid-1", "valid-2"]);
        let bids = &response.seatbid[0].bid;
        for (id, imp_id, price_bucket) in
            [("valid-1", "own-1", "1.00"), ("valid-2", "own-2", "2.00")]
        {
            let bid = bids.iter().find(|bid| bid.id == id).unwrap();
            assert_eq!(bid.impid, imp_id);
            let targeting = &bid.ext.as_ref().unwrap().prebid.as_ref().unwrap().targeting;
            assert_eq!(
                targeting.get("hb_bidder").map(String::as_str),
                Some("fake_bb")
            );
            assert_eq!(
                targeting.get("hb_pb").map(String::as_str),
                Some(price_bucket)
            );
        }
        let wire = serde_json::to_value(&response).unwrap();
        assert_eq!(
            wire["ext"]["errors"]["fake_bb"].as_array().unwrap().len(),
            2
        );
    }

    #[test]
    fn empty_metadata_uses_bidder_participation_not_the_entire_auction() {
        let response = drive(
            &[imp("own", "fake_bb"), imp("foreign", "fake_other")],
            Some(vec![vec![]]),
            vec![
                vec![bid("valid", "own", 1.0), bid("foreign", "foreign", 99.0)],
                vec![],
            ],
            false,
            false,
        );
        assert_eq!(admitted_ids(&response, "fake_bb"), ["valid"]);
    }

    #[test]
    fn empty_metadata_keeps_alias_and_base_impressions_separate() {
        let response = drive(
            &[imp("own", "fake_alias"), imp("foreign", "fake_bb")],
            Some(vec![vec![]]),
            vec![
                vec![
                    bid("alias-valid", "own", 1.0),
                    bid("alias-foreign", "foreign", 99.0),
                ],
                vec![
                    bid("base-valid", "foreign", 2.0),
                    bid("base-foreign", "own", 99.0),
                ],
            ],
            true,
            false,
        );
        assert_eq!(admitted_ids(&response, "fake_alias"), ["alias-valid"]);
        assert_eq!(admitted_ids(&response, "fake_bb"), ["base-valid"]);
    }

    #[test]
    fn metadata_cannot_enlarge_the_bidders_participation() {
        let response = drive(
            &[
                imp("own", "fake_bb"),
                imp("own-2", "fake_bb"),
                imp("foreign", "fake_other"),
            ],
            Some(vec![vec!["own", "foreign"]]),
            vec![
                vec![
                    bid("valid", "own", 1.0),
                    bid("foreign", "foreign", 99.0),
                    bid("unknown", "missing", 100.0),
                ],
                vec![],
            ],
            false,
            false,
        );
        assert_eq!(admitted_ids(&response, "fake_bb"), ["valid"]);
    }

    #[test]
    fn metadata_over_scoped_impression_count_is_rejected_before_dispatch() {
        let response = drive(
            &[imp("own", "fake_bb"), imp("foreign", "fake_other")],
            Some(vec![vec!["own", "foreign", "missing"]]),
            vec![vec![]],
            false,
            false,
        );
        assert!(admitted_ids(&response, "fake_bb").is_empty());
        let wire = serde_json::to_value(&response).unwrap();
        assert!(
            wire["ext"]["errors"]["fake_bb"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["code"] == 5_u32)
        );
    }

    #[test]
    fn rejected_impression_ids_are_not_echoed_in_diagnostics() {
        let unknown = "private-impression".repeat(4096);
        let response = drive(
            &[imp("own", "fake_bb")],
            None,
            vec![vec![bid("unknown", &unknown, 100.0)]],
            false,
            false,
        );
        assert!(response.seatbid.is_empty());
        let wire = serde_json::to_value(&response).unwrap();
        let error = &wire["ext"]["errors"]["fake_bb"][0];
        assert_eq!(error["code"], 4_u32);
        let message = error["message"].as_str().unwrap();
        assert!(message.len() <= 128);
        assert!(!message.contains("private-impression"));
    }
}
