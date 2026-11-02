//! The configured node budget, enforced on the REAL bidder-response path.
//!
//! `auctioneer-bidder`'s own guards prove two things: that
//! `parse_bid_response` refuses an over-budget body, and that nothing
//! outside the crate can reach the bytes to parse them any other way.
//! Neither proves the budget is ATTACHED in production. A response the
//! transport builds carries a budget of 0, so the exchange has to put
//! the host's figure on it before an adapter runs — and if that seam
//! were missing, the unit guards would still pass while every real
//! auction refused every bid.
//!
//! So these rows drive `run_auction` with a canned fanout and assert on
//! the returned envelope:
//!
//! 1. A bidder response WITHIN the configured budget produces its bid.
//!    This is what fails if the exchange never attaches a budget: the
//!    transport-built 0 refuses everything.
//! 2. A bidder response OVER the configured budget is refused, and the
//!    refusal is the code-4 entry in `ext.errors`, not a bid.
//!
//! Together they pin the budget to the CONFIGURED figure rather than to
//! either hardcoded extreme: row 1 fails if the attached budget is 0,
//! row 2 fails if it is unlimited.
//!
//! The fixtures stay far inside `max_bidder_response_bytes`, so neither
//! row can be passing because of the byte cap.

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
    use auctioneer_openrtb::types::field::JsonField;
    use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare};
    use auctioneer_privacy::gcl::Gcl;
    use auctioneer_privacy::gvl::GvlStore;
    use auctioneer_runtime::{Clock as _, FakeClock, SeededRng};
    use std::collections::{BTreeMap, BTreeSet};
    use std::fmt::Write as _;
    use std::time::Duration;

    const IMP_CAP_NOT_UNDER_TEST: PrepareLimits = PrepareLimits {
        max_impressions: 100,
        max_copied_header_bytes: 4_096,
    };

    /// The node budget these rows measure against.
    ///
    /// Narrow enough that a modest fixture crosses it, so the over-budget
    /// body stays far inside `max_bidder_response_bytes` and cannot be
    /// refused by the byte cap instead.
    const NODE_BUDGET: u32 = 48;

    /// Code 4, `BadServerResponse` (Appendix G).
    const BAD_SERVER_RESPONSE: i64 = 4;

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
                .map(|_call| BidderCallResult {
                    // Built the way the transport builds one: NO node
                    // budget attached. Whatever budget the parse sees is
                    // the one the exchange put there, which is the whole
                    // point of these rows.
                    result: Ok(BidderHttpResponse::new(200, vec![], self.body.clone())),
                    elapsed: Duration::from_millis(50),
                })
                .collect())
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
                max_parsed_nodes: NODE_BUDGET,
            },
            ..HostConfig::default()
        }
    }

    fn leak_info() -> &'static BidderInfo {
        Box::leak(Box::new(BidderInfo {
            name: "fake_bb".into(),
            endpoint: "https://demo.example.test/openrtb".into(),
            uses_https: true,
            ..BidderInfo::default()
        }))
    }

    fn imp_for_fake_bb() -> Imp {
        let mut obj = serde_json::Map::new();
        obj.insert("fake_bb".to_owned(), serde_json::json!({}));
        Imp {
            id: "imp-1".into(),
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

    /// A schema-valid `BidResponse` whose one bid hangs `width` extra
    /// keys off `Bid`'s `#[serde(flatten)]` catch-all.
    ///
    /// Valid on purpose: an off-schema body would be refused by the
    /// parser rather than the budget, and the row would prove nothing.
    fn bid_response_body(width: usize) -> Vec<u8> {
        let filler = (0..width).fold(String::new(), |mut acc, key| {
            write!(acc, r#","k{key}":{key}"#).expect("a String write cannot fail");
            acc
        });
        format!(
            r#"{{"id":"r","seatbid":[{{"seat":"fake_bb","bid":[{{"id":"b1","impid":"imp-1","price":1.5,"w":300,"h":250{filler}}}]}}],"cur":"USD"}}"#
        )
        .into_bytes()
    }

    /// Drive one auction over a canned response body and hand back the
    /// envelope's seatbid count plus its serialized `ext`.
    fn run_with_body(body: Vec<u8>) -> (usize, String) {
        block_on(async {
            let mut registry = BidderRegistry::new();
            registry
                .register(leak_info(), Box::new(FakeBidder::default()))
                .expect("unique test bidder");

            let req = BidRequest {
                id: "auc-nodes".into(),
                imp: vec![imp_for_fake_bb()],
                cur: vec!["USD".into()],
                ..BidRequest::default()
            };
            let headers = RequestHeaders::default();
            let normalized = prepare(req, &headers, &IMP_CAP_NOT_UNDER_TEST)
                .expect("prepare succeeds on the minimal request");

            let host = host();
            let acct = AccountConfig::default();
            let resolved = ResolvedAccount {
                id: "unknown".into(),
                source: AccountSource::DefaultFallback,
                config: &acct,
            };
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            let rng = SeededRng::new(42);
            let fanout = CannedFanout { body };

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
            let resp = run_auction(&normalized, &ctx, &plan, &mut audit)
                .await
                .expect("run_auction succeeds");
            let ext = resp
                .ext
                .as_ref()
                .map(|value| serde_json::to_string(value).expect("ext serializes"))
                .unwrap_or_default();
            (resp.seatbid.len(), ext)
        })
    }

    /// A response inside the configured budget still produces its bid.
    ///
    /// The row that fails if the exchange never attaches a budget: a
    /// transport-built response carries 0, which refuses every body, so
    /// a missing seam shows up here as a lost bid rather than as
    /// silence.
    #[test]
    fn a_response_within_the_configured_budget_yields_its_bid() {
        let (seatbids, ext) = run_with_body(bid_response_body(0));
        assert_eq!(
            seatbids, 1,
            "a small valid bidder response must produce its seatbid; if this is 0 \
             the exchange is not attaching the node budget and the \
             transport-built 0 is refusing everything.\next={ext}",
        );
    }

    /// A response past the configured budget is refused as code 4.
    ///
    /// The row that fails if the attached budget is unlimited.
    #[test]
    fn a_response_past_the_configured_budget_is_refused_as_code_4() {
        let body = bid_response_body(80);
        assert!(
            body.len() < 0x0004_0000,
            "the fixture must stay far inside `max_bidder_response_bytes` so this \
             row cannot be passing because of the BYTE cap; got {} bytes",
            body.len(),
        );
        let (seatbids, ext) = run_with_body(body);
        assert_eq!(
            seatbids, 0,
            "an over-budget bidder response must not produce a bid.\next={ext}",
        );
        let parsed: serde_json::Value =
            serde_json::from_str(&ext).expect("the envelope ext is JSON");
        let code = parsed
            .pointer("/errors/fake_bb/0/code")
            .and_then(serde_json::Value::as_i64);
        assert_eq!(
            code,
            Some(BAD_SERVER_RESPONSE),
            "the refusal must surface as the code-4 `ext.errors` entry the spec \
             assigns an unparseable bidder response.\next={ext}",
        );
    }
}
