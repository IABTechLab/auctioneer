//! Snapshot fixtures for an upstream response that breaches a response cap.
//!
//! `EdgeZero` enforces several INDEPENDENT response limits on a bidder
//! call, and each one breaches for a different reason: the encoded body
//! outgrew `max_encoded_response_bytes`, the decode expanded past
//! `max_decoded_response_bytes`, the response header block outgrew
//! `max_response_header_bytes`, or it carried more entries than
//! `max_response_header_count`. All of them arrive as the same upstream
//! variant carrying a distinguishing `reason`.
//!
//! auctioneer owes the same VERDICT for all of them — this bidder is dropped,
//! its siblings are not — but not the same DIAGNOSIS. The reason is a
//! typed upstream field, and auctioneer preserves it as an
//! `OversizeOrigin` so an operator reading a drop can tell a header-count
//! breach from a body that decompressed too far. This file pins that
//! discrimination: each cap class must map to its OWN origin, and no two
//! classes may share one.
//!
//! The file proves that in two layers, because neither layer alone can.
//!
//! The CLASSIFICATION layer drives the real wave scheduler over a
//! scripted outbound batch that hands back a genuine
//! `EdgeError::ResponseTooLarge` per cap class. That is the only place
//! the mapping from an upstream limit breach to auctioneer's taxonomy can be
//! observed, because auctioneer never mints that error itself — a fanout
//! stand-in returning `FanoutError::OversizeResponse` directly would
//! assert nothing about the mapping and would still pass if the mapping
//! were deleted. Each row also carries a SIBLING slot in the same batch
//! that answers normally, so the assertion is positional: a cap breach
//! fails one position and leaves the other intact.
//!
//! The WIRE layer drives the real auction handler, because the mapped
//! `FanoutError` is only half the contract. The other half is what an
//! operator reads off the response: `ext.errors.<bidder>` code 4,
//! `ext.prebid.seatnonbid` statuscode 100 over exactly the imps the
//! dropped call covered, no `seatbid` row for the dropped bidder, and an
//! ordinary `seatbid` row for its sibling. Those live in the exchange's
//! response assembly, which the classification layer never reaches.
//!
//! Both layers pin the clock to a `FakeClock` at the UNIX epoch. A cap
//! breach must be graded as a SIZE failure and nothing else, and a real
//! clock would let a slow runner overrun `tmax` and fold an auction-wide
//! timeout into the same expectation.

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
    use crate::serving_catalog::{self, ServingBidder};
    use auctioneer_core::test_support::outbound_request;

    use crate::support::ctx_with_config;
    use async_trait::async_trait;
    use auctioneer_bidder::BidderHttpResponse;
    use auctioneer_config::types::MediaType;
    use auctioneer_core::handlers::{TestFanoutHandle, auction};
    use auctioneer_core::host_primitives::{
        TestClockHandle, TestMonotonicClockHandle, TestRngHandle,
    };
    use auctioneer_core::outbound::BoundedFetch;
    use auctioneer_core::outbound::wave::{WaveHost, drive_fanout};
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutError,
        FanoutInternalError, OversizeOrigin, TmaxSource,
    };
    use auctioneer_runtime::{
        Clock, FakeClock, MAX_GVL_PARSED_BYTES, MIN_INBOUND_PARSE_FACTOR,
        MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR, NoopDecodeCooperator,
        ResolvedIsolateLimits, Rng, SeededRng, StreamGate, Timer as _,
    };
    use edgezero_core::body::Body;
    use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;
    use edgezero_core::error::{EdgeError, ResponseLimitReason};
    use edgezero_core::http::{HeaderMap, StatusCode};
    use edgezero_core::outbound::{
        HttpClient, IndependentOutboundProgress, OutboundBatch, OutboundBatchDriverEvent,
        OutboundDispatchRequest, OutboundExchangeOwner, OutboundFuture, OutboundHead,
        OutboundHttpClient, OutboundPending, OutboundPreparedBatch, OutboundProgressRuntime,
        OutboundRequest, OutboundResponse, OutboundSendMode, OutboundTime,
    };
    use edgezero_core::time::MonotonicClock;
    use futures::executor::block_on;
    use futures::stream;
    use http::{Method, Request, Uri};
    use std::rc::Rc;
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::Duration;
    use time::OffsetDateTime;

    /// Response cap wide enough that reading auctioneer's OWN answer back
    /// never trips it. Deliberately unrelated to the caps under test,
    /// which are upstream's per-bidder-call caps.
    const WIDE_CAP: usize = 1 << 20;

    fn roles() -> &'static [ServingBidder; 2] {
        static ROLES: OnceLock<[ServingBidder; 2]> = OnceLock::new();
        ROLES.get_or_init(|| {
            let mut candidates = serving_catalog::default_active_site_media(&[MediaType::Banner])
                .into_iter()
                .filter(|row| !row.info.disabled && !row.info.endpoint.contains("{{"));
            let first = candidates.next().unwrap();
            let second = candidates
                .find(|row| row.info.endpoint != first.info.endpoint)
                .unwrap();
            [first, second]
        })
    }

    /// Auction budget, pinned on the fixture so the deadline is a known
    /// span on the `FakeClock` axis and no row can expire by accident.
    const AUCTION_TMAX_MS: i64 = 1000;

    /// The same budget in the unit the wave scheduler reads.
    const AUCTION_MILLIS: u64 = 1000;

    /// Per-call timeout, comfortably inside the auction budget.
    const CALL_TIMEOUT_MS: u64 = 500;

    /// The imp every fixture carries, and therefore the imp id every
    /// `seatnonbid` row must name.
    const IMP_ID: &str = "imp-1";

    /// Every cap class this build recognises, paired with the origin
    /// auctioneer must report for it.
    ///
    /// Written as an explicit table rather than iterated over
    /// `ResponseLimitReason`'s variants because that enum is
    /// `#[non_exhaustive]`: the point of the table is to name the
    /// expected answer per reason, which no iteration can supply.
    ///
    /// `Unspecified` is included and expects
    /// [`OversizeOrigin::Unclassified`], which is also where an upstream
    /// reason this build does not know about must land. That pairing is
    /// what stops a future platform variant from being folded into a
    /// named class it has no claim to.
    const CAP_CLASSES: [(&str, ResponseLimitReason, OversizeOrigin); 8] = [
        (
            "encoded body over cap",
            ResponseLimitReason::EncodedBody,
            OversizeOrigin::EncodedBody,
        ),
        (
            "decoded body over cap",
            ResponseLimitReason::DecodedBody,
            OversizeOrigin::DecodedBody,
        ),
        (
            "buffered body over cap",
            ResponseLimitReason::BufferedBody,
            OversizeOrigin::BufferedBody,
        ),
        (
            "decoder working memory over cap",
            ResponseLimitReason::DecoderMemory,
            OversizeOrigin::DecoderMemory,
        ),
        (
            "brotli window too wide",
            ResponseLimitReason::BrotliWindow,
            OversizeOrigin::BrotliWindow,
        ),
        (
            "header bytes over cap",
            ResponseLimitReason::HeaderBytes,
            OversizeOrigin::HeaderBytes,
        ),
        (
            "header count over cap",
            ResponseLimitReason::HeaderCount,
            OversizeOrigin::HeaderCount,
        ),
        (
            "upstream named no reason",
            ResponseLimitReason::Unspecified,
            OversizeOrigin::Unclassified,
        ),
    ];

    // ───────── the classification layer ─────────

    /// Limits wide enough that nothing here trips a AUCTIONEER cap. Every
    /// breach in this file is upstream's verdict, delivered as a typed
    /// error; a auctioneer-side cap firing first would prove the wrong thing.
    fn test_limits() -> ResolvedIsolateLimits {
        ResolvedIsolateLimits {
            isolate_total_bytes: 512 * 1024 * 1024,
            baseline_bytes: 0,
            inbound_request_bytes: 256 * 1024,
            inbound_parse_factor: MIN_INBOUND_PARSE_FACTOR,
            max_outbound_request_bytes: 256 * 1024,
            max_outbound_raw_bytes: 256 * 1024,
            max_transport_bytes: 256 * 1024,
            max_decoded_bytes: 256 * 1024,
            response_parse_factor: MIN_RESPONSE_PARSE_FACTOR,
            response_node_peak_bytes: MIN_RESPONSE_NODE_PEAK_BYTES,
            final_response_bytes: 256 * 1024,
            egress_permits: 4,
            response_write_deadline_ms: 10_000,
            guest_decoder_bound_bytes: 0,
            aux_route_permits: 2,
            aux_route_peak_bytes: 1024,
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            gvl_cache_entries: 1,
            gvl_parsed_bytes: MAX_GVL_PARSED_BYTES,
            max_bidders_per_auction: 8,
            max_outbound_requests: 8,
            max_concurrent_auctions: 4,
            max_active_body_streams: 4,
            max_runtime_config_bytes: 1024,
            max_runtime_config_blob_bytes: 1024,
            max_parsed_nodes: 1024,
            max_copied_header_bytes: 1024,
            max_admitted_header_bytes: 8192,
            max_admitted_header_count: 64,
            max_impressions_per_auction: 100,
            max_config_json_nodes: DEFAULT_CONFIG_JSON_NODES,
            terminal_response_permits: 2,
            refusal_body_bytes: 1024,
            transmit_state_bytes: 1024,
        }
    }

    /// Stores only a thread-safe failure recipe; local response bodies are
    /// constructed after the facade has supplied the prepared batch.
    struct ScriptedClient {
        failure: Mutex<Option<EdgeError>>,
        clock: Arc<FakeClock>,
    }

    impl ScriptedClient {
        fn new(failure: EdgeError, clock: &Arc<FakeClock>) -> Self {
            Self {
                failure: Mutex::new(Some(failure)),
                clock: Arc::clone(clock),
            }
        }
    }

    impl OutboundHttpClient for ScriptedClient {
        fn begin(&self, _: OutboundDispatchRequest) -> Result<OutboundPending, EdgeError> {
            Err(EdgeError::not_implemented(
                "classification fixture must use batch dispatch",
            ))
        }

        fn dispatch_head(&self, _: OutboundDispatchRequest) -> OutboundFuture<OutboundHead> {
            panic!("classification fixture must use batch dispatch");
        }

        fn progress(&self) -> Rc<dyn OutboundProgressRuntime> {
            Rc::new(IndependentOutboundProgress)
        }

        fn start_batch_until(&self, batch: OutboundPreparedBatch) -> OutboundBatch {
            let (context, slots) = batch.into_slots();
            assert_eq!(
                slots.len(),
                2,
                "both scripted positions must reach dispatch"
            );
            let mut failure = Some(
                self.failure
                    .lock()
                    .unwrap()
                    .take()
                    .expect("single scripted wave"),
            );
            self.clock.advance(Duration::from_millis(10));
            let events = slots
                .into_iter()
                .map(|(index, request)| {
                    let outcome = if index == 0 {
                        Err(failure.take().expect("one failing position"))
                    } else {
                        Ok(OutboundResponse::new(
                            request.request().method().clone(),
                            StatusCode::OK,
                            HeaderMap::new(),
                            Body::from_bytes(br#"{"id":"r"}"#.to_vec()),
                            request.clock().clone(),
                            edgezero_core::ContentTypeProvenance::Unknown,
                        ))
                    };
                    OutboundBatchDriverEvent::Item {
                        index,
                        completed_at: context.clock().now(),
                        outcome,
                    }
                })
                .collect::<Vec<_>>();
            OutboundBatch::from_driver(
                context,
                stream::iter(events),
                OutboundExchangeOwner::new(|| {}),
            )
        }

        fn time(&self) -> OutboundTime {
            let read_clock = Arc::clone(&self.clock);
            let waits = Arc::clone(&self.clock);
            OutboundTime::new(
                MonotonicClock::new(move || read_clock.now_monotonic()),
                move |deadline, clock| {
                    let waiter = Arc::clone(&waits);
                    Box::pin(async move {
                        if let Some(remaining) = deadline.remaining_at(clock.now()) {
                            waiter.sleep(remaining).await;
                        }
                        Ok(())
                    })
                },
            )
        }

        fn validate_supported_options(
            &self,
            _: &OutboundRequest,
            _: OutboundSendMode,
        ) -> Result<(), EdgeError> {
            Ok(())
        }
    }

    fn outbound_call(url: &str) -> BidderCall {
        BidderCall {
            request: outbound_request::request(
                "POST".into(),
                url.into(),
                vec![],
                b"{}".to_vec(),
                vec![IMP_ID.to_owned()],
            )
            .expect("sink-built fixture"),
            timeout: Duration::from_millis(CALL_TIMEOUT_MS),
            max_transport_bytes: 256 * 1024,
            max_decoded_bytes: 256 * 1024,
        }
    }

    fn auction_deadline(clock: &FakeClock) -> AuctionDeadline {
        AuctionDeadline {
            deadline_at: clock
                .origin()
                .checked_add(Duration::from_millis(AUCTION_MILLIS))
                .expect("the pinned horizon is representable"),
            source: TmaxSource::Request,
        }
    }

    /// One slot upstream refused for breaching the named response limit.
    ///
    /// The message carries upstream's own free text deliberately: the
    /// classification must key off the typed VARIANT and its typed
    /// `reason`, and a mapping that read the message would have to be able
    /// to read this.
    fn too_large_slot(reason: ResponseLimitReason) -> EdgeError {
        EdgeError::response_too_large_with_reason(
            "upstream response exceeded a limit; 4194304 > 262144",
            reason,
        )
    }

    /// Drive the real wave scheduler over a two-slot batch: position 0
    /// breaches the named response limit, position 1 answers normally.
    fn classify_two_slot_wave(reason: ResponseLimitReason) -> Vec<BidderCallResult> {
        classify_two_slot_wave_slot(too_large_slot(reason))
    }

    /// Same two-slot batch, with position 0 refused by an arbitrary
    /// upstream error rather than a response-limit breach.
    fn classify_two_slot_wave_with(err: EdgeError) -> Vec<BidderCallResult> {
        classify_two_slot_wave_slot(err)
    }

    /// Drive the real wave scheduler over a two-slot batch: position 0
    /// gets `failing`, position 1 answers normally.
    fn classify_two_slot_wave_slot(failing: EdgeError) -> Vec<BidderCallResult> {
        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let gate = StreamGate::new(4);
        let client = BoundedFetch::new(
            HttpClient::with_client(ScriptedClient::new(failing, &clock)),
            test_limits(),
        );
        let deadline = auction_deadline(&clock);
        block_on(drive_fanout(
            vec![
                outbound_call("https://oversize.example/bid"),
                outbound_call("https://sibling.example/bid"),
            ],
            &deadline,
            WaveHost {
                clock: clock.as_ref(),
                timer: clock.as_ref(),
                cooperator: &NoopDecodeCooperator,
            },
            &client,
            &test_limits(),
            &gate,
        ))
        .expect("the fanout itself completes: a cap breach is one slot's failure")
    }

    /// Every cap class maps to `OversizeResponse` carrying ITS OWN
    /// origin, and only the breaching position fails.
    ///
    /// This is a discrimination assertion, not a category one. An earlier
    /// version of this test asserted the opposite — that every cap class
    /// yielded the same payload-free `OversizeResponse` — and so it
    /// pinned the very collapse that threw upstream's `reason` away. It
    /// is rewritten rather than relaxed: a `matches!` tolerant of any
    /// origin would pass against the collapsed shape too and would assert
    /// nothing.
    ///
    /// The classes are asserted in ONE test rather than one apiece so a
    /// reason that starts classifying wrongly cannot hide behind siblings
    /// that still pass; the loop names the class in every failure message.
    #[test]
    fn each_cap_class_maps_to_its_own_origin_and_spares_its_sibling() {
        for (label, reason, expected_origin) in CAP_CLASSES {
            let results = classify_two_slot_wave(reason);
            assert_eq!(
                results.len(),
                2,
                "{label}: two calls in, two results out -- positional completeness holds even \
                 when a slot is refused",
            );
            assert!(
                matches!(
                    results[0].result,
                    Err(FanoutError::OversizeResponse(origin)) if origin == expected_origin
                ),
                "{label}: a response-limit breach is OversizeResponse({expected_origin:?}), \
                 got {:?}",
                results[0].result,
            );
            assert!(
                results[1].result.is_ok(),
                "{label}: the sibling slot in the SAME batch answers normally, got {:?}",
                results[1].result,
            );
        }
    }

    /// No two cap classes report the same origin.
    ///
    /// The per-class loop above checks each expectation against the table,
    /// so a table that named one origin for every row would satisfy it
    /// while discriminating nothing. This row checks the table itself: the
    /// expected origins are pairwise distinct, which is what makes the
    /// mapping informative rather than merely typed.
    #[test]
    fn the_cap_classes_report_pairwise_distinct_origins() {
        for (label, reason, expected_origin) in CAP_CLASSES {
            for (other_label, other_reason, other_origin) in CAP_CLASSES {
                if reason == other_reason {
                    continue;
                }
                assert_ne!(
                    expected_origin, other_origin,
                    "{label} and {other_label} are separate upstream ceilings but report the \
                     same origin {expected_origin:?}, so auctioneer cannot tell them apart",
                );
            }
        }
    }

    /// A refused REQUEST header block keeps the oversize verdict but does
    /// NOT borrow a response ceiling's origin.
    ///
    /// This arm used to share `ResponseTooLarge`'s classification
    /// wholesale. It still shares the wire contract, because nothing was
    /// read back and the bidder is dropped either way, but it is a
    /// different fact: auctioneer composed a request the platform would not
    /// carry. Asserting it here stops a later tidy-up from merging the two
    /// arms back together.
    #[test]
    fn a_refused_request_header_block_reports_the_request_origin() {
        let results = classify_two_slot_wave_with(EdgeError::RequestHeaderFieldsTooLarge {
            message: "request header block exceeded a limit; 131072 > 8192".to_owned(),
        });
        assert!(
            matches!(
                results[0].result,
                Err(FanoutError::OversizeResponse(
                    OversizeOrigin::RequestHeaders
                )),
            ),
            "a refused request header block is OversizeResponse(RequestHeaders), got {:?}",
            results[0].result,
        );
        assert!(
            results[1].result.is_ok(),
            "the sibling slot in the SAME batch answers normally, got {:?}",
            results[1].result,
        );
        for (label, _reason, origin) in CAP_CLASSES {
            assert_ne!(
                origin,
                OversizeOrigin::RequestHeaders,
                "{label} is a RESPONSE ceiling and must not report the request-header origin",
            );
        }
    }

    /// The wire category is the same fixed string for EVERY origin.
    ///
    /// The origin adds diagnostic fidelity inside auctioneer and must add
    /// nothing to the wire. This row is what makes that a contract rather
    /// than an accident: every origin auctioneer can mint, including the
    /// request-header one and `Unclassified`, renders the one category.
    #[test]
    fn every_origin_renders_the_same_wire_category() {
        const EVERY_ORIGIN: [OversizeOrigin; 9] = [
            OversizeOrigin::EncodedBody,
            OversizeOrigin::DecodedBody,
            OversizeOrigin::BufferedBody,
            OversizeOrigin::DecoderMemory,
            OversizeOrigin::BrotliWindow,
            OversizeOrigin::HeaderBytes,
            OversizeOrigin::HeaderCount,
            OversizeOrigin::RequestHeaders,
            OversizeOrigin::CallResponseCap,
        ];
        for origin in EVERY_ORIGIN {
            assert_eq!(
                FanoutError::OversizeResponse(origin).public_message(),
                "bidder response too large",
                "{origin:?} must render the one oversize category",
            );
        }
        assert_eq!(
            FanoutError::OversizeResponse(OversizeOrigin::Unclassified).public_message(),
            "bidder response too large",
            "an unnameable origin renders the same category as a named one",
        );
    }

    /// The public message is the fixed category, with upstream's free
    /// text dropped.
    ///
    /// The scripted error above carries a byte count and a comparison in
    /// its message; neither may reach the wire, because upstream text is
    /// untrusted and a mapping that forwarded it would leak whatever an
    /// upstream chose to put there.
    #[test]
    fn the_oversize_category_carries_no_upstream_text() {
        for (label, reason, _origin) in CAP_CLASSES {
            let results = classify_two_slot_wave(reason);
            let Err(err) = &results[0].result else {
                panic!("{label}: the breaching slot must fail");
            };
            assert_eq!(
                err.public_message(),
                "bidder response too large",
                "{label}: the wire category is fixed per variant",
            );
            assert!(
                !err.public_message().contains("4194304"),
                "{label}: upstream's byte figures must not survive into the category",
            );
        }
    }

    // ───────── the wire layer ─────────

    /// Fails the first role's call as an oversize response and answers
    /// every other call with one banner bid per covered imp.
    ///
    /// Keyed on the request URL rather than on call order: the exchange
    /// owns bidder ordering, and a fixture that assumed a position would
    /// silently start testing the wrong bidder if that order changed.
    /// The URL is the one baked into the bidder's own `BidderInfo` —
    /// runtime `[bidders.<name>]` overlays carry no endpoint field, so
    /// there is no way to rewrite it from the deployment blob.
    struct OneOversizeBidderFanout;

    #[async_trait(?Send)]
    impl BidderFanout for OneOversizeBidderFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            Ok(calls
                .iter()
                .map(|call| {
                    let result = if call.request.url() == roles()[0].info.endpoint {
                        // The origin is arbitrary here on purpose: the
                        // wire layer must render code 4 / statuscode 100
                        // for EVERY origin, so the row picks one and the
                        // origin-blindness assertion below covers the
                        // rest.
                        Err(FanoutError::OversizeResponse(OversizeOrigin::HeaderCount))
                    } else {
                        let bids: Vec<String> = call
                            .request
                            .covered_imp_ids()
                            .map(|imp_id| {
                                format!(
                                    "{{\"id\":\"bid-{imp_id}\",\"impid\":\"{imp_id}\",\
                                     \"price\":1.5,\"w\":300,\"h\":250,\
                                     \"crid\":\"creative-{imp_id}\"}}"
                                )
                            })
                            .collect();
                        Ok(BidderHttpResponse::new(
                            200,
                            vec![],
                            format!(
                                "{{\"id\":\"r\",\"seatbid\":[{{\"seat\":\"s\",\"bid\":[{}]}}],\
                                 \"cur\":\"USD\"}}",
                                bids.join(","),
                            )
                            .into_bytes(),
                        ))
                    };
                    BidderCallResult {
                        result,
                        elapsed: Duration::from_millis(10),
                    }
                })
                .collect())
        }
    }

    /// A single banner imp naming both bidders, with
    /// `returnallbidstatus` on so both halves of the error/non-bid pair
    /// reach the wire.
    fn request_body() -> String {
        let [oversize, sibling] = roles();
        serde_json::to_string(&serde_json::json!({
            "id":"auc-oversize", "tmax":AUCTION_TMAX_MS, "cur":["USD"],
            "imp":[{"id":IMP_ID,"banner":{"w":300_u32,"h":250_u32},
                "ext":{"prebid":{"bidder":{
                    (oversize.name()):oversize.params, (sibling.name()):sibling.params
                }}}}],
            "ext":{"prebid":{"returnallbidstatus":true}}
        }))
        .unwrap()
    }

    /// POST the fixture through the real auction handler and hand back
    /// the deserialized answer.
    fn drive_handler() -> (StatusCode, serde_json::Value) {
        config_funding::install();
        let fake = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let clock: Arc<dyn Clock + Send + Sync> = Arc::<FakeClock>::clone(&fake);
        let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));
        let mut request = Request::builder()
            .method(Method::POST)
            .uri(Uri::from_static("/openrtb2/auction"))
            .header("content-type", "application/json")
            .body(Body::from_bytes(request_body().into_bytes()))
            .expect("request builds");
        request
            .extensions_mut()
            .insert(TestFanoutHandle(Arc::new(OneOversizeBidderFanout)));
        request.extensions_mut().insert(TestClockHandle(clock));
        // The SAME fake clock drives the monotonic axis, so `request_start`,
        // the composed deadline and the post-fanout recheck all sit at the
        // pinned origin. Without it the auction budget would be spent
        // against real elapsed time and a slow runner could fold an
        // auction-wide timeout into this row's expectation.
        let monotonic_source = Arc::<FakeClock>::clone(&fake);
        request
            .extensions_mut()
            .insert(TestMonotonicClockHandle(MonotonicClock::new(move || {
                monotonic_source.now_monotonic()
            })));
        request.extensions_mut().insert(TestRngHandle(rng));
        // An EMPTY deployment blob: every cap and endpoint in play here
        // is the baked one, and the row is about a response verdict
        // rather than about configuration.
        let ctx = ctx_with_config("", request);

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
                serde_json::from_slice(&bytes).unwrap_or_else(|err| {
                    panic!(
                        "the answer is JSON: {err}; body {}",
                        String::from_utf8_lossy(&bytes),
                    )
                }),
            )
        })
    }

    /// Every seat name carrying a `seatbid` row.
    fn seats(answer: &serde_json::Value) -> Vec<String> {
        answer
            .pointer("/seatbid")
            .and_then(serde_json::Value::as_array)
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| row.pointer("/seat").and_then(serde_json::Value::as_str))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// An oversize bidder is dropped with code 4 and statuscode 100
    /// while its sibling publishes an ordinary bid.
    ///
    /// The whole contract in one row, because the four surfaces are one
    /// fact: the error entry, the non-bid row, the ABSENCE of the
    /// dropped bidder's seat, and the PRESENCE of its sibling's. Split
    /// across four tests, a change that dropped the entire auction would
    /// still pass three of them.
    #[test]
    fn an_oversize_bidder_is_dropped_with_code_4_while_its_sibling_publishes() {
        let [oversize, sibling] = roles();
        assert_ne!(oversize.info.endpoint, sibling.info.endpoint);
        let (status, answer) = drive_handler();
        assert_eq!(
            status,
            StatusCode::OK,
            "an auction with one refused bidder is still a 200 with an error block; answer {answer}",
        );
        let ext = answer
            .pointer("/ext")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        assert_eq!(
            ext.pointer(&format!("/errors/{}", oversize.name())),
            Some(&serde_json::json!([{
                "code": 4_u32,
                "message": "bidder response too large",
            }])),
            "the dropped bidder reports error code 4 with the fixed category; ext {ext}",
        );
        assert_eq!(
            ext.pointer(&format!("/errors/{}", sibling.name())),
            None,
            "one bidder's cap breach must not write an error entry for the other; ext {ext}",
        );
        assert_eq!(
            ext.pointer("/prebid/seatnonbid"),
            Some(&serde_json::json!([{
                "seat": oversize.name(),
                "nonbid": [{ "impid": IMP_ID, "statuscode": 100_u32 }],
            }])),
            "exactly one non-bid row, statuscode 100, over the imp the dropped call covered; \
             ext {ext}",
        );

        let default_active_publications = seats(&answer);
        assert!(
            !default_active_publications
                .iter()
                .any(|seat| seat == oversize.name()),
            "the dropped bidder publishes no seatbid row, got seats {default_active_publications:?}",
        );
        assert!(
            default_active_publications
                .iter()
                .any(|seat| seat == sibling.name()),
            "the sibling publishes normally, got seats {default_active_publications:?}; answer {answer}",
        );
        assert_eq!(
            ext.pointer("/errors/prebid"),
            None,
            "a size failure is not a time failure: the pinned clock never reaches the deadline, \
             so no auction-wide entry may appear; ext {ext}",
        );
    }
}
