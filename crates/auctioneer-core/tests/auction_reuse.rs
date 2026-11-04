//! Direct exchange execution and the real HTTP handler reuse the same auction.
//!
//! Frozen clocks, RNG and fanout responses prove deterministic bytes. A fake
//! alias selects an independently discovered, published passthrough implementation.

mod support;

#[cfg(test)]
mod config_funding_support;
use config_funding_support as config_funding;

#[cfg(test)]
use auctioneer_bidder::test_support::serving_catalog;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use crate::support::ctx_with_config;
    use crate::{config_funding, serving_catalog};
    use async_trait::async_trait;
    use auctioneer_bidder::BidderHttpResponse;
    use auctioneer_core::config::AuctioneerAppConfig;
    use auctioneer_core::config::budgets::resolve_budgets;
    use auctioneer_core::config::resolve::resolve_host;
    use auctioneer_core::handlers::{TestFanoutHandle, auction as auction_handler};
    use auctioneer_core::host_primitives::registry::{
        TestClockHandle, TestMonotonicClockHandle, TestRngHandle,
    };
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::{
        AccountSource, AuctionContext, AuctionDeadline, AuctionPlan, BidderCall, BidderCallResult,
        BidderFanout, FanoutInternalError, ResolvedAccount, resolve_aliases, run_auction,
    };
    use auctioneer_openrtb::ext::imp::ImpExt;
    use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, Imp};
    use auctioneer_openrtb::types::bid_response::BidResponse;
    use auctioneer_openrtb::types::field::JsonField;
    use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare};
    use auctioneer_privacy::gcl::Gcl;
    use auctioneer_privacy::gvl::GvlStore;
    use auctioneer_runtime::{Clock, FakeClock, Rng, SeededRng};
    use edgezero_core::body::Body;
    use edgezero_core::time::MonotonicClock;
    use futures::executor::block_on;
    use http::{Method, Request, Uri};
    use std::collections::{BTreeMap, BTreeSet};
    use std::ptr;
    use std::sync::Arc;
    use std::time::Duration;

    /// Impression cap wide enough that the fixtures below never reach
    /// it; the cap itself is exercised by the max-impressions tests in
    /// `auctioneer-openrtb`.
    const IMP_CAP_NOT_UNDER_TEST: PrepareLimits = PrepareLimits {
        max_impressions: 100,
        max_copied_header_bytes: 4_096,
    };

    struct CannedFanout {
        body: Vec<u8>,
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
                .map(|_| BidderCallResult {
                    result: Ok(BidderHttpResponse::new(200, vec![], self.body.clone())),
                    elapsed: Duration::from_millis(10),
                })
                .collect())
        }
    }

    fn host_config() -> AuctioneerAppConfig {
        toml::from_str(
            &toml::to_string(&serde_json::json!({
                "schema_version": 1_u64,
                "host": {
                    "aliases": {"fake_bidder": serving_catalog::passthrough_info().name},
                },
            }))
            .unwrap(),
        )
        .unwrap()
    }

    fn auction_request() -> BidRequest {
        let mut bidders = serde_json::Map::new();
        let metadata = serving_catalog::passthrough_info();
        let params = serving_catalog::published()
            .into_iter()
            .find(|row| ptr::eq(row.info, metadata))
            .unwrap()
            .params;
        bidders.insert("fake_bidder".into(), params);
        BidRequest {
            id: "auc-mock".into(),
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
        }
    }

    async fn drive_direct(fanout: &dyn BidderFanout) -> BidResponse {
        let registry = auctioneer_bidder::configured_registry().unwrap();
        // Use the same host + account config the handler wires so the
        // direct-path and handler-path responses match byte-for-byte.
        let app_config = host_config();
        let host = resolve_host(&app_config, auctioneer_config_data::host_default());
        let acct = auctioneer_config_data::account_default();
        let resolved = ResolvedAccount {
            id: "unknown".into(),
            source: AccountSource::DefaultFallback,
            config: acct,
        };
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let rng = SeededRng::new(42);
        let normalized = prepare(
            auction_request(),
            &RequestHeaders::default(),
            &IMP_CAP_NOT_UNDER_TEST,
        )
        .unwrap();
        let ctx = AuctionContext::with_request_start(
            clock.now_monotonic(),
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
        let disabled = BTreeSet::new();
        let aliases = resolve_aliases(&normalized, &registry, &disabled, &host.aliases).unwrap();
        let budgets = resolve_budgets(&app_config, &host, "", resolved.source, &registry, &aliases);
        let plan = AuctionPlan::new(aliases, BTreeSet::new(), budgets, BTreeMap::new());
        let mut audit = DisclosureAudit::default();
        run_auction(&normalized, &ctx, &plan, &mut audit)
            .await
            .unwrap()
    }

    #[test]
    fn canned_response_is_deterministic_via_direct_run_auction() {
        block_on(async {
            // Raw byte-string fixture: this is a wire body, a `&[u8]`/`Vec<u8>`.
            // `serde_json::json!` yields a `Value`, so converting would mean
            // `.to_string().into_bytes()` at the call site for no gain.
            let body = br#"{"id":"r","seatbid":[{"seat":"fake_bidder","bid":[{"id":"b1","impid":"imp-1","price":1.0,"w":300,"h":250}]}],"cur":"USD"}"#.to_vec();
            let fanout = CannedFanout { body };
            let a = drive_direct(&fanout).await;
            let b = drive_direct(&fanout).await;
            assert_eq!(
                serde_json::to_vec(&a).unwrap(),
                serde_json::to_vec(&b).unwrap(),
            );
        });
    }

    /// Trusted-server reuse: the handler path
    /// (through `EdgeZero` `#[action] auction`) MUST produce
    /// byte-identical JSON to a direct `run_auction` call given
    /// equivalent inputs.
    #[test]
    fn handler_matches_direct_run_auction_byte_for_byte() {
        config_funding::install();
        block_on(async {
            // Raw byte-string fixture: this is a wire body, a `&[u8]`/`Vec<u8>`.
            // `serde_json::json!` yields a `Value`, so converting would mean
            // `.to_string().into_bytes()` at the call site for no gain.
            let body_bytes = br#"{"id":"r","seatbid":[{"seat":"fake_bidder","bid":[{"id":"b1","impid":"imp-1","price":1.0,"w":300,"h":250}]}],"cur":"USD"}"#.to_vec();

            // Direct path — the reference.
            let direct = {
                let fanout = CannedFanout {
                    body: body_bytes.clone(),
                };
                drive_direct(&fanout).await
            };
            let direct_bytes = serde_json::to_vec(&direct).unwrap();

            // Handler path — same fixtures wired via typed extensions:
            //  - TestFanoutHandle: same CannedFanout
            //  - TestClockHandle and TestMonotonicClockHandle: frozen FakeClock
            //  - TestRngHandle:    same SeededRng(42)
            // Both paths therefore see frozen request timing, the
            // same auction_timestamp_ms, and the same rng draws.
            // Deliberately NO timer or strategy handle, and no adapter
            // installed any: an injected fanout drives itself, so the
            // handler must not reach those registry cells (they panic
            // when empty). This test is the guard for that ordering.
            let canned: Arc<dyn BidderFanout + Send + Sync> = Arc::new(CannedFanout {
                body: body_bytes.clone(),
            });
            let clock: Arc<dyn Clock + Send + Sync> =
                Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
            let monotonic_source = Arc::clone(&clock);
            let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));
            let raw_body = serde_json::to_vec(&auction_request()).unwrap();
            let mut request = Request::builder()
                .method(Method::POST)
                .uri("/openrtb2/auction".parse::<Uri>().unwrap())
                .header("content-type", "application/json")
                .body(Body::from_bytes(raw_body))
                .unwrap();
            request
                .extensions_mut()
                .insert(TestFanoutHandle(Arc::clone(&canned)));
            request.extensions_mut().insert(TestClockHandle(clock));
            request
                .extensions_mut()
                .insert(TestMonotonicClockHandle(MonotonicClock::new(move || {
                    monotonic_source.now_monotonic()
                })));
            request.extensions_mut().insert(TestRngHandle(rng));
            let ctx = ctx_with_config(&toml::to_string(&host_config()).unwrap(), request);
            let response = auction_handler(ctx).await.expect("action returns Ok");
            let (_parts, body) = response.into_parts();
            let handler_bytes = body.into_bytes_bounded(0x0010_0000).await.unwrap();

            assert_eq!(
                handler_bytes.as_ref(),
                direct_bytes.as_slice(),
                "handler path must produce the same response as direct run_auction (Section 5.8)",
            );
        });
    }
}
