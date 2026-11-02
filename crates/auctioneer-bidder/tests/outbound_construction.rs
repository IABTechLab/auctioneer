#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

#[cfg(test)]
mod tests {
    use super::fake_bidder::FakeBidder;
    use auctioneer_bidder::{
        Bidder as _, BidderHttpRequest, BidderName, BidderRequestInfo, CallBudget, Compression,
        EndpointRejectReason, EndpointValidator, EntryPoint, FinalizeError,
        META_BIDDER_REGION_BYTES, OutboundBuildError, OutboundRequestSink, SinkLimits,
        TransportFinalize, finalize_for_transport,
    };
    use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, Imp};
    use std::cell::Cell;
    use std::sync::Arc;

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
            BidderName::new("fake_synthetic"),
            BidderName::new("fake_synthetic"),
            "account",
            None,
            "https://example.test/bid",
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        )
    }

    fn request(body: &[u8]) -> BidderHttpRequest {
        let budget = CallBudget::new(1);
        let mut sink = OutboundRequestSink::new(SinkLimits::new(body.len(), 100), &budget, &Accept);
        sink.push_request(
            &info(),
            |meta| {
                meta.set_method("POST")?;
                meta.set_url("https://example.test/bid")?;
                meta.header("X-Context", "kept")?;
                meta.imp_id("imp-1")
            },
            |writer| writer.write_all(body),
        )
        .unwrap();
        sink.into_outcome().accepted.pop().unwrap()
    }

    #[test]
    fn count_cap_never_runs_either_closure_and_counts_every_attempt() {
        let budget = CallBudget::new(0);
        let mut sink = OutboundRequestSink::new(SinkLimits::new(1024, 100), &budget, &Accept);
        for _ in 0_usize..23 {
            let _rejection =
                sink.push_request(&info(), |_| panic!("metadata ran"), |_| panic!("body ran"));
        }
        let out = sink.into_outcome();
        assert_eq!(out.rejected_call_count, 23);
        assert_eq!(
            out.errors,
            [OutboundBuildError::CallBudgetExhausted { max_calls: 0 }]
        );
    }

    #[test]
    fn metadata_and_body_stay_in_their_original_allocations() {
        let request = request(b"payload");
        let method = request.method().as_ptr();
        let url = request.url().as_ptr();
        assert_eq!(url, method.wrapping_add(4), "method and URL share one pool");
        let body = request.body().as_ptr();
        let (sent, finalized) = finalize_for_transport(
            request,
            &TransportFinalize {
                gpc: Some("1"),
                compression: None,
                max_encoded_bytes: 7,
            },
        );
        let (meta, bytes) = finalized.unwrap().into_parts();
        assert!(Arc::ptr_eq(&sent, &meta));
        assert_eq!(meta.method().as_ptr(), method);
        assert_eq!(meta.url().as_ptr(), url);
        assert_eq!(bytes.as_ptr(), body);
        assert_eq!(
            sent.headers().collect::<Vec<_>>(),
            [("X-Context", "kept"), ("Sec-GPC", "1")]
        );
        assert_eq!(sent.covered_imp_ids().collect::<Vec<_>>(), ["imp-1"]);
    }

    #[test]
    fn call_correlation_stays_with_the_same_metadata_on_gzip_success_and_failure() {
        use auctioneer_bidder::correlation::CorrelationMedia;

        for encoded_cap in [0, 1024] {
            let budget = CallBudget::new(1);
            let mut sink = OutboundRequestSink::new(SinkLimits::new(32, 2), &budget, &Accept);
            sink.push_request(
                &info(),
                |meta| {
                    meta.set_method("POST")?;
                    meta.set_url("https://example.test/bid")?;
                    meta.imp_id("imp-1")?;
                    meta.set_correlation_header(b"private-protocol-context")?;
                    meta.correlate_imp(0, CorrelationMedia::Video, 42)
                },
                |body| body.write_all(b"payload"),
            )
            .unwrap();
            let request = sink.into_outcome().accepted.pop().unwrap();
            let pointer = request.url().as_ptr();
            let (sent, result) = finalize_for_transport(
                request,
                &TransportFinalize {
                    gpc: Some("1"),
                    compression: Some(Compression::Gzip),
                    max_encoded_bytes: encoded_cap,
                },
            );
            assert_eq!(sent.url().as_ptr(), pointer);
            assert_eq!(
                sent.correlation_header(),
                Some(b"private-protocol-context".as_slice())
            );
            assert_eq!(
                sent.imp_correlation(0).unwrap().media(),
                CorrelationMedia::Video
            );
            assert_eq!(sent.imp_correlation(0).unwrap().token(), 42);
            assert!(sent.headers().all(|(name, value)| {
                !name.contains("private-protocol-context")
                    && !value.contains("private-protocol-context")
            }));
            if encoded_cap == 0 {
                assert_eq!(
                    result.unwrap_err(),
                    FinalizeError::OversizeEncoded { limit: 0 }
                );
            } else {
                let (transport_meta, body) = result.unwrap().into_parts();
                assert!(Arc::ptr_eq(&sent, &transport_meta));
                assert!(!body.is_empty());
                assert!(
                    sent.headers()
                        .any(|entry| entry == ("Content-Encoding", "gzip"))
                );
            }
        }
    }

    #[test]
    fn full_bidder_region_and_header_table_leave_the_reserved_finalizer_tail() {
        let budget = CallBudget::new(1);
        let mut sink = OutboundRequestSink::new(SinkLimits::new(1, 0), &budget, &Accept);
        sink.push_request(
            &info(),
            |meta| {
                meta.set_method("POST")?;
                meta.set_url("https://example.test/bid")?;
                for _ in 0_usize..31 {
                    meta.header("x", "")?;
                }
                let padding = "v"
                    .repeat(META_BIDDER_REGION_BYTES - 4 - "https://example.test/bid".len() - 32);
                meta.header("y", &padding)
            },
            |writer| writer.write_all(b"x"),
        )
        .unwrap();
        let original = sink.into_outcome().accepted.pop().unwrap();
        let pointer = original.url().as_ptr();
        let (sent, result) = finalize_for_transport(
            original,
            &TransportFinalize {
                gpc: Some("1"),
                compression: Some(Compression::Gzip),
                max_encoded_bytes: 1024,
            },
        );
        let _finalized = result.expect("reserved finalizer tail fits");
        assert_eq!(sent.url().as_ptr(), pointer);
        assert_eq!(sent.headers().count(), 34);
        assert_eq!(
            sent.headers().skip(32).collect::<Vec<_>>(),
            [("Content-Encoding", "gzip"), ("Sec-GPC", "1")]
        );
    }

    #[test]
    fn impossible_encoded_reservation_returns_typed_internal_failure_with_context() {
        let (sent, result) = finalize_for_transport(
            request(b"x"),
            &TransportFinalize {
                gpc: None,
                compression: Some(Compression::Gzip),
                max_encoded_bytes: u64::MAX,
            },
        );
        assert_eq!(result.unwrap_err(), FinalizeError::ReserveOverflow);
        assert_eq!(sent.covered_imp_ids().collect::<Vec<_>>(), ["imp-1"]);
    }

    #[test]
    fn identity_encoded_cap_keeps_rejected_slot_metadata() {
        for (size, succeeds) in [(7, true), (8, false)] {
            let (sent, result) = finalize_for_transport(
                request(&vec![0; size]),
                &TransportFinalize {
                    gpc: Some("0"),
                    compression: None,
                    max_encoded_bytes: 7,
                },
            );
            assert_eq!(result.is_ok(), succeeds);
            if !succeeds {
                assert_eq!(
                    result.unwrap_err(),
                    FinalizeError::OversizeEncoded { limit: 7 }
                );
            }
            assert!(!sent.headers().any(|(name, _)| name == "Sec-GPC"));
            assert_eq!(sent.covered_imp_ids().collect::<Vec<_>>(), ["imp-1"]);
        }
    }

    #[test]
    fn gzip_expansion_and_exact_encoded_boundary_are_enforced() {
        let opts = TransportFinalize {
            gpc: Some("1"),
            compression: Some(Compression::Gzip),
            max_encoded_bytes: 1024,
        };
        let (_, initial_result) = finalize_for_transport(request(b"x"), &opts);
        let encoded = initial_result.unwrap();
        let size = u64::try_from(encoded.body().len()).unwrap();
        assert!(size > 1, "gzip expands this input");
        assert!(
            encoded
                .headers()
                .any(|pair| pair == ("Content-Encoding", "gzip"))
        );
        assert!(
            !encoded
                .headers()
                .any(|(name, _)| name.eq_ignore_ascii_case("Content-Length"))
        );
        for cap in [1, size - 1, size] {
            let (_, result) = finalize_for_transport(
                request(b"x"),
                &TransportFinalize {
                    max_encoded_bytes: cap,
                    ..opts
                },
            );
            assert_eq!(result.is_ok(), cap == size);
            if cap < size {
                assert_eq!(
                    result.unwrap_err(),
                    FinalizeError::OversizeEncoded { limit: cap }
                );
            }
        }
    }

    #[test]
    fn raw_failure_and_semantic_discard_refund_sibling_slots() {
        let budget = CallBudget::new(1);
        let push = |sink: &mut OutboundRequestSink<'_>, raw: &[u8]| {
            sink.push_request(
                &info(),
                |meta| {
                    meta.set_method("POST")?;
                    meta.set_url("https://example.test/bid")
                },
                |writer| writer.write_all(raw),
            )
        };
        let mut first = OutboundRequestSink::new(SinkLimits::new(2, 100), &budget, &Accept);
        assert!(push(&mut first, b"123").is_err());
        push(&mut first, b"12").unwrap();
        assert!(push(&mut first, b"12").is_err());
        let out = first.into_outcome_discarding_accepted();
        assert!(out.accepted.is_empty());
        assert_eq!(out.errors.len(), 2);
        assert_eq!(out.rejected_call_count, 1);
        let mut sibling = OutboundRequestSink::new(SinkLimits::new(2, 100), &budget, &Accept);
        push(&mut sibling, b"12").unwrap();
        assert_eq!(sibling.into_outcome().accepted.len(), 1);
    }

    #[test]
    fn bidder_builds_only_inside_the_sink_and_keeps_partial_errors() {
        let budget = CallBudget::new(1);
        let mut sink = OutboundRequestSink::new(SinkLimits::new(2, 100), &budget, &Accept);
        let request = BidRequest {
            id: "synthetic-request".into(),
            imp: vec![Imp {
                id: "imp-1".into(),
                banner: Some(Banner::default()),
                ..Imp::default()
            }],
            ..BidRequest::default()
        };
        let bidder = FakeBidder::default();
        bidder.make_requests(&request, &info(), &mut sink).unwrap();
        let out = sink.into_outcome();
        assert!(out.accepted.is_empty());
        assert_eq!(out.errors, [OutboundBuildError::OversizeRaw { limit: 2 }]);
        assert_eq!(budget.remaining(), 1);
        assert_eq!(
            bidder.written_bodies(),
            1,
            "the capped writer was exercised"
        );

        let mut roomy = OutboundRequestSink::new(SinkLimits::new(4096, 100), &budget, &Accept);
        bidder.make_requests(&request, &info(), &mut roomy).unwrap();
        let accepted = roomy.into_outcome();
        assert!(accepted.errors.is_empty());
        assert_eq!(accepted.accepted.len(), 1);
        assert_eq!(budget.remaining(), 0);
        let sent = &accepted.accepted[0];
        assert_eq!(sent.method(), "POST");
        assert_eq!(sent.url(), "https://example.test/bid");
        assert_eq!(sent.covered_imp_ids().collect::<Vec<_>>(), ["imp-1"]);
        let body: serde_json::Value = serde_json::from_slice(sent.body()).unwrap();
        assert_eq!(body["id"], "synthetic-request");
        assert_eq!(body["imp"].as_array().unwrap().len(), 1);
        assert_eq!(body["imp"][0]["id"], "imp-1");
        assert_eq!(bidder.written_bodies(), 2);
    }

    #[test]
    fn reserved_headers_reject_before_body_construction() {
        let budget = CallBudget::new(1);
        let mut sink = OutboundRequestSink::new(SinkLimits::new(2, 100), &budget, &Accept);
        for name in [
            "Content-Encoding",
            "content-LENGTH",
            "Sec-GPC",
            "hOsT",
            ":authority",
            "Transfer-Encoding",
            "TE",
            "Connection",
            "Trailer",
            "Upgrade",
            "Proxy-Connection",
            "Keep-Alive",
        ] {
            assert!(
                sink.push_request(
                    &info(),
                    |meta| meta.header(name, "x"),
                    |_| panic!("body ran")
                )
                .is_err()
            );
        }
        assert_eq!(budget.remaining(), 1);
        assert_eq!(sink.into_outcome().errors.len(), 12);
    }

    #[test]
    fn exact_url_pool_cap_passes_and_over_cap_never_builds_body() {
        let budget = CallBudget::new(2);
        let calls = Cell::new(0_usize);
        for extra in [0, 1] {
            let url = format!(
                "https://example.test/{}",
                "x".repeat(META_BIDDER_REGION_BYTES - 4 - 21 + extra)
            );
            let mut sink = OutboundRequestSink::new(SinkLimits::new(2, 100), &budget, &Accept);
            let result = sink.push_request(
                &info(),
                |meta| {
                    meta.set_method("POST")?;
                    meta.set_url(&url)
                },
                |writer| {
                    calls.set(calls.get() + 1);
                    writer.write_all(b"{}")
                },
            );
            assert_eq!(result.is_ok(), extra == 0);
        }
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn finalized_metadata_cannot_be_mutated_or_finalized_again() {
        let opts = TransportFinalize {
            gpc: Some("1"),
            compression: None,
            max_encoded_bytes: 1024,
        };
        let (sent, first) = finalize_for_transport(request(b"{}"), &opts);
        let (again, second) = finalize_for_transport(first.unwrap(), &opts);
        assert_eq!(second.unwrap_err(), FinalizeError::ReserveOverflow);
        assert!(Arc::ptr_eq(&sent, &again));
        assert_eq!(
            sent.headers()
                .filter(|(name, _)| *name == "Sec-GPC")
                .count(),
            1
        );
    }
}
