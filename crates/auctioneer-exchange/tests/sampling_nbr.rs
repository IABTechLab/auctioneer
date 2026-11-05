//! Response-builder integration test for the `sample_rate` load-control
//! sampler: a bidder sampled out on every imp must still surface as
//! `nbr = NoBidInvalidRequest` (wire code 7), not merely an empty
//! response with no signal at all -- the sentinel bundle the sampler
//! writes keeps the response builder's "no live bidders" branch
//! reachable per spec Appendix E.5.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

mod support;

#[cfg(test)]
mod tests {
    use crate::fake_bidder::FakeBidder;
    use crate::support::response_budgets;
    use async_trait::async_trait;
    use futures::executor::block_on;

    use auctioneer_bidder::{BidderName, BidderRegistry};
    use auctioneer_config_data::types::{AccountConfig, BidderInfo, HostConfig, ResourceLimits};
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::{
        AccountSource, AuctionContext, AuctionDeadline, AuctionPlan, BidderCall, BidderCallResult,
        BidderFanout, FanoutInternalError, ResolvedAccount, ResolvedAliases, run_auction,
    };
    use auctioneer_openrtb::ext::imp::ImpExt;
    use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, Imp};
    use auctioneer_openrtb::types::field::JsonField;
    use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare};

    const IMP_CAP_NOT_UNDER_TEST: PrepareLimits = PrepareLimits {
        max_impressions: 100,
        max_copied_header_bytes: 4_096,
    };
    use auctioneer_privacy::gcl::Gcl;
    use auctioneer_privacy::gvl::GvlStore;
    use auctioneer_runtime::{Clock as _, FakeClock, SeededRng};
    use std::collections::{BTreeMap, BTreeSet};

    /// The sampler drops the request's only bidder before fan-out, so
    /// this fanout must never actually be invoked (`calls.is_empty()`
    /// short-circuits ahead of it in `run_auction`); it exists only to
    /// satisfy `AuctionContext`'s borrow.
    struct UnreachableFanout;

    #[async_trait(?Send)]
    impl BidderFanout for UnreachableFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            assert!(
                calls.is_empty(),
                "a fully sampled-out request must never reach the fanout",
            );
            Ok(Vec::new())
        }
    }

    fn host() -> HostConfig {
        HostConfig {
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
        }
    }

    #[test]
    fn fully_sampled_out_bidder_yields_no_bid_invalid_request() {
        block_on(async {
            let mut registry = BidderRegistry::new();
            registry
                .register(
                    Box::leak(Box::new(BidderInfo {
                        name: "fake_aaa".into(),
                        endpoint: "https://demo.example.test/openrtb".into(),
                        uses_https: true,
                        ..BidderInfo::default()
                    })),
                    Box::new(FakeBidder::default()),
                )
                .expect("unique test bidder");

            let mut obj = serde_json::Map::new();
            obj.insert("fake_aaa".to_owned(), serde_json::json!({}));
            let req = BidRequest {
                id: "auc-sampled".into(),
                imp: vec![Imp {
                    id: "imp-1".into(),
                    banner: Some(Banner {
                        w: Some(300_i32),
                        h: Some(250_i32),
                        ..Banner::default()
                    }),
                    ext: Some(ImpExt {
                        bidder: JsonField::Value(serde_json::Value::Object(obj)),
                        ..ImpExt::default()
                    }),
                    ..Imp::default()
                }],
                cur: vec!["USD".into()],
                ..BidRequest::default()
            };
            let normalized = prepare(req, &RequestHeaders::default(), &IMP_CAP_NOT_UNDER_TEST)
                .expect("prepare succeeds on a minimal single-bidder request");

            let host = host();
            let acct = AccountConfig::default();
            let resolved = ResolvedAccount {
                id: "unknown".into(),
                source: AccountSource::DefaultFallback,
                config: &acct,
            };
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            let rng = SeededRng::new(42);
            let fanout = UnreachableFanout;

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
            sample_rates.insert(BidderName::new("fake_aaa"), 0.0_f64);
            let plan = AuctionPlan::new(
                ResolvedAliases::empty(),
                BTreeSet::new(),
                response_budgets(&registry, &host),
                sample_rates,
            );

            let mut audit = DisclosureAudit::default();
            let resp = run_auction(&normalized, &ctx, &plan, &mut audit)
                .await
                .expect("run_auction succeeds even when every bidder samples out");

            assert!(
                resp.seatbid.is_empty(),
                "no bidder dispatched -> no seatbid rows: {:?}",
                resp.seatbid,
            );
            assert_eq!(resp.cur, "", "no bid -> cur is the empty-string sentinel");
            assert_eq!(
                resp.nbr,
                Some(7_i32),
                "the sentinel bundle keeps the \"no live bidders\" branch reachable, \
                 so nbr = NoBidInvalidRequest (wire code 7) must fire per spec \
                 Appendix E.5",
            );
        });
    }
}
