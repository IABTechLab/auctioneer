//! Handler-stage resolution reaches the exchange.
//!
//! Account identity, account-vs-`prepare()` precedence and alias
//! preservation are all decided in the auction handler before the plan
//! reaches `run_auction`. Each group below posts through the REAL
//! handler and varies only one input, so a difference in the answer can
//! only come from what the handler resolved.

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
    use crate::serving_catalog::passthrough_info;
    use crate::support::ctx_with_config;
    use async_trait::async_trait;
    use auctioneer_bidder::BidderHttpResponse;
    use auctioneer_core::handlers::{TestFanoutHandle, auction};
    use auctioneer_core::host_primitives::{TestClockHandle, TestRngHandle};
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutError,
        FanoutInternalError, TimeoutCause,
    };
    use auctioneer_openrtb::ext::imp::ImpExt;
    use auctioneer_openrtb::ext::request_prebid::ExtRequestPrebid;
    use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, BidRequestExt, Imp};
    use auctioneer_openrtb::types::bid_response::BidResponse;
    use auctioneer_openrtb::types::field::JsonField;
    use auctioneer_runtime::{Clock, FakeClock, Rng, SeededRng};
    use edgezero_core::body::Body;
    use edgezero_core::context::RequestContext;
    use edgezero_core::http::StatusCode;
    use futures::executor::block_on;
    use http::{Method, Request, Uri};
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::Duration;
    use time::OffsetDateTime;

    /// Response cap wide enough that nothing here trips it.
    const WIDE_CAP: usize = 1 << 20;

    /// Assemble the POST every group drives, with the given fanout
    /// installed and the fixture config bound.
    ///
    /// No adapter bootstrap runs in a test binary, so the host
    /// primitives the handler reaches for have to be injected.
    fn auction_ctx(
        config: &str,
        body_bytes: Vec<u8>,
        fanout: Arc<dyn BidderFanout + Send + Sync>,
    ) -> RequestContext {
        config_funding::install();
        let mut request = Request::builder()
            .method(Method::POST)
            .uri(Uri::from_static("/openrtb2/auction"))
            .header("content-type", "application/json")
            .body(Body::from_bytes(body_bytes))
            .expect("request builds");
        request.extensions_mut().insert(TestFanoutHandle(fanout));
        let clock: Arc<dyn Clock + Send + Sync> =
            Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));
        request.extensions_mut().insert(TestClockHandle(clock));
        request.extensions_mut().insert(TestRngHandle(rng));
        ctx_with_config(config, request)
    }

    mod bidder_override_keys {
        use super::*;
        use auctioneer_core::config::runtime::{RuntimeConfigError, resolve_runtime_config};
        use auctioneer_core::handlers::info_bidders;

        #[derive(Default)]
        struct RecordingFanout {
            calls: Mutex<usize>,
        }

        #[async_trait(?Send)]
        impl BidderFanout for RecordingFanout {
            async fn send_all(
                &self,
                calls: Vec<BidderCall>,
                _deadline: AuctionDeadline,
            ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
                let mut seen = self.calls.lock().unwrap_or_else(PoisonError::into_inner);
                *seen = seen.saturating_add(calls.len());
                Ok(calls
                    .iter()
                    .map(|_| BidderCallResult {
                        result: Err(FanoutError::Timeout(TimeoutCause::Bidder)),
                        elapsed: Duration::ZERO,
                    })
                    .collect())
            }
        }

        fn config(name: &str, fields: &str) -> String {
            format!(
                "[bidders.{name:?}]\n{fields}\n[accounts.publisher]\nmax_transport_bytes = 1024\n"
            )
        }

        fn catalog_ctx(config: &str) -> RequestContext {
            config_funding::install();
            ctx_with_config(
                config,
                Request::builder()
                    .method(Method::GET)
                    .uri(Uri::from_static("/info/bidders?enabledonly=true"))
                    .body(Body::empty())
                    .expect("catalog request builds"),
            )
        }

        #[test]
        fn production_resolution_accepts_64_byte_ascii_and_utf8_bidder_keys_with_account_map() {
            for name in [
                format!("fake_{}", "a".repeat(59)),
                format!("fake_a{}", "\u{e9}".repeat(29)),
            ] {
                let resolved = block_on(resolve_runtime_config(&catalog_ctx(&config(
                    &name,
                    "enabled = false",
                ))))
                .expect("the real extractor accepts a 64-byte bidder key");
                assert_eq!(resolved.bidders[&name].enabled, Some(false));
                assert_eq!(
                    resolved.accounts["publisher"].max_transport_bytes,
                    Some(1024)
                );
            }
        }

        #[test]
        fn production_resolution_rejects_65_byte_disabled_bidder_keys_with_account_map() {
            for name in [
                format!("fake_{}", "a".repeat(60)),
                format!("fake_{}\u{e9}", "a".repeat(58)),
            ] {
                for fields in [
                    "enabled = false",
                    "enabled = false\nmax_transport_bytes = 0",
                ] {
                    let ctx = catalog_ctx(&config(&name, fields));
                    match block_on(resolve_runtime_config(&ctx)) {
                        Err(RuntimeConfigError::MalformedHost { detail }) => {
                            assert!(detail.contains("Validation"), "got {detail}");
                            assert!(detail.len() < 256);
                            assert!(!detail.contains(&name));
                        }
                        other => panic!(
                            "the production resolver must reject an oversized bidder key: {other:?}"
                        ),
                    }
                }
            }
        }

        #[test]
        fn oversized_disabled_bidder_key_fails_before_auction_dispatch_and_catalog_construction() {
            let config = config(&format!("fake_{}", "a".repeat(60)), "enabled = false");
            let fanout = Arc::new(RecordingFanout::default());
            let handler_fanout: Arc<dyn BidderFanout + Send + Sync> =
                Arc::<RecordingFanout>::clone(&fanout);
            let body = serde_json::to_vec(&serde_json::json!({
                "id": "bidder-key-bound",
                "tmax": 1000_i64,
                "cur": ["USD"],
                "regs": { "gdpr": 0_u8 },
                "imp": [{
                    "id": "imp-1",
                    "banner": { "w": 300_i32, "h": 250_i32 },
                    "ext": { "bidder": { "fake_bidder": {} } },
                }],
                "ext": { "prebid": { "aliases": { "fake_bidder": passthrough_info().name } } },
            }))
            .expect("auction request serializes");
            let auction_response = block_on(auction(auction_ctx(&config, body, handler_fanout)))
                .expect("auction handler answers");
            let catalog_response =
                block_on(info_bidders(catalog_ctx(&config))).expect("catalog handler answers");
            for response in [auction_response, catalog_response] {
                assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
                let bytes = block_on(response.into_body().into_bytes_bounded(WIDE_CAP))
                    .expect("error response reads");
                assert_eq!(bytes.as_ref(), b"Malformed host configuration");
            }
            assert_eq!(
                *fanout.calls.lock().unwrap_or_else(PoisonError::into_inner),
                0
            );
        }
    }

    /// Account resolution runs BEFORE `prepare()`, so an account-side
    /// refusal wins over a structural request error.
    ///
    /// Each row posts the SAME structurally-invalid body through the real
    /// auction handler and varies only which publisher it names, so the
    /// status difference can only come from resolution order.
    mod account_before_prepare {
        use super::*;

        /// A disabled account and an unconfigured one, so the only
        /// variable between the rows is which `[accounts.<id>]` entry
        /// matches.
        const DISABLED_ACCOUNT: &str = "acct-42";
        const UNCONFIGURED_ACCOUNT: &str = "acct-99";

        const CONFIG: &str = "[accounts.acct-42]\ndisabled = true\n";

        /// The handler resolves its fanout before anything else, so every
        /// row needs one installed even when no call is ever dispatched.
        struct UnusedFanout;

        #[async_trait(?Send)]
        impl BidderFanout for UnusedFanout {
            async fn send_all(
                &self,
                _calls: Vec<BidderCall>,
                _deadline: AuctionDeadline,
            ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
                Ok(vec![])
            }
        }

        /// `tmax` is typed `Option<i64>`, so a string there fails the
        /// `BidRequest` deserialize `prepare()` depends on — the 400 path.
        fn body_with_invalid_tmax(publisher: &str) -> Vec<u8> {
            serde_json::to_vec(&serde_json::json!({
                "id": "auc-order",
                "tmax": "not-a-number",
                "site": { "publisher": { "id": publisher }, "page": "https://pub.example.test/a" },
                "imp": [{ "id": "imp-1", "banner": { "w": 300, "h": 250 } }],
            }))
            .expect("fixture body serialises")
        }

        fn drive(body_bytes: Vec<u8>) -> (StatusCode, String) {
            let ctx = auction_ctx(CONFIG, body_bytes, Arc::new(UnusedFanout));

            block_on(async {
                let response = auction(ctx).await.expect("the handler answers");
                let status = response.status();
                let (_parts, body) = response.into_parts();
                let bytes = body
                    .into_bytes_bounded(WIDE_CAP)
                    .await
                    .expect("response body reads");
                (
                    status,
                    String::from_utf8(bytes.to_vec()).expect("the error body is UTF-8"),
                )
            })
        }

        #[test]
        fn disabled_account_wins_over_the_prepare_validation_error() {
            let (status, body) = drive(body_with_invalid_tmax(DISABLED_ACCOUNT));
            assert_eq!(
                status,
                StatusCode::SERVICE_UNAVAILABLE,
                "the disabled account must be refused before `prepare()` reports the bad tmax; \
                 body {body}",
            );
            assert_eq!(body, "Invalid request: acct-42\n");
        }

        /// The control: the identical malformed body under a publisher the
        /// deployment has not configured still reaches `prepare()` and is a
        /// 400. Without this row the assertion above would also pass if the
        /// handler answered 503 for every malformed request.
        #[test]
        fn unconfigured_account_still_surfaces_the_prepare_validation_error() {
            let (status, body) = drive(body_with_invalid_tmax(UNCONFIGURED_ACCOUNT));
            assert_eq!(
                status,
                StatusCode::BAD_REQUEST,
                "an unknown publisher is not an error; the structural failure is; body {body}",
            );
            assert!(
                body.starts_with("Invalid request: "),
                "the 400 keeps the shared prefix; body {body}",
            );
        }
    }

    /// The resolved account id reaches the exchange, so host policy keyed by
    /// account id actually applies.
    ///
    /// The `non_standard_publishers` gate compares `ctx.account.id` against
    /// the host list. While the handler hardcoded a fallback identity that
    /// gate was unreachable for every real publisher — and would have
    /// matched EVERY request had an operator listed the fallback string. Both
    /// rows below post the same body through the real handler under the same
    /// config and vary only the publisher id, so the difference in fan-out
    /// can only come from the id the handler resolved.
    mod account_identity {
        use super::*;

        /// GDPR enforced with purpose 2 ON, so a publisher the host does NOT
        /// exempt has its bidder dropped before fan-out — and the one it DOES
        /// exempt keeps it.
        const CONFIG: &str = "[gdpr]\n\
                              enabled = true\n\
                              non_standard_publishers = [\"acct-exempt\"]\n\n\
                              [gdpr.tcf2]\n\
                              enabled = true\n\n\
                              [gdpr.tcf2.purposes.2]\n\
                              enforce_purpose = \"enabled\"\n";

        #[derive(Default)]
        struct CountingFanout {
            calls: Mutex<usize>,
        }

        #[async_trait(?Send)]
        impl BidderFanout for CountingFanout {
            async fn send_all(
                &self,
                calls: Vec<BidderCall>,
                _deadline: AuctionDeadline,
            ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
                let mut seen = self.calls.lock().unwrap_or_else(PoisonError::into_inner);
                *seen = seen.wrapping_add(calls.len());
                Ok(calls
                    .iter()
                    .map(|_| BidderCallResult {
                        result: Err(FanoutError::Timeout(TimeoutCause::Bidder)),
                        elapsed: Duration::ZERO,
                    })
                    .collect())
            }
        }

        fn body(publisher: &str) -> Vec<u8> {
            serde_json::to_vec(&serde_json::json!({
                "id": "auc-identity",
                "tmax": 1000,
                "cur": ["USD"],
                "regs": { "gdpr": 1 },
                "site": {
                    "publisher": { "id": publisher },
                    "page": "https://pub.example.test/article",
                },
                "imp": [{
                    "id": "imp-1",
                    "banner": { "w": 300, "h": 250 },
                    "ext": { "bidder": { "fake_bidder": {} } },
                }],
                "ext": { "prebid": { "aliases": { "fake_bidder": passthrough_info().name } } },
            }))
            .expect("fixture body serialises")
        }

        /// Returns `(outbound call count, response ext)`.
        fn drive(publisher: &str) -> (usize, serde_json::Value) {
            let fanout = Arc::new(CountingFanout::default());
            let handler_fanout: Arc<dyn BidderFanout + Send + Sync> =
                Arc::<CountingFanout>::clone(&fanout);
            let ctx = auction_ctx(CONFIG, body(publisher), handler_fanout);

            let response: BidResponse = block_on(async {
                let response = auction(ctx).await.expect("the handler answers");
                assert_eq!(response.status(), StatusCode::OK);
                let (_parts, response_body) = response.into_parts();
                let bytes = response_body
                    .into_bytes_bounded(WIDE_CAP)
                    .await
                    .expect("response body reads");
                serde_json::from_slice(&bytes).expect("the response deserializes as a BidResponse")
            });
            let seen = *fanout.calls.lock().unwrap_or_else(PoisonError::into_inner);
            (seen, response.ext.unwrap_or(serde_json::Value::Null))
        }

        #[test]
        fn host_listed_publisher_is_exempted_by_its_resolved_account_id() {
            let (calls, ext) = drive("acct-exempt");
            assert_eq!(
                calls, 1,
                "the host lists this publisher, so enforcement is skipped and the bidder is \
                 dispatched; ext {ext}",
            );
            assert_eq!(
                ext.pointer("/warnings/fake_bidder"),
                None,
                "an exempted account emits no privacy-block warning; ext {ext}",
            );
        }

        #[test]
        fn unlisted_publisher_is_still_enforced() {
            let (calls, ext) = drive("acct-plain");
            assert_eq!(
                calls, 0,
                "an unlisted publisher stays under enforcement, so the bidder never reaches \
                 fan-out; ext {ext}",
            );
            assert_eq!(
                ext.pointer("/warnings/fake_bidder"),
                Some(&serde_json::json!([{
                    "code": 10017_u32,
                    "message": "bidder blocked by privacy settings: fake_bidder",
                }])),
                "the enforced row carries the privacy-block warning; ext {ext}",
            );
        }
    }

    /// Request-declared aliases must survive the handler's plan build.
    ///
    /// Alias resolution moved out of `run_auction` and into the handler,
    /// which wraps the resolver's result into the `AuctionPlan`. Wrapping an
    /// EMPTY map instead would still run the resolver — so every
    /// alias-rejection 400 keeps firing — while silently discarding the
    /// resolved map. A request naming an alias would then lose that bidder:
    /// the splitter falls back to the alias name as its own base, the
    /// registry misses, and the bidder is skipped with no error anywhere.
    ///
    /// The fixture names a request alias alongside a host alias bound to
    /// the same published base. Both seats must survive: retaining only
    /// host aliases would silently lose the request-declared alias.
    mod alias_preserved {
        use super::*;

        /// A request alias alongside a host alias for the same published base.
        const ALIAS: &str = "fake_myalias";
        const BASE: &str = "fake_bidder";

        /// Answers every dispatched call with one bid on `imp-1`. The seat
        /// in the body is irrelevant — the response builder keys seat rows
        /// by the bidder the call was made FOR, which is exactly what the
        /// alias must reach.
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
                        result: Ok(BidderHttpResponse::new(200, vec![], br#"{"id":"r","seatbid":[{"seat":"fake_s","bid":[{"id":"b1","impid":"imp-1","price":1.5,"w":300,"h":250}]}],"cur":"USD"}"#.to_vec())),
                        elapsed: Duration::from_millis(5),
                    })
                    .collect())
            }
        }

        /// One imp naming BOTH the host alias and the request-declared alias.
        fn fixture_request() -> BidRequest {
            let mut bidders = serde_json::Map::new();
            bidders.insert(BASE.to_owned(), serde_json::json!({}));
            bidders.insert(ALIAS.to_owned(), serde_json::json!({}));

            let mut aliases: BTreeMap<String, String> = BTreeMap::new();
            aliases.insert(ALIAS.to_owned(), passthrough_info().name.clone());

            BidRequest {
                id: "auc-alias".to_owned(),
                tmax: Some(1000_i64),
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
                ext: Some(BidRequestExt {
                    prebid: Some(ExtRequestPrebid {
                        aliases,
                        ..ExtRequestPrebid::default()
                    }),
                    ..BidRequestExt::default()
                }),
                ..BidRequest::default()
            }
        }

        /// POST the fixture through the real auction handler.
        fn drive() -> (StatusCode, BidResponse) {
            let serialized =
                serde_json::to_vec(&fixture_request()).expect("the fixture serializes");
            let config = toml::to_string(&serde_json::json!({
                "aliases": { BASE: passthrough_info().name }
            }))
            .expect("host alias serializes");
            let ctx = auction_ctx(&config, serialized, Arc::new(BiddingFanout));

            block_on(async {
                let response = auction(ctx).await.expect("the handler answers");
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
            })
        }

        #[test]
        fn a_request_declared_alias_reaches_fanout_and_bids() {
            let (status, response) = drive();
            assert_eq!(
                status,
                StatusCode::OK,
                "the fixture is a valid auction; a 400 here would mean alias resolution rejected it",
            );

            let seats: Vec<&str> = response
                .seatbid
                .iter()
                .map(|seat| seat.seat.as_str())
                .collect();
            assert_eq!(
                seats,
                vec![BASE, ALIAS],
                "the handler must carry the RESOLVED alias map into the plan: dropping it leaves \
                 aliases with no base in the registry; retaining only host aliases silently \
                 loses the request-declared alias while the host alias still bids",
            );

            let alias_seat = response
                .seatbid
                .iter()
                .find(|seat| seat.seat == ALIAS)
                .expect("the alias seat row is present");
            assert_eq!(
                alias_seat.bid.len(),
                1,
                "the alias seat must carry the bid its own dispatched call returned",
            );
        }
    }
}
