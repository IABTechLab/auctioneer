//! auctioneer and `EdgeZero` share ONE clock domain, by type identity.
//!
//! The migration's whole point is that no projection, epoch or
//! conversion sits between auctioneer's monotonic time and `EdgeZero`'s.
//! That is a compile-time property, so the assertions here are
//! deliberately type-level: a coercion that stopped compiling would
//! mean the two crates had drifted onto separate `web-time` copies,
//! which no runtime assertion could catch.

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
    use auctioneer_bidder::BidderHttpResponse;
    use auctioneer_core::handlers::{TestFanoutHandle, auction};
    use auctioneer_core::host_primitives::{
        RequestClock, TestClockHandle, TestMonotonicClockHandle, TestRngHandle,
    };
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
    };
    use auctioneer_openrtb::ext::imp::ImpExt;
    use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, Imp};
    use auctioneer_openrtb::types::bid_response::BidResponse;
    use auctioneer_openrtb::types::field::JsonField;
    use auctioneer_openrtb::types::inventory::Site;
    use auctioneer_runtime::{Clock, FakeClock, MonotonicInstant, Rng, SeededRng};
    use edgezero_core::body::Body;
    use edgezero_core::http::StatusCode;
    use edgezero_core::time::{Deadline, MonotonicClock, MonotonicInstant as EdgeMonotonicInstant};
    use futures::executor::block_on;
    use http::{Method, Request, Uri};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use time::OffsetDateTime;

    const WIDE_CAP: usize = 1 << 20;
    const BIDDER: &str = "fake_bb";

    /// The migration, stated as a coercion.
    ///
    /// `auctioneer_runtime::MonotonicInstant` and
    /// `edgezero_core::time::MonotonicInstant` are the SAME type, so an
    /// instant minted by either side binds to the other's name without
    /// a conversion, and feeds `Deadline::at_instant` directly. If Cargo
    /// ever resolved the two crates onto separate `web-time` copies,
    /// every line below would stop compiling.
    #[test]
    fn monotonic_instant_is_edgezero_type_identity() {
        let from_auctioneer: EdgeMonotonicInstant = MonotonicInstant::now();
        let from_edgezero: MonotonicInstant = EdgeMonotonicInstant::now();
        assert!(
            from_edgezero >= from_auctioneer,
            "the two names denote one type, so their instants sit on one ordered axis"
        );

        let clock = FakeClock::new(OffsetDateTime::UNIX_EPOCH);
        let deadline = Deadline::at_instant(clock.now_monotonic());
        assert_eq!(
            deadline.instant(),
            clock.now_monotonic(),
            "the instant survives the round trip through EdgeZero's Deadline unchanged"
        );

        // And the composite behaves as an `EdgeZero` deadline: a clock
        // advanced past it reads as expired on `EdgeZero`'s own rule.
        let future_deadline = Deadline::at_instant(
            clock
                .now_monotonic()
                .checked_add(Duration::from_millis(100))
                .expect("no overflow"),
        );
        assert!(!future_deadline.is_expired_at(clock.now_monotonic()));
        clock.advance(Duration::from_millis(100));
        assert!(
            future_deadline.is_expired_at(clock.now_monotonic()),
            "a deadline exactly at now is expired on EdgeZero's inclusive rule"
        );
    }

    /// `RequestClock` reads its monotonic half from `EdgeZero`'s
    /// `MonotonicClock` and nothing else.
    ///
    /// This is the composite the auction handler builds once per
    /// request and hands to BOTH the `AuctionContext` and the
    /// `EdgeZeroFanout`. The counter proves the reads really went
    /// through the injected closure rather than to
    /// `MonotonicInstant::now()`.
    #[test]
    fn request_clock_monotonic_half_is_the_injected_edgezero_clock() {
        // An axis ten years past real now, so a stray
        // `MonotonicInstant::now()` could not possibly be mistaken for
        // one of these instants.
        const TEN_YEARS: Duration = Duration::from_hours(10 * 365 * 24);
        let fake = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let far_origin = fake
            .origin()
            .checked_add(TEN_YEARS)
            .expect("ten years is representable");
        let driver = Arc::clone(&fake);
        let calls = Arc::new(Mutex::new(0_u32));
        let counter = Arc::clone(&calls);
        let monotonic = MonotonicClock::new(move || {
            *counter.lock().expect("uncontended") += 1;
            driver
                .now_monotonic()
                .checked_add(TEN_YEARS)
                .expect("ten years is representable")
        });
        let wall: Arc<dyn Clock + Send + Sync> =
            Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let clock = RequestClock::new(monotonic, wall);

        assert_eq!(clock.now_monotonic(), far_origin);
        fake.advance(Duration::from_millis(400));
        assert_eq!(
            clock.now_monotonic(),
            far_origin
                .checked_add(Duration::from_millis(400))
                .expect("no overflow"),
            "advancing the source the closure reads moves the composite's monotonic half"
        );
        assert!(
            *calls.lock().expect("uncontended") >= 2,
            "the reads went through the injected closure, not an ambient now()"
        );
    }

    /// Records the `deadline_at` it was handed and the instant it
    /// observed on the clock the handler gave the fanout.
    struct RecordingFanout {
        clock: Arc<dyn Clock + Send + Sync>,
        seen_deadline: Mutex<Option<MonotonicInstant>>,
        seen_entry: Mutex<Option<MonotonicInstant>>,
    }

    #[async_trait(?Send)]
    impl BidderFanout for RecordingFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            *self.seen_deadline.lock().expect("uncontended") = Some(deadline.deadline_at);
            *self.seen_entry.lock().expect("uncontended") = Some(self.clock.now_monotonic());
            Ok(calls
                .iter()
                .map(|_| BidderCallResult {
                    // Raw byte-string fixture: this is a wire body, a `&[u8]`/`Vec<u8>`.
                    // `serde_json::json!` yields a `Value`, so converting would mean
                    // `.to_string().into_bytes()` at the call site for no gain.
                    result: Ok(BidderHttpResponse::new(
                        200,
                        vec![],
                        br#"{"id":"r","seatbid":[],"cur":"USD"}"#.to_vec(),
                    )),
                    elapsed: Duration::ZERO,
                })
                .collect())
        }
    }

    fn fixture_request() -> BidRequest {
        let mut bidders = serde_json::Map::new();
        bidders.insert(BIDDER.to_owned(), serde_json::json!({}));
        BidRequest {
            id: "req-clock-domain".to_owned(),
            imp: vec![Imp {
                id: "imp-1".to_owned(),
                banner: Some(Banner {
                    w: Some(300),
                    h: Some(250),
                    ..Banner::default()
                }),
                ext: Some(ImpExt {
                    bidder: JsonField::Value(serde_json::Value::Object(bidders)),
                    ..ImpExt::default()
                }),
                ..Imp::default()
            }],
            tmax: Some(300),
            cur: vec!["USD".to_owned()],
            site: Some(Site {
                page: Some("https://example.test/p".to_owned()),
                ..Site::default()
            }),
            ..BidRequest::default()
        }
    }

    /// The composed auction deadline and the fanout's wave-entry
    /// instant both land in the `RequestContext`'s OWN monotonic
    /// domain — proving neither path sampled a different source.
    ///
    /// The injected clock sits TEN YEARS past `MonotonicInstant::now()`
    /// and never advances, so both readings must come back as exact,
    /// known values on that axis. An instant a stray
    /// `MonotonicInstant::now()` produced would be a decade short and
    /// could not pass either equality, which is what makes this a
    /// proof rather than a consistency check. The closure's call
    /// counter then shows the exchange AND the fanout each really read
    /// it.
    #[test]
    fn handler_and_fanout_share_ctx_monotonic_clock() {
        const TEN_YEARS: Duration = Duration::from_hours(10 * 365 * 24);
        const TMAX_MS: u64 = 300;

        let config = format!(
            "schema_version = 1\n[host.aliases]\n{BIDDER}={:?}\n",
            serving_catalog::passthrough_info().name,
        );
        config_funding::install();
        let wall: Arc<dyn Clock + Send + Sync> =
            Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));

        // One source, ten years out, counting its own reads.
        let driver = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let far_origin = driver
            .origin()
            .checked_add(TEN_YEARS)
            .expect("ten years is representable");
        let reads = Arc::new(Mutex::new(0_u32));
        let reads_in_closure = Arc::clone(&reads);
        let driver_in_closure = Arc::clone(&driver);
        let monotonic = MonotonicClock::new(move || {
            *reads_in_closure.lock().expect("uncontended") += 1;
            driver_in_closure
                .now_monotonic()
                .checked_add(TEN_YEARS)
                .expect("ten years is representable")
        });

        // The fanout observes through a composite built from the SAME
        // monotonic source the handler will use.
        let observer: Arc<dyn Clock + Send + Sync> =
            Arc::new(RequestClock::new(monotonic.clone(), Arc::clone(&wall)));
        let fanout = Arc::new(RecordingFanout {
            clock: observer,
            seen_deadline: Mutex::new(None),
            seen_entry: Mutex::new(None),
        });
        let installed: Arc<dyn BidderFanout + Send + Sync> = Arc::<RecordingFanout>::clone(&fanout);

        let serialized = serde_json::to_vec(&fixture_request()).expect("the fixture serializes");
        let mut request = Request::builder()
            .method(Method::POST)
            .uri(Uri::from_static("/openrtb2/auction"))
            .header("content-type", "application/json")
            .body(Body::from_bytes(serialized))
            .expect("request builds");
        request.extensions_mut().insert(TestFanoutHandle(installed));
        request.extensions_mut().insert(TestClockHandle(wall));
        request
            .extensions_mut()
            .insert(TestMonotonicClockHandle(monotonic));
        request.extensions_mut().insert(TestRngHandle(rng));

        let (status, _response): (StatusCode, BidResponse) = block_on(async {
            let response = auction(ctx_with_config(&config, request))
                .await
                .expect("the handler answers");
            let status = response.status();
            let (_parts, body) = response.into_parts();
            let bytes = body
                .into_bytes_bounded(WIDE_CAP)
                .await
                .expect("response body reads");
            (
                status,
                serde_json::from_slice(&bytes).expect("the response deserializes"),
            )
        });
        assert_eq!(status, StatusCode::OK, "the fixture is a valid auction");

        let deadline_at = fanout
            .seen_deadline
            .lock()
            .expect("uncontended")
            .expect("the fanout was reached, so it recorded a deadline");
        let entry = fanout
            .seen_entry
            .lock()
            .expect("uncontended")
            .expect("the fanout recorded its wave-entry instant");

        // The clock never advanced, so every read of it returns the
        // same instant: the axis the RequestContext carries.
        assert_eq!(
            entry, far_origin,
            "the fanout's wave-entry instant is the ctx clock's instant, a decade \
             away from anything MonotonicInstant::now() could have produced"
        );
        assert_eq!(
            deadline_at,
            far_origin
                .checked_add(Duration::from_millis(TMAX_MS))
                .expect("no overflow"),
            "the composed deadline is request_start + tmax ON THAT SAME axis, so the \
             exchange read the ctx clock for request_start too"
        );
        assert!(
            *reads.lock().expect("uncontended") >= 2,
            "both the exchange and the fanout really called the injected closure"
        );
    }
}
