//! Dev `BidderFanout` interim impl, plus the two per-call helpers every
//! fanout strategy shares.
//!
//! Known gaps of `DevFanout` itself (closed by `EdgeZeroFanout`
//! driven through `FanoutStrategy`):
//! 1. No `AuctionDeadline` enforcement.
//! 2. No per-bidder `timeout`.
//! 3. Fastly runs serialized under the underlying `ProxyClient`.
//! 4. `max_decoded_bytes` enforced post-buffer (not streaming).

use async_trait::async_trait;
use auctioneer_bidder::{BidderHttpRequest, BidderHttpResponse, ProxyError};
use auctioneer_exchange::{
    AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutError, FanoutInternalError,
    OversizeOrigin,
};
use auctioneer_runtime::Clock;
use futures::future::join_all;

/// Minimal seam over `EdgeZero`'s `ProxyClient` surface. Adapters wire
/// this to whichever proxy handle they own; auctioneer-core stays portable
/// across `EdgeZero` versions.
#[async_trait(?Send)]
pub trait ProxyClientApi: Send + Sync {
    /// Send one already-prepared outbound request and buffer the
    /// response.
    ///
    /// # Errors
    /// [`ProxyError::Transport`] for any transport or decode failure.
    /// The carried string is diagnostic only — callers must classify on
    /// the VARIANT, never on the message.
    async fn send_one(&self, req: BidderHttpRequest) -> Result<BidderHttpResponse, ProxyError>;
}

pub struct DevFanout<'h, C: Clock + ?Sized, P: ProxyClientApi> {
    pub client: &'h P,
    pub clock: &'h C,
}

#[async_trait(?Send)]
impl<C: Clock + ?Sized + Send + Sync, P: ProxyClientApi> BidderFanout for DevFanout<'_, C, P> {
    async fn send_all(
        &self,
        calls: Vec<BidderCall>,
        _deadline: AuctionDeadline,
    ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
        let mut futures_vec = Vec::with_capacity(calls.len());
        for call in calls {
            futures_vec.push(self.send_one(call));
        }
        Ok(join_all(futures_vec).await)
    }
}

impl<C: Clock + ?Sized + Send + Sync, P: ProxyClientApi> DevFanout<'_, C, P> {
    async fn send_one(&self, call: BidderCall) -> BidderCallResult {
        let start = self.clock.now_monotonic();
        let outcome = self.client.send_one(call.request).await;
        let elapsed = self.clock.now_monotonic().saturating_duration_since(start);
        let result = classify(outcome, call.max_decoded_bytes);
        BidderCallResult { result, elapsed }
    }
}

/// Map one send outcome onto the exchange's error taxonomy. Matches the
/// error VARIANT — never a substring of its message.
pub(crate) fn classify(
    outcome: Result<BidderHttpResponse, ProxyError>,
    max_decoded_bytes: u64,
) -> Result<BidderHttpResponse, FanoutError> {
    match outcome {
        // auctioneer's OWN ceiling, applied to a body the transport already
        // handed back whole, so the origin is auctioneer's call cap and not
        // any platform response limit.
        Ok(resp) if u64::try_from(resp.body_len()).unwrap_or(u64::MAX) > max_decoded_bytes => Err(
            FanoutError::OversizeResponse(OversizeOrigin::CallResponseCap),
        ),
        Ok(resp) if !is_biddable_status(resp.status()) => Err(FanoutError::BadServerResponse),
        Ok(resp) => Ok(resp),
        // The untrusted transport string goes NOWHERE: not the wire, not
        // the log. Scrubbing it is bypassable (a standalone `token=xyz`,
        // a bearer credential, any key/value the upstream happens to
        // include), so the only safe rule is to emit allowlisted
        // metadata — here the fixed category — and drop the free text.
        // The carried payload is a FIXED string too, so even a stray
        // `{:?}` on the `FanoutError` cannot leak. No bidder name is
        // logged either: a `BidderCall` carries no identity, and raw
        // request aliases must never be logged.
        Err(ProxyError::Transport(_detail)) => {
            log::warn!(target: "auctioneer::fanout", "bidder transport error");
            Err(FanoutError::Other("transport error".to_owned()))
        }
    }
}

