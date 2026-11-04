//! An auction budget that runs out mid-fanout has to reach the wire as
//! the global `ext.errors.prebid` code 12, not only as the per-bidder
//! code 1 — and the auction CLOCK is the only thing that decides it.
//! A slot's own `TimeoutCause` is provenance about which budget was
//! smaller at dispatch, so it grades the per-bidder surface and stops
//! there.
//!
//! The timeout is injected at the FANOUT layer, which is the only layer
//! that can mint a `TimeoutCause`: `ProxyClientApi::send_one` returns a
//! `ProxyError`, so no test proxy client could produce one. Every row
//! therefore drives the real auction handler with a `TestFanoutHandle`
//! and reads the serialized `BidResponse` back.

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
    use crate::config_funding;
    use crate::serving_catalog;
    use crate::support::ctx_with_config;
    use async_trait::async_trait;
    use auctioneer_core::handlers::{TestFanoutHandle, auction};
    use auctioneer_core::host_primitives::{
        TestClockHandle, TestMonotonicClockHandle, TestRngHandle,
    };
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutError,
        FanoutInternalError, TimeoutCause,
    };
    use auctioneer_openrtb::ext::imp::ImpExt;
    use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, Imp};
    use auctioneer_openrtb::types::bid_response::BidResponse;
    use auctioneer_openrtb::types::field::JsonField;
    use auctioneer_runtime::{Clock, FakeClock, Rng, SeededRng};
    use edgezero_core::body::Body;
    use edgezero_core::http::StatusCode;
    use edgezero_core::time::MonotonicClock;
    use futures::executor::block_on;
    use http::{Method, Request, Uri};
    use std::sync::Arc;
    use std::time::Duration;
    use time::OffsetDateTime;

    /// Pinned on the fixture instead of inherited from the host default,
    /// so the auction budget is a known span on the `FakeClock` axis and
    /// each row controls exactly how much of it is spent.
    const AUCTION_TMAX_MS: i64 = 1000;

    /// The same budget as [`AUCTION_TMAX_MS`], in the unit the clock
    /// speaks. Spending exactly this much lands the clock ON the
    /// deadline rather than past it.
    const AUCTION_BUDGET: Duration = Duration::from_secs(1);

    /// Response cap wide enough that nothing here trips it.
    const WIDE_CAP: usize = 1 << 20;

    /// Every call lost its race to the ABSOLUTE auction deadline. This
    /// is what `RaceTimersStrategy` returns when the deadline arm wins.
    struct AuctionDeadlineFanout;

    #[async_trait(?Send)]
    impl BidderFanout for AuctionDeadlineFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            Ok(calls
                .iter()
                .map(|_| BidderCallResult {
                    result: Err(FanoutError::Timeout(TimeoutCause::AuctionDeadline)),
                    elapsed: Duration::from_millis(50),
                })
                .collect())
        }
    }

    /// Every call reported its OWN per-bidder budget, and the auction
    /// budget ran out on the same tick. No per-call cause names the
    /// auction here — only the post-fanout clock re-read can.
    struct BidderTimeoutFanout {
        clock: Arc<FakeClock>,
        spend: Duration,
    }

    #[async_trait(?Send)]
    impl BidderFanout for BidderTimeoutFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            self.clock.advance(self.spend);
            Ok(calls
                .iter()
                .map(|_| BidderCallResult {
                    result: Err(FanoutError::Timeout(TimeoutCause::Bidder)),
                    elapsed: self.spend,
                })
                .collect())
        }
    }

    /// Single-imp banner request naming a real-backed `fake_` alias, with
    /// an explicit budget so the auction never short-circuits on the
    /// PRE-fanout expiry path — that path already emits code 12 and
    /// would make either row pass for the wrong reason.
    fn fixture_request() -> BidRequest {
        let mut bidders = serde_json::Map::new();
        bidders.insert("fake_bb".to_owned(), serde_json::json!({}));
        BidRequest {
            id: "auc-mid-fanout".to_owned(),
            tmax: Some(AUCTION_TMAX_MS),
            imp: vec![Imp {
                id: "imp-1".to_owned(),
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
            cur: vec!["USD".to_owned()],
            ..BidRequest::default()
        }
    }

    /// POST the fixture through the real auction handler with `fanout`
    /// installed, and hand back the deserialized `BidResponse`.
    fn drive_handler(
        fanout: Arc<dyn BidderFanout + Send + Sync>,
        clock: &Arc<FakeClock>,
    ) -> BidResponse {
        let config = format!(
            "schema_version = 1\n[host.aliases]\nfake_bb={:?}\n",
            serving_catalog::passthrough_info().name,
        );
        config_funding::install();
        let handler_clock: Arc<dyn Clock + Send + Sync> = Arc::<FakeClock>::clone(clock);
        let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));
        let serialized = serde_json::to_vec(&fixture_request()).expect("fixture serializes");
        let mut request = Request::builder()
            .method(Method::POST)
            .uri(Uri::from_static("/openrtb2/auction"))
            .header("content-type", "application/json")
            .body(Body::from_bytes(serialized))
            .expect("request builds");
        request.extensions_mut().insert(TestFanoutHandle(fanout));
        request
            .extensions_mut()
            .insert(TestClockHandle(handler_clock));
        // The same `FakeClock` drives MONOTONIC time too, so
        // `request_start`, the composed deadline and the post-fanout
        // recheck all move when the fanout advances the clock.
        let monotonic_source = Arc::<FakeClock>::clone(clock);
        request
            .extensions_mut()
            .insert(TestMonotonicClockHandle(MonotonicClock::new(move || {
                monotonic_source.now_monotonic()
            })));
        request.extensions_mut().insert(TestRngHandle(rng));

        block_on(async {
            let response = auction(ctx_with_config(&config, request))
                .await
                .expect("the handler answers");
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "a timed-out auction is still a 200 with an error block",
            );
            let (_parts, body) = response.into_parts();
            let bytes = body
                .into_bytes_bounded(WIDE_CAP)
                .await
                .expect("response body reads");
            serde_json::from_slice(&bytes).expect("the response deserializes as a BidResponse")
        })
    }

    fn ext_of(response: &BidResponse) -> serde_json::Value {
        response.ext.clone().unwrap_or(serde_json::Value::Null)
    }

    /// A slot's `TimeoutCause::AuctionDeadline` is PROVENANCE, not
    /// proof: it names which composed budget was smaller when the call
    /// was dispatched, and a best-effort target can report it for a
    /// timeout its platform enforced in an earlier phase while this
    /// auction's clock still has budget. So the cause alone writes only
    /// the per-bidder surface; the global code 12 belongs to the
    /// auction clock, which has not moved here.
    #[test]
    fn auction_deadline_cause_alone_raises_no_global_code_12_error() {
        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let response = drive_handler(Arc::new(AuctionDeadlineFanout), &clock);
        let ext = ext_of(&response);
        assert_eq!(
            ext.pointer("/errors/fake_bb"),
            Some(&serde_json::json!([{ "code": 1_u32, "message": "bidder timeout" }])),
            "the per-bidder surface still reports code 1, which proves the call was dispatched \
             and its result folded in; got ext {ext}",
        );
        assert_eq!(
            ext.pointer("/errors/prebid"),
            None,
            "the auction clock never reached the deadline, so the slot's cause must not \
             manufacture a global error; got ext {ext}",
        );
    }

    /// Belt and braces: the budget elapsed exactly as the per-bidder
    /// timeouts fired, so no result carries an `AuctionDeadline` cause.
    /// The post-fanout clock re-read is the only thing that can raise
    /// code 12 here.
    #[test]
    fn deadline_elapsed_during_fanout_raises_code_12_without_an_auction_deadline_cause() {
        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let fanout = Arc::new(BidderTimeoutFanout {
            clock: Arc::<FakeClock>::clone(&clock),
            // Exactly the pinned budget: the deadline is reached, not
            // overshot, so the check has to treat "at the deadline" as
            // expired.
            spend: AUCTION_BUDGET,
        });
        let response = drive_handler(fanout, &clock);
        let ext = ext_of(&response);
        assert_eq!(
            ext.pointer("/errors/fake_bb"),
            Some(&serde_json::json!([{ "code": 1_u32, "message": "bidder timeout" }])),
            "the per-bidder rows are plain bidder timeouts, got ext {ext}",
        );
        assert_eq!(
            ext.pointer("/errors/prebid"),
            Some(&serde_json::json!([{ "code": 12_u32, "message": "tmax expired" }])),
            "the post-fanout deadline check must raise code 12 on its own, got ext {ext}",
        );
    }

    /// The mirror image: a fanout that finishes inside the budget with
    /// an ordinary per-bidder timeout must NOT raise the global error.
    /// Without this row a check that always fired would still pass the
    /// two rows above.
    #[test]
    fn a_bidder_timeout_inside_the_budget_raises_no_global_error() {
        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let fanout = Arc::new(BidderTimeoutFanout {
            clock: Arc::<FakeClock>::clone(&clock),
            spend: Duration::from_millis(200),
        });
        let response = drive_handler(fanout, &clock);
        let ext = ext_of(&response);
        assert_eq!(
            ext.pointer("/errors/fake_bb"),
            Some(&serde_json::json!([{ "code": 1_u32, "message": "bidder timeout" }])),
            "the bidder still reports its own timeout, got ext {ext}",
        );
        assert_eq!(
            ext.pointer("/errors/prebid"),
            None,
            "the auction budget survived, so there is no global error at all; got ext {ext}",
        );
    }
}
