//! Bidder response decoding through the shared boundary is node-budgeted.
//!
//! A bidder response is the one JSON body on the auction path that an
//! upstream the host does not control writes. `max_bidder_response_bytes`
//! bounds how many BYTES arrive; it does not bound what parsing them
//! retains. `{"a":{"a":…}}` and `[[[[…]]]]` both fit a few KiB while
//! expanding into one heap node per level, and `Bid` carries a
//! `#[serde(flatten)]` catch-all map, so the inflation does not even
//! need to be off-schema: a structurally VALID `BidResponse` can hang
//! arbitrary structure off every bid.
//!
//! So each row here feeds the synthetic bidder a body that is well-formed and
//! schema-valid but structurally larger than the budget, and requires a
//! REFUSAL. A row that merely asserted the decode succeeded under a
//! wide budget would pass against a raw `serde_json::from_slice`, which
//! is the defect these rows exist to catch.
//!
//! The refusal is code 4, `BadServerResponse` — Appendix G, "bidder
//! returned an unparseable response", the same class the spec assigns
//! `OversizeResponse`. Not code 14 `FailedToUnmarshal`: the body
//! unmarshals fine, and reporting a decode failure for a structural
//! refusal would send an operator reading the wrong error.
//!
//! Every row drives the real `Bidder::make_bids` through the trait, so
//! none of them can pass by exercising a helper the production path does
//! not call.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