/// Whether a transport status may carry a bidder's auction answer at
/// all, decided before any adapter is handed the body.
///
/// Only 2xx can. Everything else is the transport describing the
/// request's fate rather than a bidder describing an auction:
///
/// * **3xx** says where the resource lives. auctioneer sends nothing it
///   wants followed, so a redirect is a misconfigured endpoint or an
///   intercepting proxy, and the entity attached to one belongs to
///   whatever issued the redirect — not to the bidder. Declining to
///   FOLLOW a redirect does not cover this: the redirect's own body
///   still arrives and would otherwise be parsed.
/// * **4xx** says auctioneer's request was refused, so there is no answer
///   in it.
/// * **5xx** says the bidder failed to produce one.
/// * **1xx** is informational and provisional — never a final answer.
///   In practice a client sees a `100` or `103` consumed by the
///   transport and gets the real status next, so this is a defensive
///   floor rather than a reachable branch; if one ever surfaced here,
///   treating it as a bid would admit a body that promised a later
///   status.
///
/// Deciding it HERE is what keeps the rule single. Fifty adapters each
/// repeating a status guard is fifty chances to write the wrong
/// comparison, and an adapter admitting everything under 400 already
/// admits both 3xx and 1xx.
pub(crate) fn is_biddable_status(status: u16) -> bool {
    (200..300).contains(&status)
}

#[cfg(test)]
mod tests {
    use crate::test_support::outbound_request;

