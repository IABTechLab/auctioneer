//! Call-local media correlation through construction, finalization, and dispatch.

mod support;

#[cfg(test)]
mod tests {
    use crate::support::response_budgets;
    use async_trait::async_trait;
    use auctioneer_bidder::correlation::CorrelationMedia;
    use auctioneer_bidder::{
        Bidder, BidderError, BidderHttpResponse, BidderRegistry, BidderRequestInfo, BidderResponse,
        DebugBodySchema, OutboundRequestMeta, OutboundRequestSink, TypedBid,
    };
    use auctioneer_config_data::types::{AccountConfig, BidderInfo, HostConfig, ResourceLimits};
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::{
        AccountSource, AuctionContext, AuctionDeadline, AuctionPlan, BidderCall, BidderCallResult,
        BidderFanout, FanoutError, FanoutInternalError, ResolvedAccount, ResolvedAliases,
        run_auction,
    };
    use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::BidResponse};
    use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare};
    use auctioneer_privacy::{gcl::Gcl, gvl::GvlStore};
    use auctioneer_runtime::{Clock as _, FakeClock, SeededRng};
    use futures::{
        channel::oneshot,
        executor::block_on,
        future::{join, join_all},
    };
    use serde_json::{Value, json};
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use std::{mem, ptr};

    const BIDDER: &str = "fake_fx";
    const IMP_ID: &str = "shared-imp";

    #[derive(Clone, Copy)]
    struct CallFixture {
        media: CorrelationMedia,
        token: u32,
        header: &'static [u8],
        bid_id: &'static str,
        format: &'static str,
        price: f64,
    }

    const CALLS: [CallFixture; 2] = [
        CallFixture {
            media: CorrelationMedia::Banner,
            token: 101,
            header: b"\x00banner\xff",
            bid_id: "banner-bid",
            format: "banner",
            price: 1.25,
        },
        CallFixture {
            media: CorrelationMedia::Video,
            token: 202,
            header: b"\x00video\xfe",
            bid_id: "video-bid",
            format: "video",
            price: 2.5,
        },
    ];

    #[test]
    fn reversed_completion_keeps_same_impression_media_and_tokens_call_local() {
        let (response, trace) = drive(None);
        assert_sent_context(&trace);
        assert_eq!(trace.parsed_tokens, [101, 202]);

        let wire = serde_json::to_value(response).unwrap();
        let seats = wire["seatbid"].as_array().unwrap();
        assert_eq!(seats.len(), 1);
        assert_eq!(seats[0]["seat"], BIDDER);
        let bids = seats[0]["bid"].as_array().unwrap();
        assert_eq!(bids.len(), 2, "both calls cover the same impression");
        for call in CALLS {
            let bid = bids.iter().find(|bid| bid["id"] == call.bid_id).unwrap();
            assert_bid(bid, call, call.media == CorrelationMedia::Video);
        }
        assert!(wire["ext"]["errors"][BIDDER].is_null());
        assert!(wire["ext"]["warnings"][BIDDER].is_null());
        assert!(wire["ext"]["prebid"]["seatnonbid"].is_null());
    }

    #[test]
    fn failed_call_does_not_shift_successful_sibling_metadata_or_attribution() {
        for failed_slot in 0..CALLS.len() {
            let (response, trace) = drive(Some(failed_slot));
            assert_sent_context(&trace);
            let survivor = CALLS[1 - failed_slot];
            assert_eq!(trace.parsed_tokens, [survivor.token]);

            let wire = serde_json::to_value(response).unwrap();
            let seats = wire["seatbid"].as_array().unwrap();
            assert_eq!(seats.len(), 1);
            assert_eq!(seats[0]["seat"], BIDDER);
            let bids = seats[0]["bid"].as_array().unwrap();
            assert_eq!(bids.len(), 1, "a failed call must not discard its sibling");
            assert_bid(&bids[0], survivor, true);
            assert_eq!(
                wire["ext"]["errors"][BIDDER],
                json!([{"code": 4_u32, "message": "bidder server error"}])
            );
            assert!(wire["ext"]["warnings"][BIDDER].is_null());
            assert_eq!(
                wire["ext"]["prebid"]["seatnonbid"],
                json!([{"seat": BIDDER, "nonbid": [{"impid": IMP_ID, "statuscode": 100_u32}]}])
            );
        }
    }

    #[derive(Default)]
    struct Trace {
        sent: Vec<Arc<OutboundRequestMeta>>,
        completed_slots: Vec<usize>,
        parsed_tokens: Vec<u32>,
    }

    type SharedTrace = Arc<Mutex<Trace>>;

    struct SplitMediaBidder {
        trace: SharedTrace,
    }

    impl Bidder for SplitMediaBidder {
        fn debug_body_schema(&self) -> DebugBodySchema {
            DebugBodySchema::Unsupported
        }

        fn make_requests(
            &self,
            request: &BidRequest,
            info: &BidderRequestInfo<'_>,
            sink: &mut OutboundRequestSink<'_>,
        ) -> Result<(), Vec<BidderError>> {
            for call in CALLS {
                let mut scoped = request.clone();
                for imp in &mut scoped.imp {
                    match call.media {
                        CorrelationMedia::Banner => imp.video = None,
                        CorrelationMedia::Video => imp.banner = None,
                        CorrelationMedia::Audio | CorrelationMedia::Native => {
                            return Err(vec![BidderError::bad_input(
                                "fixture supports banner and video",
                            )]);
                        }
                    }
                }
                let _outcome = sink.push_request(
                    info,
                    |meta| {
                        meta.set_method("POST")?;
                        meta.set_url(info.endpoint_template)?;
                        meta.header("Content-Type", "application/json")?;
                        meta.imp_id(IMP_ID)?;
                        meta.set_correlation_header(call.header)?;
                        meta.correlate_imp(0, call.media, call.token)?;
                        Ok(())
                    },
                    |body| body.write_json(&scoped),
                );
            }
            Ok(())
        }

        #[expect(
            clippy::panic_in_result_fn,
            clippy::unwrap_in_result,
            reason = "test spy asserts exact sent metadata and response pairing"
        )]
        fn make_bids(
            &self,
            request: &BidRequest,
            sent: &OutboundRequestMeta,
            response: &BidderHttpResponse,
        ) -> Result<BidderResponse, Vec<BidderError>> {
            assert_eq!(request.imp.len(), 1);
            assert!(request.imp[0].banner.is_some());
            assert!(request.imp[0].video.is_some());
            let correlation = sent.imp_correlation(0).unwrap();
            let expected = {
                let mut trace = self.trace.lock().unwrap();
                let slot = trace
                    .sent
                    .iter()
                    .position(|meta| ptr::eq(sent, meta.as_ref()))
                    .expect("make_bids must receive the exact Arc-owned transport metadata");
                trace.parsed_tokens.push(correlation.token());
                CALLS[slot]
            };
            assert_eq!(correlation.media(), expected.media);
            assert_eq!(correlation.token(), expected.token);
            assert_eq!(sent.correlation_header(), Some(expected.header));
            assert!(response.max_parsed_nodes() > 0);
            let parsed = response.parse_bid_response()?;
            let bids = parsed
                .seatbid
                .into_iter()
                .flat_map(|seat| seat.bid)
                .map(|bid| {
                    assert_eq!(bid.id, expected.bid_id, "fanout results remain positional");
                    assert_eq!(bid.impid, IMP_ID);
                    assert!(!bid.other.contains_key("mtype"));
                    assert!(bid.ext.is_none(), "no response-provided media hint");
                    TypedBid {
                        bid,
                        media_type: correlation.media().into(),
                    }
                })
                .collect();
            Ok(BidderResponse {
                bids,
                currency: parsed.cur,
                errors: vec![],
                auction_configs: Vec::new(),
            })
        }
    }

    struct ReverseCompletionFanout {
        trace: SharedTrace,
        failed_slot: Option<usize>,
    }

    #[async_trait(?Send)]
    impl BidderFanout for ReverseCompletionFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            assert_eq!(calls.len(), CALLS.len());
            let mut gates = Vec::new();
            let mut pending = Vec::new();
            for (slot, outbound) in calls.into_iter().enumerate() {
                let (meta, body) = outbound.request.into_parts();
                self.trace.lock().unwrap().sent.push(meta);
                drop(body);
                let (release, released) = oneshot::channel::<()>();
                let (complete, completed) = oneshot::channel::<()>();
                gates.push((release, completed));
                pending.push(async move {
                    released.await.expect("completion gate remains open");
                    let result = if self.failed_slot == Some(slot) {
                        Err(FanoutError::BadServerResponse)
                    } else {
                        let call = CALLS[slot];
                        Ok(BidderHttpResponse::new(
                            200,
                            vec![],
                            serde_json::to_vec(&json!({
                                "id": "response", "cur": "USD",
                                "seatbid": [{"seat": "fake_upstream", "bid": [{
                                    "id": call.bid_id, "impid": IMP_ID, "price": call.price
                                }]}]
                            }))
                            .unwrap(),
                        ))
                    };
                    self.trace.lock().unwrap().completed_slots.push(slot);
                    complete.send(()).expect("completion observer remains open");
                    BidderCallResult {
                        result,
                        elapsed: Duration::from_millis(1),
                    }
                });
            }

            // Wait for each released call to complete before releasing its predecessor.
            // join_all still returns the original input slots, as BidderFanout requires.
            let release_in_reverse = async move {
                for (release, completed) in gates.into_iter().rev() {
                    release.send(()).expect("call is waiting for release");
                    completed.await.expect("released call completes");
                }
            };
            let (results, ()) = join(join_all(pending), release_in_reverse).await;
            Ok(results)
        }
    }

    fn drive(failed_slot: Option<usize>) -> (BidResponse, Trace) {
        let trace = SharedTrace::default();
        let mut registry = BidderRegistry::new();
        registry
            .register(
                Box::leak(Box::new(BidderInfo {
                    name: BIDDER.into(),
                    endpoint: "https://bidder.example.test/auction".into(),
                    uses_https: true,
                    ..BidderInfo::default()
                })),
                Box::new(SplitMediaBidder {
                    trace: Arc::clone(&trace),
                }),
            )
            .expect("unique test bidder");
        let request = prepare(
            serde_json::from_value(json!({
                "id": "call-correlation", "site": {"id": "site"}, "regs": {"gdpr": 0_i32},
                "imp": [{
                    "id": IMP_ID,
                    "banner": {"w": 300_i32, "h": 250_i32},
                    "video": {"mimes": ["video/mp4"]},
                    "ext": {"bidder": {(BIDDER): {}}}
                }],
                "ext": {"prebid": {
                    "returnallbidstatus": true,
                    "targeting": {
                        "includeformat": true, "includewinners": true, "includebidderkeys": true
                    }
                }}
            }))
            .unwrap(),
            &RequestHeaders::default(),
            &PrepareLimits {
                max_impressions: 1,
                max_copied_header_bytes: 4096,
            },
        )
        .unwrap();
        let host = HostConfig {
            resource_limits: ResourceLimits {
                default_tmax_ms: 1000,
                max_tmax_ms: 2000,
                max_bidder_response_bytes: 0x0004_0000,
                max_bidders_per_auction: 1,
                max_outbound_requests: 2,
                max_outbound_raw_bytes: 0x0004_0000,
                max_outbound_request_bytes: 0x0004_0000,
                max_transport_bytes: 0x0004_0000,
                max_decoded_bytes: 0x0004_0000,
                max_parsed_nodes: 0x0001_0000,
            },
            ..HostConfig::default()
        };
        let config = AccountConfig::default();
        let account = ResolvedAccount {
            id: "publisher".into(),
            source: AccountSource::DefaultFallback,
            config: &config,
        };
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let rng = SeededRng::new(42);
        let fanout = ReverseCompletionFanout {
            trace: Arc::clone(&trace),
            failed_slot,
        };
        let ctx = AuctionContext::with_request_start(
            clock.now_monotonic(),
            &host,
            &account,
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
        let response = block_on(run_auction(&request, &ctx, &plan, &mut audit)).unwrap();
        let observed = mem::take(&mut *trace.lock().unwrap());
        (response, observed)
    }

    fn assert_sent_context(trace: &Trace) {
        assert_eq!(trace.sent.len(), CALLS.len());
        assert_eq!(trace.completed_slots, [1, 0]);
        assert!(!Arc::ptr_eq(&trace.sent[0], &trace.sent[1]));
        for (meta, expected) in trace.sent.iter().zip(CALLS) {
            assert_eq!(meta.covered_imp_ids().collect::<Vec<_>>(), [IMP_ID]);
            assert_eq!(meta.correlation_header(), Some(expected.header));
            let row = meta.imp_correlation(0).unwrap();
            assert_eq!(row.media(), expected.media);
            assert_eq!(row.token(), expected.token);
            assert_eq!(meta.imp_correlation(1), None);
        }
    }

    fn assert_bid(bid: &Value, expected: CallFixture, winner: bool) {
        assert_eq!(bid["id"], expected.bid_id);
        assert_eq!(bid["impid"], IMP_ID);
        assert_eq!(bid["price"], json!(expected.price));
        assert_eq!(bid["ext"]["prebid"]["type"], expected.format);
        let targeting = &bid["ext"]["prebid"]["targeting"];
        assert_eq!(targeting["hb_format_fake_fx"], expected.format);
        assert_eq!(targeting["hb_bidder_fake_fx"], BIDDER);
        if winner {
            assert_eq!(targeting["hb_format"], expected.format);
            assert_eq!(targeting["hb_bidder"], BIDDER);
        } else {
            assert!(targeting["hb_format"].is_null());
            assert!(targeting["hb_bidder"].is_null());
        }
    }
}