#[cfg(test)]
mod tests {
    use super::fake_bidder::FakeBidder;
    use auctioneer_bidder::{
        Bidder as _, BidderHttpResponse, META_BYTES_CAPACITY, OutboundRequestMeta,
        OutboundRequestMetaBuilder,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use std::fmt::Write as _;

    /// Code 4, `BadServerResponse` (Appendix G).
    const BAD_SERVER_RESPONSE: u32 = 4;
    /// Code 14, `FailedToUnmarshal` (Appendix G) — the code a MALFORMED
    /// body raises, which a structural refusal must not be reported as.
    const FAILED_TO_UNMARSHAL: u32 = 14;

    /// Narrow enough that the oversize fixtures below cross it, wide
    /// enough that the small valid one does not.
    const TIGHT_BUDGET: u32 = 64;

    fn sent() -> OutboundRequestMeta {
        let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut builder = OutboundRequestMetaBuilder::new(&mut pool, 100);
        builder.set_method("POST").expect("fits");
        builder
            .set_url("https://demo.example.test/openrtb")
            .expect("fits");
        builder.imp_id("i").unwrap();
        for index in 0_u32..8 {
            builder.imp_id(&format!("i{index}")).unwrap();
        }
        builder.finish()
    }

    /// A 200 response carrying `body`, budgeted at `budget` nodes.
    ///
    /// Built the way the exchange builds one — `new` then
    /// `within_node_budget` — because the body is sealed and there is
    /// no struct literal to write.
    fn response(status: u16, body: Vec<u8>, budget: u32) -> BidderHttpResponse {
        BidderHttpResponse::new(status, vec![], body).within_node_budget(budget)
    }

    /// A schema-VALID `BidResponse` whose every bid hangs `width` extra
    /// keys off the `#[serde(flatten)]` catch-all.
    ///
    /// Valid on purpose: a bidder could refuse this by failing to
    /// parse it, and then the refusal would prove nothing about the
    /// node budget.
    fn valid_but_structurally_large(bids: usize, width: usize) -> Vec<u8> {
        let filler = (0..width).fold(String::new(), |mut acc, key| {
            write!(acc, r#","k{key}":{key}"#).expect("a String write cannot fail");
            acc
        });
        let seat_bids = (0..bids).fold(String::new(), |mut acc, bid| {
            if !acc.is_empty() {
                acc.push(',');
            }
            write!(
                acc,
                r#"{{"id":"b{bid}","impid":"i{bid}","price":1.5{filler}}}"#
            )
            .expect("a String write cannot fail");
            acc
        });
        format!(r#"{{"id":"r-1","seatbid":[{{"seat":"fake_s","bid":[{seat_bids}]}}],"cur":"USD"}}"#)
            .into_bytes()
    }

    /// A nesting chain under the catch-all: a few hundred BYTES, one
    /// retained node per level. This is the shape a byte cap cannot see.
    fn valid_but_deeply_nested(depth: usize) -> Vec<u8> {
        let chain = format!("{}1{}", r#"{"a":"#.repeat(depth), "}".repeat(depth));
        format!(
            r#"{{"id":"r-1","seatbid":[{{"seat":"fake_s","bid":[{{"id":"b","impid":"i","price":1.5,"deep":{chain}}}]}}],"cur":"USD"}}"#
        )
        .into_bytes()
    }

    fn request() -> BidRequest {
        let mut impressions = vec![serde_json::json!({"id": "i", "banner": {}})];
        impressions.extend((0_u32..8_u32).map(|index| {
            serde_json::json!({
                "id": format!("i{index}"), "banner": {},
            })
        }));
        serde_json::from_value(serde_json::json!({"id": "request", "imp": impressions})).unwrap()
    }

    #[test]
    fn flattened_catch_all_flood_is_refused_at_the_bidder_boundary() {
        // Wide first: the body IS a valid `BidResponse`, so a decode
        // that accepted it accepted an unbounded parse.
        let body = valid_but_structurally_large(8, 40);
        let bidder = FakeBidder::default();
        let wide = bidder
            .make_bids(&request(), &sent(), &response(200, body.clone(), 8_192))
            .expect("the same flood is schema-valid under a wide node budget");
        assert_eq!(wide.bids.len(), 8);
        assert!(wide.errors.is_empty());
        let errors = bidder
            .make_bids(&request(), &sent(), &response(200, body, TIGHT_BUDGET))
            .expect_err("an over-budget but schema-valid response must be refused");
        assert_eq!(
            errors.iter().map(|error| error.code).collect::<Vec<_>>(),
            vec![BAD_SERVER_RESPONSE],
        );
    }

    #[test]
    fn nesting_chain_under_the_catch_all_is_refused_at_the_bidder_boundary() {
        let body = valid_but_deeply_nested(100);
        assert!(
            body.len() < 1_024,
            "the fixture must stay small enough that a BYTE cap would admit it, \
             which is the whole asymmetry these rows cover; got {} bytes",
            body.len(),
        );
        let bidder = FakeBidder::default();
        let wide = bidder
            .make_bids(&request(), &sent(), &response(200, body.clone(), 8_192))
            .expect("the same nested body is schema-valid under a wide node budget");
        assert_eq!(wide.bids.len(), 1);
        assert!(wide.errors.is_empty());
        let errors = bidder
            .make_bids(&request(), &sent(), &response(200, body, TIGHT_BUDGET))
            .expect_err("a nesting chain admitted by the byte cap exceeds the node budget");
        assert_eq!(
            errors.iter().map(|error| error.code).collect::<Vec<_>>(),
            vec![BAD_SERVER_RESPONSE],
        );
    }

    #[test]
    fn an_unbudgeted_response_refuses_every_body() {
        // The fail-closed property the boundary rests on. A
        // transport-built `BidderHttpResponse` has no budget, and
        // `new` leaves it at 0 rather than at "unlimited", so a path
        // that forgot to attach one cannot parse at all instead of
        // parsing unbounded. Without this, a missed
        // `within_node_budget` would be invisible: the response would
        // decode fine and the bound would simply not be there.
        let body = valid_but_structurally_large(1, 0);
        let unbudgeted = BidderHttpResponse::new(200, vec![], body.clone());
        assert_eq!(
            unbudgeted.max_parsed_nodes(),
            0,
            "a transport-built response must start with no parse budget",
        );
        let errs = FakeBidder::default()
            .make_bids(&request(), &sent(), &unbudgeted)
            .expect_err("an unbudgeted response must not parse");
        assert_eq!(
            errs.iter().map(|err| err.code).collect::<Vec<_>>(),
            vec![BAD_SERVER_RESPONSE],
        );

        // The SAME body, once a budget is attached, decodes — so the
        // refusal above is the missing budget and not a broken fixture.
        let budgeted = BidderHttpResponse::new(200, vec![], body).within_node_budget(TIGHT_BUDGET);
        let decoded = FakeBidder::default()
            .make_bids(&request(), &sent(), &budgeted)
            .expect("the same body decodes once a budget is attached");
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(decoded.bids[0].bid.id, "b0");
        assert_eq!(decoded.bids[0].bid.impid, "i0");
    }

    #[test]
    fn a_within_budget_response_still_decodes() {
        // The complement: the budget refuses structure, not every
        // response. Without this row the two above would pass against
        // a bidder that refused unconditionally.
        let body = valid_but_structurally_large(1, 0);
        let decoded = FakeBidder::default()
            .make_bids(&request(), &sent(), &response(200, body, TIGHT_BUDGET))
            .expect("a small valid response decodes under the same budget");
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(decoded.bids[0].bid.id, "b0");
        assert_eq!(decoded.bids[0].bid.impid, "i0");
        assert!((decoded.bids[0].bid.price - 1.5).abs() < f64::EPSILON);
        assert_eq!(decoded.currency, "USD");
    }

    #[test]
    fn a_malformed_body_still_reports_failed_to_unmarshal() {
        // The budget must not swallow the ordinary decode failure: a
        // structural refusal and a malformed body are different
        // operator problems and keep different codes.
        let errs = FakeBidder::default()
            .make_bids(
                &BidRequest::default(),
                &sent(),
                &response(200, b"{not json".to_vec(), TIGHT_BUDGET),
            )
            .expect_err("a malformed body is refused");
        assert_eq!(
            errs.iter().map(|err| err.code).collect::<Vec<_>>(),
            vec![FAILED_TO_UNMARSHAL],
        );
    }

    #[test]
    fn the_pre_parse_early_exits_are_unchanged() {
        // A budget so narrow that ANY parse would be refused, so a row
        // that returns successfully here proves the exit ran BEFORE the
        // parse.
        let request = BidRequest::default();
        let meta = sent();

        for (label, status, body) in [
            ("204 no content", 204_u16, vec![]),
            ("204 ignores a malformed body", 204, b"not json".to_vec()),
            ("empty body", 200, vec![]),
        ] {
            let empty = response(status, body, 0);
            let decoded = FakeBidder::default()
                .make_bids(&request, &meta, &empty)
                .unwrap_or_else(|errs| panic!("{label} must return before parsing, got {errs:?}"));
            assert!(decoded.bids.is_empty(), "{label}");
        }

        for status in [400, 500] {
            for budget in [0, TIGHT_BUDGET] {
                for body in [vec![], b"oops".to_vec()] {
                    let over_400 = response(status, body, budget);
                    let errs = FakeBidder::default()
                        .make_bids(&request, &meta, &over_400)
                        .expect_err("an error status is refused, even with an empty body");
                    assert_eq!(
                        errs.iter().map(|err| err.code).collect::<Vec<_>>(),
                        vec![BAD_SERVER_RESPONSE],
                        "an error status must report code 4 before parsing; a malformed \
                         body under a nonzero budget would instead report code 14",
                    );
                    assert!(errs.iter().all(|error| !error.message.is_empty()));
                }
            }
        }
    }
}
