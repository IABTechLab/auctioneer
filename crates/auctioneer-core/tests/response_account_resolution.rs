//! Snapshot fixtures for single-TOML account resolution.
//!
//! Both rows post a recorded request body through the real auction
//! handler with a `[accounts.<id>]` overlay installed, and compare the
//! whole answer against a recorded expectation:
//!
//! - a configured overlay reaches the auction, so the account-supplied
//!   `targeting_prefix` shows up in every emitted targeting key. The
//!   prefix is the observable proof that `AccountSource::Configured` was
//!   taken: the baked default account carries `hb`, so an
//!   `acct_*` key can only come from the `[accounts.<id>]` entry.
//! - a disabled overlay refuses the request with a 503 whose whole wire
//!   shape (status, content type, body) is pinned. The same body also
//!   carries a malformed `tmax`, so the row doubles as the ordering
//!   contract: account resolution runs before `prepare()`, and the
//!   account refusal must win.

mod support;

#[cfg(test)]
mod config_funding_support;
use config_funding_support as config_funding;

#[cfg(test)]
pub use auctioneer_bidder::test_support::serving_catalog;

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
        TestClockHandle, TestMonotonicClockHandle, TestRngHandle,
    };
    use auctioneer_core::test_support::config_store::auction_context_for_route;
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
    };
    use auctioneer_runtime::{Clock, FakeClock, Rng, SeededRng};
    use edgezero_core::body::Body;
    use edgezero_core::http::StatusCode;
    use edgezero_core::http::header::CONTENT_TYPE;
    use edgezero_core::time::MonotonicClock;
    use futures::executor::block_on;
    use http::{Method, Request, Uri};
    use std::fs;
    use std::mem;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use time::OffsetDateTime;

    /// Response cap wide enough that nothing here trips it.
    const WIDE_CAP: usize = 1 << 20;

    static AUCTIONS: Mutex<()> = Mutex::new(());

    /// The publisher id both fixtures name, and the `[accounts.<id>]`
    /// key both overlays declare.
    const ACCOUNT_ID: &str = "acct-21";

    const CONFIGURED_TOML: &str =
        "schema_version = 1\n[host]\n[accounts.acct-21]\ntargeting_prefix = \"acct\"\n";
    const DISABLED_TOML: &str = "schema_version = 1\n[host]\n[accounts.acct-21]\ndisabled = true\ntargeting_prefix = \"acct\"\n";

    /// Answers every dispatched call with one banner bid on `imp-1`.
    struct BiddingFanout;

    #[async_trait(?Send)]
    impl BidderFanout for BiddingFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            Ok(calls
                .iter()
                .map(|_| BidderCallResult {
                    // Raw byte-string fixture: this is a wire body, a `&[u8]`/`Vec<u8>`.
                    // `serde_json::json!` yields a `Value`, so converting would mean
                    // `.to_string().into_bytes()` at the call site for no gain.
                    result: Ok(BidderHttpResponse::new(200, vec![], br#"{"id":"r","seatbid":[{"seat":"fake_s","bid":[{"id":"bid-1","impid":"imp-1","price":1.5,"w":300,"h":250,"crid":"creative-1"}]}],"cur":"USD"}"#.to_vec())),
                    elapsed: Duration::from_millis(10),
                })
                .collect())
        }
    }

    fn fixture(name: &str) -> String {
        let path = format!(
            "{}/tests/fixtures/account_resolution/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("fixture {path} unreadable: {err}"))
    }

    /// POST a fixture body through the real auction handler with the
    /// given overlay installed.
    ///
    /// The clock is a `FakeClock` frozen at the UNIX epoch and the rng is
    /// seeded, so `ext.prebid.auctiontimestamp`, every response-time
    /// figure, and the whole-auction deadline are fixed values. A real
    /// clock would let a slow runner overrun `tmax` and add an
    /// auction-wide timeout entry to the recorded answer.
    fn drive(config_toml: &str, request_body: &str) -> (StatusCode, Option<String>, Vec<u8>) {
        let _serialized = AUCTIONS.lock().expect("auction fixture lock");
        config_funding::install();
        let mut config: toml::Value = toml::from_str(config_toml).unwrap();
        let aliases = toml::Table::from_iter([(
            "fake_ac".to_owned(),
            toml::Value::String(serving_catalog::passthrough_info().name.clone()),
        )]);
        config["host"]
            .as_table_mut()
            .unwrap()
            .insert("aliases".into(), toml::Value::Table(aliases));
        let configured = toml::to_string(&config).unwrap();
        let fake = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let clock: Arc<dyn Clock + Send + Sync> = Arc::<FakeClock>::clone(&fake);
        let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));
        let mut request = Request::builder()
            .method(Method::POST)
            .uri(Uri::from_static("/openrtb2/auction"))
            .header("content-type", "application/json")
            .body(Body::from_bytes(request_body.as_bytes().to_vec()))
            .expect("request builds");
        request
            .extensions_mut()
            .insert(TestFanoutHandle(Arc::new(BiddingFanout)));
        request.extensions_mut().insert(TestClockHandle(clock));
        // `TestClockHandle` governs wall-clock reads only: after the
        // clock-domain migration the auction path takes monotonic time
        // from `ctx.monotonic_clock()` and never consults it. Without
        // this handle `responsetimemillis` sums a frozen
        // `max_call_elapsed` with REAL elapsed time for the other two
        // terms, so a recorded response containing it drifts by a millisecond
        // under load.
        let monotonic_source = Arc::<FakeClock>::clone(&fake);
        let monotonic = MonotonicClock::new(move || monotonic_source.now_monotonic());
        request
            .extensions_mut()
            .insert(TestMonotonicClockHandle(monotonic.clone()));
        request.extensions_mut().insert(TestRngHandle(rng));
        let (ctx, mut envelope) = auction_context_for_route(
            ctx_with_config(&configured, request)
                .into_request()
                .unwrap(),
            "/openrtb2/auction",
            monotonic,
        );

        block_on(async {
            *envelope.response_mut() = auction(ctx).await.expect("the handler answers");
            let response = mem::take(envelope.response_mut());
            let status = response.status();
            let content_type = response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            let (_parts, body) = response.into_parts();
            let bytes = body
                .into_bytes_bounded(WIDE_CAP)
                .await
                .expect("response body reads");
            drop(envelope);
            (status, content_type, bytes.to_vec())
        })
    }

    #[test]
    fn configured_account_overlay_reaches_the_auction_targeting_keys() {
        let (status, content_type, body) =
            drive(CONFIGURED_TOML, &fixture("configured_overlay_request.json"));
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/json"));

        let actual: serde_json::Value =
            serde_json::from_slice(&body).expect("the response is JSON");
        let expected: serde_json::Value =
            serde_json::from_str(&fixture("configured_overlay_expected.json"))
                .expect("the expectation is JSON");
        assert_eq!(
            actual,
            expected,
            "recorded answer drifted; actual was {}",
            serde_json::to_string_pretty(&actual).expect("actual re-serialises"),
        );
        // The clock is pinned, so an auction-wide timeout is unreachable.
        // Recording one would freeze the wrong contract in place.
        assert!(
            !serde_json::to_string(&actual)
                .expect("actual re-serialises")
                .contains("\"code\":12"),
            "the pinned clock must never let an auction-wide timeout reach the recorded response",
        );
    }

    #[test]
    fn publisher_absence_does_not_select_the_explicit_unknown_account() {
        let config = "schema_version = 1\n[host]\n[account_defaults]\ntargeting_prefix = \"default\"\n[accounts.unknown]\ntargeting_prefix = \"named\"\n";
        for (named, expected_key, excluded_key) in [
            (false, "default_bidder", "named_bidder"),
            (true, "named_bidder", "default_bidder"),
        ] {
            let mut request: serde_json::Value =
                serde_json::from_str(&fixture("configured_overlay_request.json")).unwrap();
            assert_eq!(request["site"]["publisher"]["id"], ACCOUNT_ID);
            if named {
                request["site"]["publisher"]["id"] = serde_json::json!("unknown");
            } else {
                let removed = request["site"]["publisher"]
                    .as_object_mut()
                    .unwrap()
                    .remove("id")
                    .expect("the before-state contains the publisher identity");
                assert_eq!(removed, ACCOUNT_ID);
            }
            let (status, content_type, body) = drive(config, &request.to_string());
            assert_eq!(status, StatusCode::OK, "named={named}");
            assert_eq!(content_type.as_deref(), Some("application/json"));
            let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
            let seats = response["seatbid"].as_array().unwrap();
            assert_eq!(seats.len(), 1);
            let bids = seats[0]["bid"].as_array().unwrap();
            assert_eq!(bids.len(), 1);
            assert_eq!(bids[0]["id"], "bid-1");
            let targeting = bids[0]["ext"]["prebid"]["targeting"].as_object().unwrap();
            assert_eq!(targeting[expected_key], "fake_ac", "named={named}");
            assert!(!targeting.contains_key(excluded_key), "named={named}");
        }
    }

    #[test]
    fn disabled_account_overlay_is_a_503_with_the_plain_text_body() {
        let (status, content_type, body) =
            drive(DISABLED_TOML, &fixture("disabled_account_request.json"));
        let text = String::from_utf8(body).expect("the error body is UTF-8");

        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "a disabled account is refused before `prepare()` reports the malformed tmax; \
             body {text}",
        );
        assert_eq!(content_type.as_deref(), Some("text/plain; charset=utf-8"));
        assert_eq!(
            text,
            format!("Invalid request: {ACCOUNT_ID}\n"),
            "the wire body carries the account id and nothing else: no numeric taxonomy code \
             and no variant name",
        );
        assert!(
            !text.contains('7') && !text.contains("AccountDisabled"),
            "the taxonomy code and the variant name are internal; body {text}",
        );
    }

    /// Control for the row above. That row asserts a 503 for a request
    /// whose `tmax` is *also* malformed, which alone cannot distinguish
    /// "the account gate ran first" from "this handler answers 503 to
    /// everything". Same request body, same handler, but no
    /// `[accounts.acct-21]` entry: the account gate has nothing to refuse,
    /// so `prepare()` reports the malformed `tmax` and the answer is a
    /// 400. The pair is what pins the ordering.
    #[test]
    fn unconfigured_account_surfaces_the_malformed_tmax_as_a_400() {
        let (status, _content_type, body) = drive(
            "schema_version = 1\n[host]\n",
            &fixture("disabled_account_request.json"),
        );
        let text = String::from_utf8(body).expect("the error body is UTF-8");

        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "with no account overlay to refuse, the same body must fail validation \
             instead; body {text}",
        );
    }
}
