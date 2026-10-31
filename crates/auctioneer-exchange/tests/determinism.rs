//! Deterministic response-serialization test. Runs the same auction
//! twice with identical seeds + `FakeClock` + canned mock responses
//! and asserts byte-equal JSON serialization.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

mod support;

#[cfg(test)]
mod tests {
    use crate::fake_bidder::FakeBidder;
    use crate::support::response_budgets;
    use async_trait::async_trait;
    use futures::executor::block_on;

    use auctioneer_bidder::{BidderHttpResponse, BidderRegistry};
    use auctioneer_config::types::{AccountConfig, BidderInfo, HostConfig, ResourceLimits};
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::{
        AccountSource, AuctionContext, AuctionDeadline, AuctionPlan, BidderCall, BidderCallResult,
        BidderFanout, FanoutInternalError, ResolvedAccount, ResolvedAliases, run_auction,
    };
    use auctioneer_openrtb::ext::imp::ImpExt;
    use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, Imp};
    use auctioneer_openrtb::types::bid_response::BidResponse;
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
    use std::time::Duration;

    /// Canned 200 bodies, one per call position.
    ///
    /// The BODIES are stored rather than built responses: a
    /// `BidderHttpResponse` seals its body and is deliberately not
    /// `Clone`, so a fixture handing out one template per call would
    /// have to duplicate sealed state. The determinism property is
    /// unaffected — the same bytes still go to the same position on
    /// both runs, which is what this binary compares.
    struct CannedFanout {
        bodies: Vec<Vec<u8>>,
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
                .map(|(i, _)| BidderCallResult {
                    // Position `i`, else the first canned body, else an
                    // empty 204 — the same fallback chain as before,
                    // expressed over the bytes.
                    result: Ok(self
                        .bodies
                        .get(i)
                        .or_else(|| self.bodies.first())
                        .map_or_else(
                            || BidderHttpResponse::new(204, vec![], vec![]),
                            |body| BidderHttpResponse::new(200, vec![], body.clone()),
                        )),
                    elapsed: Duration::from_millis(50),
                })
                .collect())
        }
    }

    fn leak(name: &'static str, endpoint: &'static str) -> &'static BidderInfo {
        Box::leak(Box::new(BidderInfo {
            name: name.into(),
            endpoint: endpoint.into(),
            uses_https: true,
            ..BidderInfo::default()
        }))
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

    async fn drive() -> BidResponse {
        let mut registry = BidderRegistry::new();
        registry
            .register(
                leak("fake_aaa", "https://fake-a.example.test/openrtb2"),
                Box::new(FakeBidder::default()),
            )
            .expect("unique test bidder");
        registry
            .register(
                leak("fake_ccc", "https://hb.fake_ccc.example.test/openrtb"),
                Box::new(FakeBidder::default()),
            )
            .expect("unique test bidder");

        let mut bidders = serde_json::Map::new();
        bidders.insert("fake_aaa".into(), serde_json::json!({}));
        bidders.insert("fake_ccc".into(), serde_json::json!({}));
        let req = BidRequest {
            id: "auc-det-1".into(),
            imp: vec![Imp {
                id: "imp-1".into(),
                banner: Some(Banner {
                    w: Some(300_i32),
                    h: Some(250_i32),
                    ..Banner::default()
                }),
                ext: Some(ImpExt {
                    bidder: JsonField::Value(serde_json::Value::Object(bidders)),
                    ..ImpExt::default()
                }),
                ..Imp::default()
            }],
            cur: vec!["USD".into()],
            ..BidRequest::default()
        };
        let normalized = prepare(req, &RequestHeaders::default(), &IMP_CAP_NOT_UNDER_TEST).unwrap();
        let host = host();
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
        let body = br#"{"id":"r","seatbid":[{"seat":"fake_x","bid":[{"id":"b","impid":"imp-1","price":2.5,"w":300,"h":250}]}],"cur":"USD"}"#;
        let fanout = CannedFanout {
            bodies: vec![body.to_vec()],
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
        run_auction(&normalized, &ctx, &plan, &mut audit)
            .await
            .unwrap()
    }

    #[test]
    fn same_inputs_produce_byte_identical_responses() {
        let (a, b) = block_on(async { (drive().await, drive().await) });
        let bytes_a = serde_json::to_vec(&a).unwrap();
        let bytes_b = serde_json::to_vec(&b).unwrap();
        assert_eq!(bytes_a, bytes_b, "byte-stable response requirement");
    }

    #[test]
    fn seatbid_sorted_by_seat_lexicographic() {
        let resp = block_on(drive());
        let seats: Vec<&str> = resp.seatbid.iter().map(|s| s.seat.as_str()).collect();
        let mut sorted = seats.clone();
        sorted.sort_unstable();
        assert_eq!(seats, sorted, "seatbid must be sorted by seat");
    }
}