    use super::{BidderFanout as _, DevFanout, ProxyClientApi, classify};
    use async_trait::async_trait;
    use auctioneer_bidder::{BidderHttpRequest, BidderHttpResponse, ProxyError};
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, FanoutError, OversizeOrigin, TmaxSource,
    };
    use auctioneer_runtime::{FakeClock, MonotonicInstant};
    use futures::executor::block_on;
    use std::time::Duration;

    /// A transport detail carrying every shape a scrubber would have to
    /// anticipate: credentials in the authority, a query parameter, and
    /// a bare key/value token.
    const HOSTILE_DETAIL: &str = "https://user:pw@bidder.example/rtb?key=abc token=xyz refused";

    /// Substrings that must not survive into any log line, public
    /// message, or debug rendering.
    const SECRET_NEEDLES: [&str; 5] = ["bidder.example", "user", "pw", "key=abc", "token=xyz"];

    /// Valid, bid-bearing `OpenRTB`: a status rule has to refuse it on
    /// its status alone, since nothing about its content is refusable.
    // Raw-string `const`: `serde_json::json!` yields a `Value`, which no
    // `const` can hold. Converting means turning this into a function
    // returning a `String` and re-rendering it at every use.
    const BID_JSON: &[u8] =
        br#"{"id":"a","cur":"USD","seatbid":[{"seat":"fake_s","bid":[{"id":"b","impid":"i","price":1.5}]}]}"#;

    /// Answers every call with the same canned status and body, or
    /// fails every call with the same error.
    ///
    /// The status and BODY are stored rather than a built response,
    /// because a `BidderHttpResponse` seals its body and is
    /// deliberately not `Clone`: a mock answering repeated calls from
    /// one template would have to duplicate sealed state. Building a
    /// fresh response per call is what a real transport does.
    struct MockClient {
        status: u16,
        body: Vec<u8>,
        fail: Option<ProxyError>,
    }

    #[async_trait(?Send)]
    impl ProxyClientApi for MockClient {
        async fn send_one(
            &self,
            _req: BidderHttpRequest,
        ) -> Result<BidderHttpResponse, ProxyError> {
            match &self.fail {
                Some(err) => Err(err.clone()),
                None => Ok(BidderHttpResponse::new(
                    self.status,
                    vec![],
                    self.body.clone(),
                )),
            }
        }
    }

    fn call_with_cap(cap: u64) -> BidderCall {
        BidderCall {
            request: outbound_request::request(
                "POST".into(),
                "https://x".into(),
                vec![],
                vec![],
                vec![],
            )
            .expect("sink-built fixture"),
            timeout: Duration::from_millis(100),
            max_transport_bytes: cap,
            max_decoded_bytes: cap,
        }
    }

    fn deadline() -> AuctionDeadline {
        AuctionDeadline {
            deadline_at: MonotonicInstant::now()
                .checked_add(Duration::from_secs(1))
                .expect("no overflow"),
            source: TmaxSource::Request,
        }
    }

    /// A transport failure is no longer refined by guessing at the
    /// message. Every kind the host cannot distinguish collapses to
    /// `Other`, whose payload is the fixed category.
    #[test]
    fn transport_error_is_categorized_as_other() {
        block_on(async {
            let client = MockClient {
                status: 200,
                body: vec![],
                fail: Some(ProxyError::Transport("timeout exceeded".into())),
            };
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            let fanout = DevFanout {
                client: &client,
                clock: &clock,
            };
            let results = fanout
                .send_all(vec![call_with_cap(1024)], deadline())
                .await
                .unwrap();
            match &results[0].result {
                Err(FanoutError::Other(payload)) => assert_eq!(
                    payload, "transport error",
                    "the fixed category, never the mock's detail",
                ),
                other => panic!("expected Other(\"transport error\"), got {other:?}"),
            }
        });
    }

    #[test]
    fn oversize_response_categorized() {
        block_on(async {
            let body = vec![0_u8; 4096];
            let client = MockClient {
                status: 200,
                body,
                fail: None,
            };
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            let fanout = DevFanout {
                client: &client,
                clock: &clock,
            };
            let results = fanout
                .send_all(vec![call_with_cap(100)], deadline())
                .await
                .unwrap();
            assert!(
                matches!(
                    results[0].result,
                    Err(FanoutError::OversizeResponse(
                        OversizeOrigin::CallResponseCap
                    ))
                ),
                "this fanout's only size ceiling is the call's own \
                 max_decoded_bytes, got {:?}",
                results[0].result,
            );
        });
    }

    /// Every status class that cannot carry an auction answer is
    /// refused, and the one that can is admitted.
    ///
    /// The bodies are non-empty and identical across rows so the
    /// verdict can only be coming from the status.
    #[test]
    fn only_2xx_is_handed_back_as_a_bid_bearing_response() {
        let refused: [u16; 8] = [100, 101, 103, 300, 301, 302, 307, 308];
        for status in refused {
            let outcome = classify(
                Ok(BidderHttpResponse::new(status, vec![], BID_JSON.to_vec())),
                1024,
            );
            assert!(
                matches!(outcome, Err(FanoutError::BadServerResponse)),
                "status {status} cannot carry a bid, got {outcome:?}",
            );
        }
        for status in [200_u16, 204, 299] {
            let outcome = classify(
                Ok(BidderHttpResponse::new(status, vec![], BID_JSON.to_vec())),
                1024,
            );
            assert!(
                matches!(outcome, Ok(resp) if resp.status() == status),
                "status {status} is a bidder answer and passes through",
            );
        }
    }

    /// The oversize ceiling still outranks the status rule: a body past
    /// the cap is reported as oversize whatever status carried it, so
    /// the new guard has not reordered the taxonomy.
    #[test]
    fn oversize_outranks_the_status_refusal() {
        let outcome = classify(
            Ok(BidderHttpResponse::new(307, vec![], vec![0_u8; 4096])),
            100,
        );
        assert!(
            matches!(
                outcome,
                Err(FanoutError::OversizeResponse(
                    OversizeOrigin::CallResponseCap
                ))
            ),
            "the size ceiling is still read first, got {outcome:?}",
        );
    }

    #[test]
    fn transport_detail_not_on_wire() {
        let err = classify(Err(ProxyError::Transport(HOSTILE_DETAIL.to_owned())), 1024)
            .expect_err("a transport failure classifies as an error");
        assert_eq!(
            err.public_message(),
            "internal fanout error",
            "the wire carries the fixed category",
        );
        let debug = format!("{err:?}");
        for needle in SECRET_NEEDLES {
            assert!(
                !err.public_message().contains(needle),
                "public message leaked {needle}",
            );
            assert!(!debug.contains(needle), "debug rendering leaked {needle}");
        }
    }

    #[test]
    fn transport_detail_not_in_log() {
        testing_logger::setup();
        let outcome = classify(Err(ProxyError::Transport(HOSTILE_DETAIL.to_owned())), 1024);
        assert!(
            outcome.is_err(),
            "a transport failure classifies as an error"
        );
        testing_logger::validate(|captured| {
            let warns: Vec<&String> = captured
                .iter()
                .filter(|line| line.level == log::Level::Warn)
                .map(|line| &line.body)
                .collect();
            assert_eq!(
                warns.len(),
                1,
                "exactly one fanout warning per classified transport failure",
            );
            assert_eq!(
                warns[0], "bidder transport error",
                "the warning is the fixed category and nothing else",
            );
            for line in captured {
                for needle in SECRET_NEEDLES {
                    assert!(
                        !line.body.contains(needle),
                        "log line leaked {needle}: {}",
                        line.body,
                    );
                }
            }
        });
    }
}
