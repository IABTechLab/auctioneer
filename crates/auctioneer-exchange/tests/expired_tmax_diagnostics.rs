//! Accepted `prebid` aliases retain bidder and auction-wide diagnostics.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

mod support;

#[cfg(test)]
mod tests {
    use crate::fake_bidder::FakeBidder;
    use crate::support::response_budgets;
    use async_trait::async_trait;
    use futures::executor::block_on;

    use auctioneer_bidder::{BidderHttpResponse, BidderName, BidderRegistry};
    use auctioneer_config_data::types::{AccountConfig, BidderInfo, HostConfig, ResourceLimits};
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::{
        AccountSource, AuctionContext, AuctionDeadline, AuctionPlan, BidderCall, BidderCallResult,
        BidderFanout, FanoutInternalError, ResolvedAccount, resolve_aliases, run_auction,
    };
    use auctioneer_openrtb::ext::imp::{ExtImpPrebid, ImpExt};
    use auctioneer_openrtb::ext::request_prebid::ExtRequestPrebid;
    use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, BidRequestExt, Imp};
    use auctioneer_openrtb::types::bid_response::{Bid, BidResponse, SeatBid};
    use auctioneer_openrtb::types::field::JsonField;
    use auctioneer_openrtb::types::regs::Regs;
    use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare};
    use auctioneer_privacy::gcl::Gcl;
    use auctioneer_privacy::gvl::GvlStore;
    use auctioneer_runtime::{Clock as _, FakeClock, SeededRng};
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::LazyLock;
    use std::time::Duration;

    static GENERIC: LazyLock<BidderInfo> = LazyLock::new(|| BidderInfo {
        name: "fake_bb".into(),
        endpoint: Some("https://demo.example.test/fake_bb".into()),
        uses_https: true,
        ..BidderInfo::default()
    });

    struct MustNotDispatch;

    #[async_trait(?Send)]
    impl BidderFanout for MustNotDispatch {
        async fn send_all(
            &self,
            _calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            panic!("an expired tmax must short-circuit before fanout");
        }
    }

    struct NonUsdReply;

    #[async_trait(?Send)]
    impl BidderFanout for NonUsdReply {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            assert_eq!(calls.len(), 1, "the accepted alias dispatches one call");
            let response = BidResponse {
                id: "non-usd-reply".into(),
                cur: "EUR".into(),
                seatbid: vec![SeatBid {
                    seat: "fake_bb".into(),
                    bid: vec![Bid {
                        id: "bid-1".into(),
                        impid: "imp-1".into(),
                        price: 1.0,
                        w: Some(300_i32),
                        h: Some(250_i32),
                        ..Bid::default()
                    }],
                    ..SeatBid::default()
                }],
                ..BidResponse::default()
            };
            Ok(vec![BidderCallResult {
                result: Ok(BidderHttpResponse::new(
                    200,
                    vec![],
                    serde_json::to_vec(&response).expect("typed reply serializes"),
                )),
                elapsed: Duration::from_millis(1),
            }])
        }
    }

    fn request_with_prebid_alias(aliases: BTreeMap<String, String>) -> BidRequest {
        BidRequest {
            id: "expired-prebid-alias".into(),
            tmax: Some(500),
            cur: vec!["USD".into()],
            regs: Some(Regs {
                gdpr: Some(0),
                ..Regs::default()
            }),
            imp: vec![Imp {
                id: "imp-1".into(),
                banner: Some(Banner {
                    w: Some(300),
                    h: Some(250),
                    ..Banner::default()
                }),
                ext: Some(ImpExt {
                    prebid: JsonField::Value(ExtImpPrebid {
                        bidder: BTreeMap::from([("prebid".into(), serde_json::json!({}))]),
                        ..ExtImpPrebid::default()
                    }),
                    ..ImpExt::default()
                }),
                ..Imp::default()
            }],
            ext: Some(BidRequestExt {
                prebid: Some(ExtRequestPrebid {
                    aliases,
                    returnallbidstatus: Some(true),
                    ..ExtRequestPrebid::default()
                }),
                ..BidRequestExt::default()
            }),
            ..BidRequest::default()
        }
    }

    fn alias_response(
        request: BidRequest,
        host_aliases: BTreeMap<String, String>,
        advance: Duration,
        fanout: &dyn BidderFanout,
    ) -> BidResponse {
        block_on(async {
            let mut registry = BidderRegistry::new();
            registry
                .register(&GENERIC, Box::new(FakeBidder::default()))
                .expect("unique test bidder");
            let host = HostConfig {
                aliases: host_aliases,
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
            let aliases = resolve_aliases(&request, &registry, &BTreeSet::new(), &host.aliases)
                .expect("prebid is an accepted alias");
            assert_eq!(
                aliases
                    .map
                    .bases
                    .get(&BidderName::new(RESERVED_PREBID_NAMESPACE)),
                Some(&BidderName::new("fake_bb"))
            );
            assert!(aliases.disabled_warnings.is_empty());
            assert!(aliases.dropped_alias_names.is_empty());
            let normalized = prepare(
                request,
                &RequestHeaders::default(),
                &PrepareLimits {
                    max_impressions: 100,
                    max_copied_header_bytes: 4_096,
                },
            )
            .expect("alias request prepares");
            let account = AccountConfig::default();
            let resolved = ResolvedAccount {
                id: "unknown".into(),
                source: AccountSource::DefaultFallback,
                config: &account,
            };
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            let request_start = clock.now_monotonic();
            clock.advance(advance);
            let rng = SeededRng::new(42);
            let ctx = AuctionContext::with_request_start(
                request_start,
                &host,
                &resolved,
                &registry,
                fanout,
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
                response_budgets(&registry, &host),
                BTreeMap::new(),
            );
            let mut audit = DisclosureAudit::default();
            let outcome = run_auction(&normalized, &ctx, &plan, &mut audit)
                .await
                .expect("alias auction returns diagnostics");
            assert_eq!(outcome.request_id(), normalized.id.as_str());
            assert_eq!(outcome.impression_ids(), ["imp-1"]);
            assert_eq!(outcome.winners().len(), 1);
            assert!(outcome.winners()[0].is_none());
            assert!(outcome.selected_bid(0).is_none());
            if advance >= Duration::from_millis(500) {
                let errors = &outcome.response_material().prebid_errors;
                assert_eq!(errors.len(), 1);
                assert_eq!(errors[0].code, 12);
                assert_eq!(errors[0].message, "tmax expired");
            }
            outcome.into_bid_response().unwrap()
        })
    }

    fn expired_alias_response(
        request_aliases: BTreeMap<String, String>,
        host_aliases: BTreeMap<String, String>,
    ) -> BidResponse {
        alias_response(
            request_with_prebid_alias(request_aliases),
            host_aliases,
            Duration::from_millis(501),
            &MustNotDispatch,
        )
    }

    fn assert_preserved_timeout_diagnostics(response: BidResponse) {
        assert_eq!(response.id, "expired-prebid-alias");
        assert!(response.seatbid.is_empty());
        assert_eq!(response.nbr, Some(7_i32));
        let ext = response.ext.expect("expired auction has ext");
        assert_eq!(
            ext["errors"],
            serde_json::json!({
                "prebid": [
                    {"code": 1_u32, "message": "timeout"},
                    {"code": 12_u32, "message": "tmax expired"},
                ],
            })
        );
        assert_eq!(
            ext.pointer("/prebid/seatnonbid"),
            Some(&serde_json::json!([{
                "seat": "prebid",
                "nonbid": [{"impid": "imp-1", "statuscode": 101_u32}],
            }]))
        );
        assert!(ext.get("warnings").is_none());
        assert!(ext.get("responsetimemillis").is_none());
    }

    fn assert_preserved_warning_diagnostics(
        request_aliases: BTreeMap<String, String>,
        host_aliases: BTreeMap<String, String>,
    ) {
        let mut request = request_with_prebid_alias(request_aliases);
        request.id = "non-usd-prebid-alias".into();
        // Present but empty gpp_sid warns without overriding gdpr = 0.
        request.regs.as_mut().expect("fixture has regs").gpp_sid = Some(vec![]);
        let response = alias_response(request, host_aliases, Duration::ZERO, &NonUsdReply);
        assert_eq!(response.id, "non-usd-prebid-alias");
        assert!(response.seatbid.is_empty(), "non-USD bids are dropped");
        assert_eq!(response.nbr, None, "the alias was dispatched");
        let ext = response.ext.expect("live auction has ext");
        assert_eq!(
            ext["warnings"],
            serde_json::json!({
                "prebid": [
                    {
                        "code": 10004_u32,
                        "message": "non-USD currency; bids dropped",
                    },
                    {
                        "code": 10001_u32,
                        "message": "regs.gdpr set together with regs.gpp_sid; gpp_sid presence disambiguates when non-empty, else regs.gdpr wins",
                    },
                ],
            }),
            "bidder warning precedes the auction-wide wire warning"
        );
        assert!(ext.get("errors").is_none(), "the reply parsed successfully");
        assert_eq!(
            ext["responsetimemillis"],
            serde_json::json!({"prebid": 1_u32}),
            "only the observable alias has dispatch timing"
        );
    }

    const RESERVED_PREBID_NAMESPACE: &str = "prebid";

    #[test]
    fn request_prebid_alias_keeps_timeout_before_expired_tmax_error() {
        let response = expired_alias_response(
            BTreeMap::from([("prebid".into(), "fake_bb".into())]),
            BTreeMap::new(),
        );
        assert_preserved_timeout_diagnostics(response);
    }

    #[test]
    fn host_prebid_alias_keeps_timeout_before_expired_tmax_error() {
        let response = expired_alias_response(
            BTreeMap::new(),
            BTreeMap::from([("prebid".into(), "fake_bb".into())]),
        );
        assert_preserved_timeout_diagnostics(response);
    }

    #[test]
    fn request_prebid_alias_keeps_non_usd_warning_before_wire_warning() {
        assert_preserved_warning_diagnostics(
            BTreeMap::from([("prebid".into(), "fake_bb".into())]),
            BTreeMap::new(),
        );
    }

    #[test]
    fn host_prebid_alias_keeps_non_usd_warning_before_wire_warning() {
        assert_preserved_warning_diagnostics(
            BTreeMap::new(),
            BTreeMap::from([("prebid".into(), "fake_bb".into())]),
        );
    }
}
