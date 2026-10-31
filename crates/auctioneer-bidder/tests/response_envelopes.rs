//! Sealed-response receipt controls and standard-envelope compatibility.

#[cfg(test)]
mod tests {
    use auctioneer_bidder::test_support::fake_bidder::FakeBidder;
    use auctioneer_bidder::{
        Bidder as _, BidderError, BidderErrorKind, BidderHttpResponse, DebugBodyEligibility,
        META_BYTES_CAPACITY, OutboundRequestMeta, OutboundRequestMetaBuilder,
    };
    use auctioneer_config::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::json;

    const FAKE_DECODE_EXHAUSTED: &str = "bidder response exceeded the decode budget";

    fn fake_standard() -> Vec<u8> {
        serde_json::to_vec(&json!({"id": "fake_response", "seatbid": []})).unwrap()
    }

    fn fake_response(body: &[u8], nodes: u32) -> BidderHttpResponse {
        BidderHttpResponse::new(200, Vec::new(), body.to_vec()).within_node_budget(nodes)
    }

    fn assert_fake_parse_error(errors: &[BidderError], code: u32, message: &str) {
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, code);
        assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
        assert_eq!(errors[0].message, message);
    }

    fn fake_request() -> BidRequest {
        serde_json::from_value(json!({
            "id": "fake_request",
            "imp": [{"id": "fake_imp", "banner": {}}]
        }))
        .unwrap()
    }

    fn fake_sent() -> OutboundRequestMeta {
        let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut builder = OutboundRequestMetaBuilder::new(&mut pool, 4);
        builder.set_method("POST").unwrap();
        builder.set_url("https://fake.example.test/bid").unwrap();
        builder.imp_id("fake_imp").unwrap();
        builder.finish()
    }

    #[test]
    fn fake_standard_wrapper_cannot_retain_a_second_tree() {
        // A generous node cap must not authorize replaying the delivered body.
        let response = fake_response(&fake_standard(), 4_096);
        let first = response.parse_bid_response().unwrap();
        let errors = response.parse_bid_response().unwrap_err();
        assert_fake_parse_error(&errors, 4, FAKE_DECODE_EXHAUSTED);
        assert_eq!(first.id, "fake_response");
        assert!(first.seatbid.is_empty());
    }

    #[test]
    fn fake_candidate_extension_failure_keeps_other_seats_and_parse_provenance() {
        let body = serde_json::to_vec(&json!({"id":"fake_response", "cur":"EUR",
            "seatbid":[
                {"bid":[{"id":"fake_bad", "impid":"fake_imp", "price":1_i32,
                    "ext":{"prebid":{"type":false}}}]},
                {"bid":[{"id":"fake_good", "impid":"fake_imp", "price":2_i32}]}
            ], "ext":{"fake_config":true}}))
        .unwrap();
        let response = fake_response(&body, 256);
        let (parsed, errors) = response.parse_bid_candidates().unwrap();
        assert_eq!(parsed.cur, "EUR");
        assert!(parsed.seatbid[0].bid.is_empty());
        assert_eq!(parsed.seatbid[1].bid[0].id, "fake_good");
        assert_eq!(parsed.ext, Some(json!({"fake_config":true})));
        assert_fake_parse_error(&errors, 14, "invalid bidder response");
        assert_fake_parse_error(
            &response.parse_bid_response().unwrap_err(),
            4,
            FAKE_DECODE_EXHAUSTED,
        );
    }

    #[test]
    fn fake_candidate_standard_failure_is_envelope_fatal_even_with_extension_siblings() {
        for invalid in [
            json!({"id":"fake_bad", "impid":"fake_imp", "price":"fake_secret"}),
            json!({"id":"fake_bad", "price":1_i32}),
            json!(null),
        ] {
            let body = serde_json::to_vec(&json!({"id":"fake_response", "seatbid":[
                {"bid":[{"id":"fake_good", "impid":"fake_imp", "price":1_i32},
                    {"id":"fake_bad_ext", "impid":"fake_imp", "price":1_i32, "ext":false}]},
                {"bid":[invalid]}
            ]}))
            .unwrap();
            let response = fake_response(&body, 256);
            assert_fake_parse_error(
                &response.parse_bid_candidates().unwrap_err(),
                14,
                "invalid bidder response",
            );
            assert_fake_parse_error(
                &response.parse_bid_candidates().unwrap_err(),
                4,
                FAKE_DECODE_EXHAUSTED,
            );
        }
    }

    #[test]
    fn fake_candidate_and_standard_parsers_share_one_receipt_in_both_orders() {
        for candidates_first in [false, true] {
            let response = fake_response(&fake_standard(), 64);
            if candidates_first {
                drop(response.parse_bid_candidates().unwrap());
            } else {
                drop(response.parse_bid_response().unwrap());
            }
            let reattached = response.within_node_budget(u32::MAX);
            assert_eq!(reattached.max_parsed_nodes(), 64);
            assert_fake_parse_error(
                &reattached.parse_bid_candidates().unwrap_err(),
                4,
                FAKE_DECODE_EXHAUSTED,
            );
            assert_fake_parse_error(
                &reattached.parse_bid_response().unwrap_err(),
                4,
                FAKE_DECODE_EXHAUSTED,
            );
        }
    }

    #[test]
    fn fake_candidate_source_guard_and_rollback_are_separate_from_standard_parser() {
        for depth in [63_usize, 64, 65] {
            let mut nested = json!(null);
            for _ in 1..depth {
                nested = json!([nested]);
            }
            let body = serde_json::to_vec(&json!({"id":"fake_response", "ext":nested})).unwrap();
            let response = fake_response(&body, 256);
            if depth <= 64 {
                assert!(response.parse_bid_candidates().unwrap().1.is_empty());
            } else {
                assert_fake_parse_error(
                    &response.parse_bid_candidates().unwrap_err(),
                    4,
                    FAKE_DECODE_EXHAUSTED,
                );
            }
        }
        let mut body = serde_json::to_vec(&json!({"id":"fake_response", "seatbid":[{"bid":[
            {"id":"fake_good", "impid":"fake_imp", "price":1_i32},
            {"id":"fake_bad", "impid":"fake_imp", "price":1_i32, "ext":false}
        ]}]}))
        .unwrap();
        body.extend_from_slice(b" trailing");
        assert_fake_parse_error(
            &fake_response(&body, 256)
                .parse_bid_candidates()
                .unwrap_err(),
            14,
            "invalid bidder response",
        );
    }

    #[test]
    fn fake_candidate_admission_uses_the_attached_source_cap_and_spends_failed_attempts() {
        let body = serde_json::to_vec(&json!({"id":"fake_response", "seatbid":[{"bid":[
            {"id":"fake_good", "impid":"fake_imp", "price":1_i32}
        ]}]}))
        .unwrap();
        let exact = auctioneer_runtime::count_json_nodes(&body, 256).unwrap();
        assert_eq!(
            fake_response(&body, exact)
                .parse_bid_candidates()
                .unwrap()
                .0
                .seatbid[0]
                .bid
                .len(),
            1
        );
        for cap in [0, exact.checked_sub(1).unwrap()] {
            let response = fake_response(&body, cap);
            assert_fake_parse_error(
                &response.parse_bid_candidates().unwrap_err(),
                4,
                FAKE_DECODE_EXHAUSTED,
            );
            let reattached = response.within_node_budget(u32::MAX);
            assert_eq!(reattached.max_parsed_nodes(), cap);
            assert_fake_parse_error(
                &reattached.parse_bid_response().unwrap_err(),
                4,
                FAKE_DECODE_EXHAUSTED,
            );
        }
    }

    #[test]
    fn fake_candidate_reserved_raw_capture_is_structural_not_an_extension_type_error() {
        let body = serde_json::to_vec(&json!({"id":"fake_response", "seatbid":[{"bid":[
            {"id":"fake_good", "impid":"fake_imp", "price":1_i32},
            {"id":"fake_bad", "impid":"fake_imp", "price":1_i32,
                "ext":{"fake_unknown":{"$serde_json::private::RawValue":"[0,0,0]"}}}
        ]}]}))
        .unwrap();
        assert_fake_parse_error(
            &fake_response(&body, 256)
                .parse_bid_candidates()
                .unwrap_err(),
            4,
            FAKE_DECODE_EXHAUSTED,
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn fake_concurrent_candidate_and_standard_borrowers_share_one_claim() {
        use std::{
            sync::{Arc, Barrier},
            thread,
        };

        let response = Arc::new(fake_response(&fake_standard(), 256));
        let start = Arc::new(Barrier::new(4));
        let threads: Vec<_> = (0_usize..4)
            .map(|index| {
                let worker_response = Arc::clone(&response);
                let worker_start = Arc::clone(&start);
                thread::spawn(move || {
                    worker_start.wait();
                    if index.is_multiple_of(2) {
                        worker_response
                            .parse_bid_candidates()
                            .map(|(parsed, errors)| {
                                assert!(errors.is_empty());
                                parsed
                            })
                    } else {
                        worker_response.parse_bid_response()
                    }
                })
            })
            .collect();
        let mut successes = 0_usize;
        for thread in threads {
            match thread.join().unwrap() {
                Ok(parsed) => {
                    assert_eq!(parsed.id, "fake_response");
                    successes += 1;
                }
                Err(errors) => assert_fake_parse_error(&errors, 4, FAKE_DECODE_EXHAUSTED),
            }
        }
        assert_eq!(successes, 1);
    }

    #[test]
    fn fake_reserved_object_keys_cannot_expand_a_sealed_response() {
        let encoded = json!(vec![0_u32; 512]).to_string();
        let hidden = json!({"$serde_json::private::RawValue": encoded});
        for body in [
            json!({"id":"fake_response","ext":hidden}),
            json!({"id":"fake_response","seatbid":[{"bid":[{
                "id":"fake_bid","impid":"fake_imp","price":1.0_f64,
                "ext":{"fake_extension":hidden}
            }]}]}),
        ] {
            let wire = serde_json::to_vec(&body).unwrap();
            let source_nodes = auctioneer_runtime::count_json_nodes(&wire, 64).unwrap();
            assert!(source_nodes <= 64);
            let response = fake_response(&wire, 64);
            assert_fake_parse_error(
                &response.parse_bid_response().unwrap_err(),
                4,
                "bidder response exceeded the JSON node budget",
            );
            assert_fake_parse_error(
                &response.parse_bid_response().unwrap_err(),
                4,
                FAKE_DECODE_EXHAUSTED,
            );
        }
    }

    #[test]
    fn fake_dropping_a_tree_does_not_refund_the_decode_receipt() {
        let response = fake_response(&fake_standard(), 4_096);
        drop(response.parse_bid_response().unwrap());
        let errors = response.parse_bid_response().unwrap_err();
        assert_fake_parse_error(&errors, 4, FAKE_DECODE_EXHAUSTED);
    }

    #[test]
    fn fake_attaching_limits_cannot_renew_a_spent_receipt() {
        for replacement in [5, 64, u32::MAX] {
            let response = fake_response(&fake_standard(), 64);
            let first = response.parse_bid_response().unwrap();
            let reattached = response.within_node_budget(replacement);
            let errors = reattached.parse_bid_response().unwrap_err();
            assert_fake_parse_error(&errors, 4, FAKE_DECODE_EXHAUSTED);
            assert_eq!(first.id, "fake_response");
        }
    }

    #[test]
    fn fake_attaching_limits_after_an_unfunded_attempt_cannot_rearm_decoding() {
        let response = BidderHttpResponse::new(200, Vec::new(), fake_standard());
        assert_fake_parse_error(
            &response.parse_bid_response().unwrap_err(),
            4,
            "bidder response exceeded the JSON node budget",
        );
        let reattached = response.within_node_budget(64);
        assert_fake_parse_error(
            &reattached.parse_bid_response().unwrap_err(),
            4,
            FAKE_DECODE_EXHAUSTED,
        );
    }

    #[test]
    fn fake_structural_failure_cannot_be_retried_with_a_larger_limit() {
        let response = fake_response(&fake_standard(), 4);
        assert_fake_parse_error(
            &response.parse_bid_response().unwrap_err(),
            4,
            "bidder response exceeded the JSON node budget",
        );
        let reattached = response.within_node_budget(4_096);
        assert_fake_parse_error(
            &reattached.parse_bid_response().unwrap_err(),
            4,
            FAKE_DECODE_EXHAUSTED,
        );
    }

    #[test]
    fn fake_failed_syntax_decode_spends_the_shared_receipt() {
        // Deliberately incomplete JSON cannot be constructed with json!.
        let response = fake_response(br#"{"id":"fake_upstream_secret", "#, 64);
        assert_fake_parse_error(
            &response.parse_bid_response().unwrap_err(),
            14,
            "invalid bidder response",
        );
        assert_fake_parse_error(
            &response.parse_bid_response().unwrap_err(),
            4,
            FAKE_DECODE_EXHAUSTED,
        );
    }

    #[test]
    fn fake_failed_typed_decode_spends_the_shared_receipt() {
        let body = serde_json::to_vec(&json!({
            "id": "fake_response", "seatbid": "fake_upstream_secret"
        }))
        .unwrap();
        let response = fake_response(&body, 64);
        assert_fake_parse_error(
            &response.parse_bid_response().unwrap_err(),
            14,
            "invalid bidder response",
        );
        assert_fake_parse_error(
            &response.parse_bid_response().unwrap_err(),
            4,
            FAKE_DECODE_EXHAUSTED,
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn fake_concurrent_borrows_cannot_duplicate_the_funded_tree() {
        use std::{
            iter::repeat_with,
            sync::{Arc, Barrier},
            thread,
        };

        let response = Arc::new(fake_response(&fake_standard(), 4_096));
        let barrier = Arc::new(Barrier::new(4));
        let threads: Vec<_> = repeat_with(|| {
            let borrowed = Arc::clone(&response);
            let start = Arc::clone(&barrier);
            thread::spawn(move || {
                start.wait();
                borrowed.parse_bid_response()
            })
        })
        .take(4)
        .collect();
        let mut retained = Vec::new();
        let mut refused = 0_usize;
        for thread in threads {
            match thread.join().unwrap() {
                Ok(parsed) => retained.push(parsed),
                Err(errors) => {
                    assert_fake_parse_error(&errors, 4, FAKE_DECODE_EXHAUSTED);
                    refused += 1;
                }
            }
        }
        assert_eq!(retained.len(), 1);
        assert_eq!(refused, 3);
        assert_eq!(retained[0].id, "fake_response");
    }

    #[test]
    fn fake_limits_attached_before_decoding_fund_the_first_exact_parse() {
        // Root, two keys, a string and an empty array are five source nodes.
        let response = fake_response(&fake_standard(), 5);
        let parsed = response.parse_bid_response().unwrap();
        assert_eq!(parsed.id, "fake_response");
        assert!(parsed.seatbid.is_empty());
    }

    #[test]
    fn fake_standard_parser_remains_whole_response_typed() {
        let body = serde_json::to_vec(&json!({
            "id": "fake_response",
            "seatbid": [
                {"seat": "fake_first", "bid": [
                    {"id": "fake_good_first", "impid": "fake_imp", "price": 1.0_f64}
                ]},
                {"seat": "fake_bad", "bid": [
                    {"id": "fake_bad_typed", "impid": "fake_imp", "price": "fake_secret"}
                ]},
                {"seat": "fake_last", "bid": [
                    {"id": "fake_good_last", "impid": "fake_imp", "price": 2.0_f64}
                ]}
            ]
        }))
        .unwrap();
        let errors = FakeBidder::default()
            .make_bids(&fake_request(), &fake_sent(), &fake_response(&body, 256))
            .unwrap_err();
        assert_fake_parse_error(&errors, 14, "invalid bidder response");
    }

    #[test]
    fn fake_standard_semantic_failure_keeps_good_siblings_in_other_seats() {
        let body = serde_json::to_vec(&json!({
            "id": "fake_response",
            "cur": "USD",
            "seatbid": [
                {"seat": "fake_first", "bid": [
                    {"id": "fake_good_first", "impid": "fake_imp", "price": 1.0_f64}
                ]},
                {"seat": "fake_bad", "bid": [
                    {"id": "fake_bad_semantic", "impid": "fake_unknown_imp", "price": 1.0_f64}
                ]},
                {"seat": "fake_last", "bid": [
                    {"id": "fake_good_last", "impid": "fake_imp", "price": 2.0_f64}
                ]}
            ]
        }))
        .unwrap();
        let decoded = FakeBidder::default()
            .make_bids(&fake_request(), &fake_sent(), &fake_response(&body, 256))
            .unwrap();
        assert_eq!(decoded.currency, "USD");
        assert_eq!(
            decoded
                .bids
                .iter()
                .map(|bid| bid.bid.id.as_str())
                .collect::<Vec<_>>(),
            ["fake_good_first", "fake_good_last"],
        );
        assert!(
            decoded
                .bids
                .iter()
                .all(|bid| bid.media_type == MediaType::Banner)
        );
        assert_eq!(decoded.errors.len(), 1);
        assert_eq!(decoded.errors[0].code, 4);
        assert_eq!(decoded.errors[0].kind, BidderErrorKind::Other);
        assert_eq!(
            decoded.errors[0].message,
            "fake invalid bid media or coverage"
        );
    }

    #[test]
    fn fake_proprietary_nesting_guard_does_not_narrow_standard_parsing() {
        let mut chain = json!(0_u64);
        for _ in 0..65_usize {
            chain = json!([chain]);
        }
        let body = serde_json::to_vec(&json!({
            "id": "fake_response", "seatbid": [], "ext": chain
        }))
        .unwrap();
        let response = fake_response(&body, 256);
        let parsed = response.parse_bid_response().unwrap();
        let mut current = parsed.ext.as_ref().unwrap();
        for _ in 0..65_usize {
            current = &current.as_array().unwrap()[0];
        }
        assert_eq!(current.as_u64(), Some(0));
    }

    #[test]
    fn fake_decoding_keeps_status_headers_and_original_media_evidence() {
        let body = fake_standard();
        let headers = vec![
            ("content-type".to_owned(), "application/json".to_owned()),
            ("x-fake".to_owned(), "fake_first".to_owned()),
            ("x-fake".to_owned(), "fake_second".to_owned()),
        ];
        let response = BidderHttpResponse::from_transport(
            201,
            headers.clone(),
            body.clone(),
            DebugBodyEligibility::SingleTextValue,
        )
        .within_node_budget(5);
        let original_debug = format!("{response:?}");
        assert_eq!(response.parse_bid_response().unwrap().id, "fake_response");
        assert_eq!(response.status(), 201);
        assert_eq!(response.headers(), headers);
        assert_eq!(response.max_parsed_nodes(), 5);
        assert!(response.body_equals(&body));
        assert_eq!(format!("{response:?}"), original_debug);
        assert_eq!(response.into_body(), body);
    }

    #[test]
    fn fake_sealed_response_preserves_send_sync_and_equality_traits() {
        fn assert_traits<T: Send + Sync + PartialEq + Eq>() {}

        assert_traits::<BidderHttpResponse>();
        let body = fake_standard();
        let first = fake_response(&body, 64);
        let second = fake_response(&body, 64);
        assert_eq!(first, second);
        first.parse_bid_response().unwrap();
        assert_eq!(first, second);
        second.parse_bid_response().unwrap();
        assert_eq!(first, second);
    }
}
