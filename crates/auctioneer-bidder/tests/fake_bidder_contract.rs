//! Exercise the synthetic bidder through production construction and parsing boundaries.

pub use auctioneer_bidder::test_support::fake_bidder;

#[cfg(test)]
mod tests {
    use super::fake_bidder::{FakeBidder, RequestMode, ResponseCoverage};
    use auctioneer_bidder::{
        Bidder as _, BidderHttpRequest, BidderHttpResponse, BidderName, BidderRequestInfo,
        CallBudget, EndpointRejectReason, EndpointValidator, EntryPoint, OutboundRequestSink,
        SinkLimits,
    };
    use auctioneer_config::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};
    use std::num::NonZeroUsize;

    struct Accept;
    impl EndpointValidator for Accept {
        fn validate(
            &self,
            _: &url::Url,
            _: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            Ok(())
        }
    }

    fn info() -> BidderRequestInfo<'static> {
        BidderRequestInfo::new(
            BidderName::new("fake_alias"),
            BidderName::new("fake_bidder"),
            "account",
            None,
            "https://bidder.example.test/bid",
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        )
    }

    fn request() -> BidRequest {
        serde_json::from_value(json!({
            "id":"request", "tmax":300_i32, "site":{"id":"site"},
            "imp":[
                {"id":"a", "banner":{}},
                {"id":"b", "video":{}},
                {"id":"c", "audio":{}},
                {"id":"d", "native":{}},
            ],
            "ext":{"sentinel":{"keep":[1_i32,2_i32]}},
            "unknown":{"keep":"verbatim"},
        }))
        .unwrap()
    }

    #[test]
    fn shared_fake_supports_aggregate_split_and_chunked_borrowed_construction() {
        for (request_mode, expected_groups) in [
            (RequestMode::Aggregate, vec![vec!["a", "b", "c", "d"]]),
            (
                RequestMode::Split,
                vec![vec!["a"], vec!["b"], vec!["c"], vec!["d"]],
            ),
            (
                RequestMode::Chunked(NonZeroUsize::new(2).unwrap()),
                vec![vec!["a", "b"], vec!["c", "d"]],
            ),
        ] {
            let bidder = FakeBidder {
                request_mode,
                ..FakeBidder::default()
            };
            let input = request();
            let before = serde_json::to_value(&input).unwrap();
            let context = info();
            let budget = CallBudget::new(4);
            let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 4), &budget, &Accept);
            bidder.make_requests(&input, &context, &mut sink).unwrap();
            let outcome = sink.into_outcome();
            assert!(outcome.errors.is_empty());
            assert_eq!(outcome.accepted.len(), expected_groups.len());
            for (call, ids) in outcome.accepted.iter().zip(expected_groups) {
                assert_eq!(call.covered_imp_ids().collect::<Vec<_>>(), ids);
                assert_eq!(call.method(), "POST");
                assert_eq!(call.url(), "https://bidder.example.test/bid");
                let mut expected = before.clone();
                expected["imp"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|imp| ids.contains(&imp["id"].as_str().unwrap()));
                assert_eq!(
                    serde_json::from_slice::<Value>(call.body()).unwrap(),
                    expected
                );
            }
            assert_eq!(serde_json::to_value(&input).unwrap(), before);
        }
    }

    #[test]
    fn scoped_response_mode_exposes_cross_call_bids_without_admitting_foreign_impressions() {
        let input = request();
        let info = info();
        let budget = CallBudget::new(4);
        let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 4), &budget, &Accept);
        FakeBidder {
            request_mode: RequestMode::Split,
            ..FakeBidder::default()
        }
        .make_requests(&input, &info, &mut sink)
        .unwrap();
        let calls = sink.into_outcome().accepted;
        let first = &calls[0];
        assert_eq!(first.covered_imp_ids().collect::<Vec<_>>(), ["a"]);
        let payload = json!({"id":"response", "seatbid":[{"bid":[
            {"id":"cross-call", "impid":"b", "price":1_i32, "mtype":2_i32},
            {"id":"foreign", "impid":"outside", "price":2_i32, "mtype":1_i32}
        ]}]});
        // Each comparison owns one independently admitted decode receipt.
        let response = || {
            BidderHttpResponse::new(200, vec![], serde_json::to_vec(&payload).unwrap())
                .within_node_budget(256)
        };
        let strict = FakeBidder::default()
            .make_bids(&input, first.meta(), &response())
            .unwrap_err();
        assert_eq!(strict.len(), 2);
        assert!(strict.iter().all(|error| error.code == 4));
        let exposed = FakeBidder {
            response_coverage: ResponseCoverage::ScopedRequest,
            ..FakeBidder::default()
        }
        .make_bids(&input, first.meta(), &response())
        .unwrap();
        assert_eq!(exposed.bids.len(), 1);
        assert_eq!(exposed.bids[0].bid.id, "cross-call");
        assert_eq!(exposed.bids[0].media_type, MediaType::Video);
        assert_eq!(exposed.errors.len(), 1);
        assert_eq!(exposed.errors[0].code, 4);
    }

    #[test]
    fn shared_fake_decodes_all_media_and_retains_valid_siblings_and_configs() {
        let bidder = FakeBidder::default();
        let input = request();
        let context = info();
        let budget = CallBudget::new(1);
        let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 4), &budget, &Accept);
        bidder.make_requests(&input, &context, &mut sink).unwrap();
        let sent = sink.into_outcome().accepted.pop().unwrap();
        let payload = json!({
            "id":"response", "cur":"EUR",
            "seatbid":[{"bid":[
                {"id":"banner", "impid":"a", "price":1.0_f64, "mtype":1_i32},
                {"id":"video", "impid":"b", "price":2.0_f64, "mtype":2_i32},
                {"id":"audio", "impid":"c", "price":3.0_f64, "mtype":3_i32},
                {"id":"native", "impid":"d", "price":4.0_f64, "mtype":4_i32},
                {"id":"bad", "impid":"a", "price":9.0_f64, "mtype":9_i32},
            ]}],
            "ext":{"auctionconfigs":[{"impid":"a", "config":{"payload":[true,null]}}]},
        });
        let response = BidderHttpResponse::new(200, vec![], serde_json::to_vec(&payload).unwrap())
            .within_node_budget(512);
        let decoded = bidder.make_bids(&input, sent.meta(), &response).unwrap();
        assert_eq!(decoded.currency, "EUR");
        assert_eq!(
            decoded
                .bids
                .iter()
                .map(|bid| bid.bid.id.as_str())
                .collect::<Vec<_>>(),
            ["banner", "video", "audio", "native"]
        );
        assert_eq!(
            decoded
                .bids
                .iter()
                .map(|bid| bid.media_type)
                .collect::<Vec<_>>(),
            [
                MediaType::Banner,
                MediaType::Video,
                MediaType::Audio,
                MediaType::Native
            ]
        );
        assert_eq!(
            decoded
                .errors
                .iter()
                .map(|error| error.code)
                .collect::<Vec<_>>(),
            [4]
        );
        assert_eq!(decoded.auction_configs.len(), 1);
        assert_eq!(decoded.auction_configs[0].imp_id, "a");
        assert_eq!(
            decoded.auction_configs[0].config,
            json!({"payload":[true,null]})
        );
        let unfunded = response.within_node_budget(0);
        assert_eq!(
            bidder
                .make_bids(&input, sent.meta(), &unfunded)
                .unwrap_err()[0]
                .code,
            4
        );
    }

    #[test]
    fn shared_fake_records_bounded_call_local_media_tokens_and_header() {
        let bidder = FakeBidder {
            request_mode: RequestMode::Chunked(NonZeroUsize::new(2).unwrap()),
            record_correlation: true,
            correlation_header: Some(b"fake-context"),
            ..FakeBidder::default()
        };
        let context = info();
        let budget = CallBudget::new(2);
        let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 4), &budget, &Accept);
        bidder
            .make_requests(&request(), &context, &mut sink)
            .unwrap();
        let outcome = sink.into_outcome();
        assert!(outcome.errors.is_empty());
        assert_eq!(outcome.accepted.len(), 2);
        for (call, expected_media) in outcome.accepted.iter().zip([
            [MediaType::Banner, MediaType::Video],
            [MediaType::Audio, MediaType::Native],
        ]) {
            assert_eq!(
                call.meta().correlation_header(),
                Some(b"fake-context".as_slice())
            );
            for (ordinal, media) in expected_media.into_iter().enumerate() {
                let row = call
                    .meta()
                    .imp_correlation(u16::try_from(ordinal).unwrap())
                    .unwrap();
                assert_eq!(MediaType::from(row.media()), media);
                assert_eq!(row.token(), u32::try_from(ordinal + 1).unwrap());
            }
            assert_eq!(call.meta().imp_correlation(2), None);
            assert!(!call.headers().any(|(key, _)| key == "fake-context"));
        }
    }

    #[test]
    fn shared_fake_filters_impressions_without_changing_call_coverage_or_caller_input() {
        for request_mode in [
            RequestMode::Aggregate,
            RequestMode::Split,
            RequestMode::Chunked(NonZeroUsize::new(2).unwrap()),
        ] {
            let bidder = FakeBidder {
                request_mode,
                reject_imp: Some("b"),
                ..FakeBidder::default()
            };
            let input = request();
            let before = serde_json::to_value(&input).unwrap();
            let context = info();
            let budget = CallBudget::new(4);
            let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 4), &budget, &Accept);
            bidder.make_requests(&input, &context, &mut sink).unwrap();
            let outcome = sink.into_outcome();
            assert!(outcome.errors.is_empty());
            assert_eq!(
                outcome
                    .accepted
                    .iter()
                    .flat_map(BidderHttpRequest::covered_imp_ids)
                    .collect::<Vec<_>>(),
                ["a", "c", "d"]
            );
            for call in &outcome.accepted {
                let body: Value = serde_json::from_slice(call.body()).unwrap();
                let ids = body["imp"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|imp| imp["id"].as_str().unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(ids, call.covered_imp_ids().collect::<Vec<_>>());
                assert_eq!(body["unknown"], json!({"keep":"verbatim"}));
                assert_eq!(body["ext"], before["ext"]);
            }
            assert_eq!(serde_json::to_value(&input).unwrap(), before);
        }
    }

    #[test]
    fn shared_fake_uses_reserved_slots_before_metadata_or_body_work() {
        use std::sync::atomic::Ordering;
        for slots in [0_u32, 1, 3, 4] {
            let bidder = FakeBidder {
                request_mode: RequestMode::Split,
                ..FakeBidder::default()
            };
            let context = info();
            let budget = CallBudget::new(slots);
            let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 4), &budget, &Accept);
            bidder
                .make_requests(&request(), &context, &mut sink)
                .unwrap();
            let outcome = sink.into_outcome();
            assert_eq!(outcome.accepted.len(), usize::try_from(slots).unwrap());
            assert_eq!(
                bidder.metadata_writes.load(Ordering::Relaxed),
                usize::try_from(slots).unwrap()
            );
            assert_eq!(bidder.written_bodies(), usize::try_from(slots).unwrap());
            assert_eq!(
                outcome.rejected_call_count,
                4_u32.checked_sub(slots).unwrap()
            );
        }
    }

    #[test]
    fn shared_fake_semantic_refusal_never_starts_construction() {
        let bidder = FakeBidder {
            semantic_error: true,
            ..FakeBidder::default()
        };
        let context = info();
        let budget = CallBudget::new(4);
        let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 4), &budget, &Accept);
        assert_eq!(
            bidder.make_requests(&request(), &context, &mut sink),
            Err(vec![auctioneer_bidder::BidderError::bad_input(
                "fake semantic refusal"
            )])
        );
        assert!(sink.into_outcome().accepted.is_empty());
        assert_eq!(budget.remaining(), 4);
        assert_eq!(bidder.written_bodies(), 0);
    }

    #[test]
    fn shared_fake_streams_endpoint_bindings_and_headers_through_metadata_policy() {
        let bidder = FakeBidder {
            method: "PUT",
            endpoint_values: &[("AccountID", "id /?&=#")],
            headers: &[("X-Synthetic", "marker"), ("Cookie", "fake=one")],
            ..FakeBidder::default()
        };
        let mut context = info();
        context.endpoint_template = "https://bidder.example.test/bid/{{.AccountID}}";
        context.endpoint_macros = &["AccountID"];
        let budget = CallBudget::new(1);
        let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 4), &budget, &Accept);
        bidder
            .make_requests(&request(), &context, &mut sink)
            .unwrap();
        let call = sink.into_outcome().accepted.pop().unwrap();
        assert_eq!(call.method(), "PUT");
        assert_eq!(
            call.url(),
            "https://bidder.example.test/bid/id%20%2F%3F%26%3D%23"
        );
        assert_eq!(
            call.headers().collect::<Vec<_>>(),
            [
                ("Content-Type", "application/json"),
                ("Accept", "application/json"),
                ("X-Synthetic", "marker"),
                ("Cookie", "fake=one")
            ]
        );

        let reserved = FakeBidder {
            headers: &[("Sec-GPC", "1")],
            ..FakeBidder::default()
        };
        let reserved_context = info();
        let reserved_budget = CallBudget::new(1);
        let mut reserved_sink =
            OutboundRequestSink::new(SinkLimits::new(4096, 4), &reserved_budget, &Accept);
        reserved
            .make_requests(&request(), &reserved_context, &mut reserved_sink)
            .unwrap();
        let refused = reserved_sink.into_outcome();
        assert!(refused.accepted.is_empty());
        assert_eq!(
            refused.errors,
            [auctioneer_bidder::OutboundBuildError::ReservedHeader {
                name: "Sec-GPC".into()
            }]
        );
        assert_eq!(reserved.written_bodies(), 0);
    }

    #[test]
    fn shared_fake_requests_use_real_gzip_and_exchange_header_finalization() {
        use auctioneer_bidder::{Compression, TransportFinalize, finalize_for_transport};
        use flate2::read::GzDecoder;
        use std::io::Read as _;

        let bidder = FakeBidder::default();
        let input = request();
        let context = info();
        let budget = CallBudget::new(1);
        let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 4), &budget, &Accept);
        bidder.make_requests(&input, &context, &mut sink).unwrap();
        let call = sink.into_outcome().accepted.pop().unwrap();
        let expected = call.body().to_vec();
        let (sent, finalized) = finalize_for_transport(
            call,
            &TransportFinalize {
                gpc: Some("1"),
                compression: Some(Compression::Gzip),
                max_encoded_bytes: 4096,
            },
        );
        let encoded = finalized.unwrap();
        assert_eq!(
            sent.covered_imp_ids().collect::<Vec<_>>(),
            ["a", "b", "c", "d"]
        );
        assert!(
            encoded
                .headers()
                .any(|(name, value)| name == "Content-Encoding" && value == "gzip")
        );
        assert!(
            encoded
                .headers()
                .any(|(name, value)| name == "Sec-GPC" && value == "1")
        );
        let mut decoded = Vec::new();
        GzDecoder::new(encoded.body())
            .read_to_end(&mut decoded)
            .unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn shared_fake_config_only_output_retains_null_and_valid_siblings_on_local_error() {
        let bidder = FakeBidder::default();
        let context = info();
        let input = request();
        let budget = CallBudget::new(1);
        let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 4), &budget, &Accept);
        bidder.make_requests(&input, &context, &mut sink).unwrap();
        let call = sink.into_outcome().accepted.pop().unwrap();
        let payload = json!({"id":"r", "ext":{"auctionconfigs":[
            {"impid":"a", "config":null},
            {"impid":17_i32, "config":{"reject":"sentinel"}},
            {"impid":"b", "config":{"keep":true}},
        ]}});
        let response = BidderHttpResponse::new(200, vec![], serde_json::to_vec(&payload).unwrap())
            .within_node_budget(128);
        let decoded = bidder.make_bids(&input, call.meta(), &response).unwrap();
        assert!(decoded.bids.is_empty());
        assert_eq!(decoded.currency, "USD");
        assert_eq!(
            decoded.errors,
            [auctioneer_bidder::BidderError::bad_server_response(
                "fake invalid configuration"
            )]
        );
        assert_eq!(decoded.auction_configs.len(), 2);
        assert_eq!(decoded.auction_configs[0].imp_id, "a");
        assert_eq!(decoded.auction_configs[0].config, Value::Null);
        assert_eq!(decoded.auction_configs[1].imp_id, "b");
        assert_eq!(decoded.auction_configs[1].config, json!({"keep":true}));
    }

    #[test]
    fn shared_fake_http_no_bid_and_parse_refusals_are_distinct_and_redacted() {
        let bidder = FakeBidder::default();
        let context = info();
        let input = request();
        let budget = CallBudget::new(1);
        let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 4), &budget, &Accept);
        bidder.make_requests(&input, &context, &mut sink).unwrap();
        let call = sink.into_outcome().accepted.pop().unwrap();
        for status in [400_u16, 401, 429, 500, 503] {
            let response =
                BidderHttpResponse::new(status, vec![], b"SECRET_RESPONSE_CANARY".to_vec())
                    .within_node_budget(128);
            let errors = bidder
                .make_bids(&input, call.meta(), &response)
                .unwrap_err();
            assert_eq!(
                errors,
                [auctioneer_bidder::BidderError::bad_server_response(
                    "fake HTTP refusal"
                )]
            );
            assert!(!format!("{errors:?}").contains("SECRET_RESPONSE_CANARY"));
        }
        for (status, body) in [(204_u16, b"ignored".as_slice()), (200, b"".as_slice())] {
            let response = BidderHttpResponse::new(status, vec![], body.to_vec());
            let decoded = bidder.make_bids(&input, call.meta(), &response).unwrap();
            assert!(decoded.bids.is_empty());
            assert!(decoded.errors.is_empty());
            assert!(decoded.auction_configs.is_empty());
        }
        let malformed =
            BidderHttpResponse::new(200, vec![], b"invalid JSON SECRET_RESPONSE_CANARY".to_vec())
                .within_node_budget(128);
        let errors = bidder
            .make_bids(&input, call.meta(), &malformed)
            .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 14);
        assert!(!format!("{errors:?}").contains("SECRET_RESPONSE_CANARY"));
    }

    #[test]
    fn shared_fake_identity_envelope_observes_the_real_factory_context_and_execution() {
        use super::fake_bidder::IdentityEnvelope;
        use std::sync::atomic::AtomicUsize;
        static EXECUTIONS: AtomicUsize = AtomicUsize::new(0);
        let bidder = FakeBidder {
            identity_envelope: Some(IdentityEnvelope {
                marker: "local-helper-marker",
                module: "synthetic::owner",
                constructor_count: 7,
                executions: &EXECUTIONS,
            }),
            ..FakeBidder::default()
        };
        let context = info();
        let input = request();
        for ordinal in [1_usize, 2] {
            let budget = CallBudget::new(1);
            let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 4), &budget, &Accept);
            bidder.make_requests(&input, &context, &mut sink).unwrap();
            let outcome = sink.into_outcome();
            assert!(outcome.errors.is_empty());
            assert_eq!(outcome.accepted.len(), 1);
            let call = &outcome.accepted[0];
            assert_eq!(
                serde_json::from_slice::<Value>(call.body()).unwrap(),
                json!({
                    "marker":"local-helper-marker", "public":"fake_alias", "core":"fake_bidder", "request_id":"request",
                })
            );
            assert!(
                call.headers().any(
                    |(name, value)| name == "X-Discovery-Module" && value == "synthetic::owner"
                )
            );
            assert!(
                call.headers()
                    .any(|(name, value)| name == "X-Discovery-Constructors" && value == "7")
            );
            assert!(
                call.headers()
                    .any(|(name, value)| name == "X-Discovery-Executions"
                        && value == ordinal.to_string())
            );
        }
    }

    #[test]
    fn shared_fake_applies_borrowed_query_modes_through_the_bounded_endpoint_builder() {
        use super::fake_bidder::QueryMode;
        let replacement = String::from("new /?&");
        let appended = [("new key", replacement.as_str())];
        for (query, expected) in [
            (
                QueryMode::Preserve,
                "https://bidder.example.test/bid?keep=yes&item=old&item=second",
            ),
            (
                QueryMode::Append(&appended),
                "https://bidder.example.test/bid?keep=yes&item=old&item=second&new+key=new+%2F%3F%26",
            ),
            (
                QueryMode::Set {
                    key: "item",
                    value: &replacement,
                },
                "https://bidder.example.test/bid?keep=yes&item=new+%2F%3F%26",
            ),
        ] {
            let bidder = FakeBidder {
                query,
                ..FakeBidder::default()
            };
            let mut context = info();
            context.endpoint_template =
                "https://bidder.example.test/bid?keep=yes&item=old&item=second";
            let budget = CallBudget::new(1);
            let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 4), &budget, &Accept);
            bidder
                .make_requests(&request(), &context, &mut sink)
                .unwrap();
            let call = sink.into_outcome().accepted.pop().unwrap();
            assert_eq!(call.url(), expected);
            assert_eq!(bidder.written_bodies(), 1);
        }
    }

    #[test]
    fn shared_fake_retains_nonfatal_diagnostics_without_losing_valid_calls() {
        use auctioneer_bidder::ConstructionDiagnostic;
        let bidder = FakeBidder {
            request_mode: RequestMode::Split,
            reject_imp: Some("b"),
            rejected_imp_diagnostic: Some(ConstructionDiagnostic::UnsupportedMediaType),
            ..FakeBidder::default()
        };
        let context = info();
        let budget = CallBudget::new(4);
        let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 4), &budget, &Accept);
        bidder
            .make_requests(&request(), &context, &mut sink)
            .unwrap();
        let outcome = sink.into_outcome();
        assert!(outcome.errors.is_empty());
        assert_eq!(outcome.accepted.len(), 3);
        assert_eq!(
            outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
            [ConstructionDiagnostic::UnsupportedMediaType]
        );
        assert_eq!(
            serde_json::to_value(&outcome.diagnostics).unwrap(),
            json!([{"code":2_u32,"message":"unsupported media type"}])
        );
    }

    #[test]
    fn shared_fake_advertises_supported_and_unsupported_debug_schemas_explicitly() {
        use auctioneer_bidder::DebugBodySchema;
        for debug_schema in [DebugBodySchema::OpenRtb, DebugBodySchema::Unsupported] {
            let bidder = FakeBidder {
                debug_schema,
                ..FakeBidder::default()
            };
            assert_eq!(bidder.debug_body_schema(), debug_schema);
        }
    }

    #[test]
    fn shared_fake_selects_only_reviewed_hosts_through_real_metadata_construction() {
        use auctioneer_bidder::{META_BIDDER_REGION_BYTES, OutboundBuildError};
        for (selected_host, expected_url) in [
            (Some("eu.example.test"), Some("https://eu.example.test/bid")),
            (Some("us.example.test"), Some("https://us.example.test/bid")),
            (None, None),
            (Some("rogue.example.test"), None),
        ] {
            let bidder = FakeBidder {
                selected_host,
                ..FakeBidder::default()
            };
            let mut context = info();
            context.endpoint_template = "https://{{.Host}}/bid";
            context.host_allowlist = Some(&["eu.example.test", "us.example.test"]);
            let budget = CallBudget::new(1);
            let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 4), &budget, &Accept);
            bidder
                .make_requests(&request(), &context, &mut sink)
                .unwrap();
            let outcome = sink.into_outcome();
            if let Some(expected) = expected_url {
                assert!(outcome.errors.is_empty());
                assert_eq!(outcome.accepted.len(), 1);
                assert_eq!(outcome.accepted[0].url(), expected);
                assert_eq!(bidder.written_bodies(), 1);
            } else {
                assert!(outcome.accepted.is_empty());
                assert_eq!(
                    outcome.errors,
                    [OutboundBuildError::MetaOverCap {
                        limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap(),
                    }]
                );
                assert_eq!(bidder.written_bodies(), 0);
            }
        }
    }

    #[test]
    fn shared_fake_requests_obey_bounded_debug_projection_and_unsupported_policy() {
        use auctioneer_bidder::{DebugBodySchema, DebugBodyStatus, project_request_body};
        let input: BidRequest = serde_json::from_value(json!({
            "id":"SECRET", "test":1_i32,
            "imp":[{"id":"SECRET_IMP", "banner":{"w":300_i32,"h":250_i32}}],
        }))
        .unwrap();
        for debug_schema in [DebugBodySchema::OpenRtb, DebugBodySchema::Unsupported] {
            let bidder = FakeBidder {
                debug_schema,
                ..FakeBidder::default()
            };
            let context = info();
            let budget = CallBudget::new(1);
            let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 4), &budget, &Accept);
            bidder.make_requests(&input, &context, &mut sink).unwrap();
            let outcome = sink.into_outcome();
            assert!(outcome.errors.is_empty());
            assert_eq!(outcome.accepted.len(), 1);
            let call = &outcome.accepted[0];
            let projection =
                project_request_body(bidder.debug_body_schema(), call.body(), call.headers(), 128);
            if debug_schema == DebugBodySchema::OpenRtb {
                assert_eq!(projection.body_status, DebugBodyStatus::Captured);
                assert_eq!(
                    serde_json::from_str::<Value>(projection.body.as_deref().unwrap()).unwrap(),
                    json!({
                        "id":"[REDACTED]", "test":1_i32,
                        "imp":[{"id":"[REDACTED]", "banner":{"w":300_i32,"h":250_i32}}],
                    })
                );
                assert!(!projection.body.unwrap().contains("SECRET"));
                let refused = project_request_body(
                    bidder.debug_body_schema(),
                    call.body(),
                    call.headers(),
                    1,
                );
                assert_eq!(refused.body_status, DebugBodyStatus::OmittedStructure);
                assert_eq!(refused.body, None);
            } else {
                assert_eq!(projection.body_status, DebugBodyStatus::OmittedType);
                assert_eq!(projection.body, None);
            }
            assert_eq!(
                serde_json::from_slice::<Value>(call.body()).unwrap(),
                serde_json::to_value(&input).unwrap()
            );
        }
    }
}
