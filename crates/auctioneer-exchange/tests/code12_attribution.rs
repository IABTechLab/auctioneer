//! Code-12 (`tmax expired`) is attributed by the auction clock ALONE.
//!
//! A per-slot `Timeout(TimeoutCause::AuctionDeadline)` is *provenance* —
//! it names which composed budget was smaller when the call was
//! dispatched, not proof that the auction's own deadline has passed. A
//! best-effort target can report that cause for an earlier
//! platform-phase timeout while the auction clock still has budget
//! left. So the only admissible evidence for the auction-wide entry is
//! `clock.now_monotonic() >= deadline.deadline_at`.
//!
//! The per-bidder side is unchanged and asserted here too: any timeout,
//! whatever its cause, is still code 1 / `seatnonbid` statuscode 101.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

mod support;

#[cfg(test)]
mod tests {
    use crate::fake_bidder::FakeBidder;
    use crate::support::response_budgets;
    use async_trait::async_trait;
    use auctioneer_bidder::BidderName;
    use auctioneer_bidder::BidderRegistry;
    use futures::executor::block_on;

    use auctioneer_config_data::types::{AccountConfig, BidderInfo, HostConfig, ResourceLimits};
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::{
        AccountSource, AuctionContext, AuctionDeadline, AuctionPlan, BidderCall, BidderCallResult,
        BidderFanout, ExchangeFeatures, FanoutError, FanoutInternalError, ResolvedAccount,
        ResolvedAliases, TimeoutCause, run_auction,
    };
    use auctioneer_openrtb::ext::imp::ImpExt;
    use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, Imp};
    use auctioneer_openrtb::types::field::JsonField;
    use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare};
    use auctioneer_privacy::gcl::Gcl;
    use auctioneer_privacy::gvl::GvlStore;
    use auctioneer_runtime::{Clock as _, FakeClock, SeededRng};
    use std::collections::{BTreeMap, BTreeSet};
    use std::time::Duration;

    const IMP_CAP_NOT_UNDER_TEST: PrepareLimits = PrepareLimits {
        max_impressions: 100,
        max_copied_header_bytes: 4_096,
    };

    /// The auction budget every fixture here composes against.
    const BUDGET_MS: u64 = 500;

    /// A fanout that reports every call as
    /// `Timeout(TimeoutCause::AuctionDeadline)` without consuming any
    /// budget of its own. The clock position is set by the test before
    /// the auction runs, so the slot cause and the auction clock are
    /// two independent axes.
    struct AuctionDeadlineTimeoutFanout;

    #[async_trait(?Send)]
    impl BidderFanout for AuctionDeadlineTimeoutFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            Ok(calls
                .iter()
                .map(|_call| BidderCallResult {
                    result: Err(FanoutError::Timeout(TimeoutCause::AuctionDeadline)),
                    elapsed: Duration::from_millis(1),
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

    fn leak_info(name: &'static str, endpoint: &'static str) -> &'static BidderInfo {
        Box::leak(Box::new(BidderInfo {
            name: name.into(),
            endpoint: endpoint.into(),
            uses_https: true,
            ..BidderInfo::default()
        }))
    }

    fn imp_with_bidder(id: &str, bidder: &str) -> Imp {
        let mut obj = serde_json::Map::new();
        obj.insert(bidder.to_owned(), serde_json::json!({}));
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

    /// The serialized `ext` of one auction whose single bidder times out
    /// with `TimeoutCause::AuctionDeadline`, run with the clock moved
    /// `advance` past `request_start`.
    ///
    /// The budget is `BUDGET_MS`, so `advance < BUDGET_MS` leaves the
    /// auction clock live and `advance >= BUDGET_MS` expires it.
    fn auction_ext_with_clock_advanced_by(advance: Duration) -> serde_json::Value {
        block_on(async {
            let mut registry = BidderRegistry::new();
            registry
                .register(
                    leak_info("fake_bb", "https://demo.example.test/fake_bb"),
                    Box::new(FakeBidder::default()),
                )
                .expect("unique test bidder");
            let req = BidRequest {
                id: "auc-code12".into(),
                imp: vec![imp_with_bidder("imp-1", "fake_bb")],
                cur: vec!["USD".into()],
                tmax: Some(i64::try_from(BUDGET_MS).expect("budget fits in i64")),
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

            // Pin the axis the assertions rest on: the auction deadline
            // is `request_start + BUDGET_MS`, and the advance below is
            // measured against exactly that instant.
            let deadline_at =
                auctioneer_exchange::resolve_tmax(Some(BUDGET_MS), &host, request_start)
                    .expect("a bounded budget composes a representable deadline")
                    .deadline
                    .deadline_at;
            assert_eq!(
                deadline_at,
                request_start
                    .checked_add(Duration::from_millis(BUDGET_MS))
                    .expect("no overflow"),
                "deadline_at is request_start + budget"
            );

            clock.advance(advance);

            let fanout = AuctionDeadlineTimeoutFanout;
            // `seatnonbid` is emitted only under `returnallbidstatus`,
            // and the per-bidder statuscode is half of what this test
            // pins, so the fixture turns it on.
            let features = ExchangeFeatures {
                returnallbidstatus_default: true,
                ..ExchangeFeatures::default()
            };
            let no_timeout_overrides: BTreeMap<BidderName, u32> = BTreeMap::new();
            let ctx = AuctionContext::with_request_start_ext(
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
                features,
                &no_timeout_overrides,
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
                .expect("the auction answers")
                .into_bid_response()
                .unwrap();
            serde_json::to_value(resp.ext.as_ref().expect("ext is present"))
                .expect("ext serializes")
        })
    }

    /// Every `ext.errors.prebid` code in the response, in order.
    fn prebid_error_codes(ext: &serde_json::Value) -> Vec<i64> {
        ext.get("errors")
            .and_then(|errs| errs.get("prebid"))
            .and_then(serde_json::Value::as_array)
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|entry| entry.get("code").and_then(serde_json::Value::as_i64))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Every `ext.errors.<bidder>` code for one bidder, in order.
    fn bidder_error_codes(ext: &serde_json::Value, bidder: &str) -> Vec<i64> {
        ext.get("errors")
            .and_then(|errs| errs.get(bidder))
            .and_then(serde_json::Value::as_array)
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|entry| entry.get("code").and_then(serde_json::Value::as_i64))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Every `seatnonbid` statuscode reported for one bidder.
    fn seatnonbid_statuscodes(ext: &serde_json::Value, bidder: &str) -> Vec<i64> {
        ext.pointer("/prebid/seatnonbid")
            .and_then(serde_json::Value::as_array)
            .map(|seats| {
                seats
                    .iter()
                    .filter(|seat| {
                        seat.get("seat").and_then(serde_json::Value::as_str) == Some(bidder)
                    })
                    .filter_map(|seat| seat.get("nonbid").and_then(serde_json::Value::as_array))
                    .flatten()
                    .filter_map(|row| row.get("statuscode").and_then(serde_json::Value::as_i64))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The auction-wide code-12 entry follows the auction clock and
    /// nothing else.
    ///
    /// Both halves run the SAME slot outcome — one
    /// `Err(Timeout(AuctionDeadline))` — and differ only in where the
    /// clock sits relative to `deadline_at`. With the clock 100 ms
    /// short of the deadline the slot cause must not manufacture a
    /// code 12; with the clock past the deadline the entry appears
    /// exactly once. Either way the bidder keeps its code 1 /
    /// statuscode 101.
    #[test]
    fn code_12_requires_clock_expiry_not_slot_cause() {
        let live = auction_ext_with_clock_advanced_by(Duration::from_millis(BUDGET_MS - 100));
        assert!(
            !prebid_error_codes(&live).contains(&12),
            "a slot-reported AuctionDeadline cause is provenance, not clock expiry: {live}"
        );
        assert_eq!(
            bidder_error_codes(&live, "fake_bb"),
            vec![1],
            "the bidder still reports its own timeout as code 1: {live}"
        );
        assert_eq!(
            seatnonbid_statuscodes(&live, "fake_bb"),
            vec![101],
            "the bidder still reports seatnonbid statuscode 101: {live}"
        );

        let expired = auction_ext_with_clock_advanced_by(Duration::from_millis(BUDGET_MS + 100));
        assert_eq!(
            prebid_error_codes(&expired)
                .into_iter()
                .filter(|code| *code == 12)
                .count(),
            1,
            "an expired auction clock raises code 12 exactly once: {expired}"
        );
        assert_eq!(
            bidder_error_codes(&expired, "fake_bb"),
            vec![1],
            "the per-bidder entry is unchanged by the auction-wide code 12: {expired}"
        );
        assert_eq!(
            seatnonbid_statuscodes(&expired, "fake_bb"),
            vec![101],
            "the per-bidder statuscode is unchanged by the auction-wide code 12: {expired}"
        );
    }

    /// A batch-level deadline report reaching the exchange while the
    /// auction clock is still live is a BIDDER timeout, never the
    /// auction-wide code 12.
    ///
    /// This is the same substitution the platform makes on a
    /// best-effort target: whichever composed budget the platform
    /// measured against, what it hands back is one slot's outcome. The
    /// exchange grades that slot — code 1, statuscode 101 — and asks
    /// its own clock, which still has budget, so no `ext.errors.prebid`
    /// entry is emitted at all.
    #[test]
    fn batch_deadline_report_with_live_auction_is_bidder_timeout_not_code_12() {
        let ext = auction_ext_with_clock_advanced_by(Duration::from_millis(1));
        assert!(
            prebid_error_codes(&ext).is_empty(),
            "a live auction clock emits no auction-wide prebid error: {ext}"
        );
        assert_eq!(
            bidder_error_codes(&ext, "fake_bb"),
            vec![1],
            "the batch deadline report grades as a bidder timeout: {ext}"
        );
        assert_eq!(
            seatnonbid_statuscodes(&ext, "fake_bb"),
            vec![101],
            "the bidder's nonbid row is the timeout statuscode: {ext}"
        );
    }
}
