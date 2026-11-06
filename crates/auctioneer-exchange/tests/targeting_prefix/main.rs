//! Targeting-prefix resolution: request-block override wins over
//! account-block default; account-block fills in when the request omits
//! it. Both cases feed `run_auction` end-to-end and inspect the keys
//! attached to the winning bid's `ext.prebid.targeting`.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

use auctioneer_exchange::test_support as support;

#[cfg(test)]
mod tests {
    // Published provider inputs are loaded from owner files on the native host.
    #[cfg(not(target_arch = "wasm32"))]
    mod provider_conformance;
    use crate::fake_bidder::FakeBidder;
    use crate::support::response_budgets;
    use async_trait::async_trait;
    use futures::executor::block_on;

    use auctioneer_bidder::{Bidder, BidderHttpResponse, BidderRegistry};
    use auctioneer_config_data::types::{
        AccountConfig, BidderInfo, Capabilities, HostConfig, MediaType, ResourceLimits,
    };
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::{
        AccountSource, AuctionContext, AuctionDeadline, AuctionPlan, BidderCall, BidderCallResult,
        BidderFanout, FanoutInternalError, ResolvedAccount, ResolvedAliases, run_auction,
    };
    use auctioneer_openrtb::ext::imp::ImpExt;
    use auctioneer_openrtb::ext::request_prebid::{ExtRequestPrebid, ExtRequestTargeting};
    use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, BidRequestExt, Imp, Video};
    use auctioneer_openrtb::types::bid_response::BidResponse;
    use auctioneer_openrtb::types::field::JsonField;
    use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare};

    const IMP_CAP_NOT_UNDER_TEST: PrepareLimits = PrepareLimits {
        max_impressions: 100,
        max_copied_header_bytes: 4_096,
    };
    const PRICE_GRANULARITY_WARNING_CODE: u64 = 10021;
    const GLOBAL_PRICE_GRANULARITY_SCOPE: &str = "ext.prebid.targeting.pricegranularity";
    const BANNER_PRICE_GRANULARITY_SCOPE: &str =
        "ext.prebid.targeting.mediatypepricegranularity.banner";
    use auctioneer_privacy::gcl::Gcl;
    use auctioneer_privacy::gvl::GvlStore;
    use auctioneer_runtime::{Clock as _, FakeClock, SeededRng};
    use std::collections::{BTreeMap, BTreeSet};
    use std::time::Duration;

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
                .map(|_| BidderCallResult {
                    result: Ok(BidderHttpResponse::new(200, vec![], self.body.clone())),
                    elapsed: Duration::from_millis(10),
                })
                .collect())
        }
    }

    fn leak(name: &'static str, endpoint: &'static str) -> &'static BidderInfo {
        Box::leak(Box::new(BidderInfo {
            name: name.into(),
            endpoint: endpoint.into(),
            uses_https: true,
            capabilities: Capabilities {
                site: vec![
                    MediaType::Banner,
                    MediaType::Video,
                    MediaType::Native,
                    MediaType::Audio,
                ],
                app: vec![
                    MediaType::Banner,
                    MediaType::Video,
                    MediaType::Native,
                    MediaType::Audio,
                ],
                dooh: vec![],
            },
            ..BidderInfo::default()
        }))
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
                max_parsed_nodes: 0x0001_0000,
            },
            ..HostConfig::default()
        }
    }

    fn base_request() -> BidRequest {
        let mut bidders = serde_json::Map::new();
        bidders.insert("fake_bb".into(), serde_json::json!({}));
        BidRequest {
            id: "auc-prefix".into(),
            imp: vec![Imp {
                id: "imp-1".into(),
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
            cur: vec!["USD".into()],
            ..BidRequest::default()
        }
    }

    async fn drive(request: BidRequest, account: &AccountConfig) -> BidResponse {
        // Raw byte-string fixture: this is a wire body, a `&[u8]`/`Vec<u8>`.
        // `serde_json::json!` yields a `Value`, so converting would mean
        // `.to_string().into_bytes()` at the call site for no gain.
        let body = br#"{"id":"r","seatbid":[{"seat":"fake_bb","bid":[{"id":"b1","impid":"imp-1","price":1.0,"w":300,"h":250}]}],"cur":"USD"}"#.to_vec();
        drive_with_body(request, account, body).await
    }

    async fn drive_with_body(
        request: BidRequest,
        account: &AccountConfig,
        body: Vec<u8>,
    ) -> BidResponse {
        drive_with_bidders(request, account, body, &["fake_bb"]).await
    }

    async fn drive_with_bidders(
        request: BidRequest,
        account: &AccountConfig,
        body: Vec<u8>,
        bidders: &[&'static str],
    ) -> BidResponse {
        drive_with_adapters(
            request,
            account,
            body,
            bidders
                .iter()
                .map(|&name| {
                    let adapter: Box<dyn Bidder> = Box::new(FakeBidder::default());
                    (name, adapter)
                })
                .collect(),
        )
        .await
    }

    async fn drive_with_adapters(
        request: BidRequest,
        account: &AccountConfig,
        body: Vec<u8>,
        adapters: Vec<(&'static str, Box<dyn Bidder>)>,
    ) -> BidResponse {
        let mut registry = BidderRegistry::new();
        for (bidder, adapter) in adapters {
            registry
                .register(leak(bidder, "https://demo.example.test/openrtb"), adapter)
                .expect("unique test bidder");
        }
        let host = host();
        let resolved = ResolvedAccount {
            id: "test".into(),
            source: AccountSource::DefaultFallback,
            config: account,
        };
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let rng = SeededRng::new(42);
        let normalized =
            prepare(request, &RequestHeaders::default(), &IMP_CAP_NOT_UNDER_TEST).unwrap();
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
        run_auction(&normalized, &ctx, &plan, &mut audit)
            .await
            .unwrap()
    }

    #[test]
    fn mixed_media_response_preserves_winner_targeting_and_bid_local_errors() {
        block_on(async {
            let adapters: [(&'static str, Box<dyn Bidder>); 3] = [
                ("fake_bb", Box::new(FakeBidder::default())),
                ("fake_aaa", Box::new(FakeBidder::default())),
                ("fake_ccc", Box::new(FakeBidder::default())),
            ];
            for (name, adapter) in adapters {
                let mut request = base_request();
                request.imp[0].video = Some(Video::default());
                request.imp[0].ext.as_mut().unwrap().bidder = JsonField::Value(serde_json::json!({
                    name: {},
                }));
                let mut targeting = ExtRequestTargeting {
                    includeformat: Some(true),
                    pricegranularity: Some(serde_json::json!("low")),
                    ..ExtRequestTargeting::default()
                };
                targeting.other.insert(
                    "mediatypepricegranularity".into(),
                    serde_json::json!({"video": "high"}),
                );
                request.ext = Some(BidRequestExt {
                    prebid: Some(ExtRequestPrebid {
                        targeting: Some(targeting),
                        ..Default::default()
                    }),
                    ..Default::default()
                });
                let body = serde_json::to_vec(&serde_json::json!({
                    "id": "r", "cur": "USD", "seatbid": [
                        {"seat": "fake_first", "bid": [
                            {"id": "bad-first", "impid": "imp-1", "price": 99.0_f64, "mtype": 0_i32},
                            {"id": "video-winner", "impid": "imp-1", "price": 9.5_f64, "mtype": 2_i32},
                        ]},
                        {"seat": "fake_second", "bid": [
                            {"id": "banner-sibling", "impid": "imp-1", "price": 0.75_f64, "mtype": 1_i32},
                            {"id": "bad-second", "impid": "imp-1", "price": 100.0_f64, "mtype": 3_i32},
                        ]},
                    ],
                })).unwrap();
                let response = drive_with_adapters(
                    request,
                    &AccountConfig::default(),
                    body,
                    vec![(name, adapter)],
                )
                .await;
                let wire = serde_json::to_value(&response).unwrap();
                assert_eq!(
                    response.seatbid.len(),
                    1,
                    "{name}: preserve classified bids"
                );
                assert_eq!(
                    response.seatbid[0].bid.len(),
                    2,
                    "{name}: one rejection per invalid bid"
                );
                let winner = response.seatbid[0]
                    .bid
                    .iter()
                    .find(|bid| bid.id == "video-winner")
                    .unwrap();
                let winner_targeting = &winner
                    .ext
                    .as_ref()
                    .unwrap()
                    .prebid
                    .as_ref()
                    .unwrap()
                    .targeting;
                assert_eq!(
                    winner_targeting.get("hb_format").map(String::as_str),
                    Some("video"),
                    "{name}"
                );
                assert_eq!(
                    winner_targeting.get("hb_pb").map(String::as_str),
                    Some("9.50"),
                    "{name}"
                );
                assert_eq!(
                    winner_targeting
                        .get(&format!("hb_format_{name}"))
                        .map(String::as_str),
                    Some("video")
                );
                let errors = wire["ext"]["errors"][name].as_array().unwrap();
                assert_eq!(
                    errors.len(),
                    2,
                    "{name}: both bid-local errors reach the wire"
                );
                assert!(errors.iter().all(|error| error["code"] == 4_i32));
            }
        });
    }

    #[test]
    fn response_classification_uses_post_fpd_formats() {
        block_on(async {
            for name in ["fake_bb", "fake_aaa", "fake_ccc"] {
                for (original_banner, patch, fields, expect_error) in [
                    (
                        false,
                        serde_json::json!({"banner": {"w": 300_i32, "h": 250_i32}}),
                        serde_json::json!({"mtype": 1_i32}),
                        false,
                    ),
                    (
                        true,
                        serde_json::json!({"video": null}),
                        serde_json::json!({}),
                        false,
                    ),
                    (
                        true,
                        serde_json::json!({"video": null}),
                        serde_json::json!({"mtype": 2_i32}),
                        true,
                    ),
                ] {
                    let adapter: Box<dyn Bidder> = Box::new(FakeBidder::default());
                    let mut request = base_request();
                    request.imp[0].video = Some(Video::default());
                    if !original_banner {
                        request.imp[0].banner = None;
                    }
                    request.imp[0].ext = Some(
                        serde_json::from_value(serde_json::json!({
                            "bidder": {name: {}},
                            "prebid": {"imp": {name: patch}},
                        }))
                        .unwrap(),
                    );
                    let mut bid =
                        serde_json::json!({"id": "result", "impid": "imp-1", "price": 9.5_f64});
                    bid.as_object_mut()
                        .unwrap()
                        .extend(fields.as_object().unwrap().clone());
                    let body = serde_json::to_vec(&serde_json::json!({"id": "r", "cur": "USD", "seatbid": [{"seat": name, "bid": [bid]}]})).unwrap();
                    let response = drive_with_adapters(
                        request,
                        &AccountConfig::default(),
                        body,
                        vec![(name, adapter)],
                    )
                    .await;
                    let wire = serde_json::to_value(&response).unwrap();
                    if expect_error {
                        assert!(
                            response.seatbid.is_empty(),
                            "{name}: removed media cannot be admitted"
                        );
                        assert_eq!(wire["ext"]["errors"][name][0]["code"], 4_i32);
                    } else {
                        assert_eq!(
                            response.seatbid.len(),
                            1,
                            "{name}: post-FPD banner must be classified"
                        );
                        assert_eq!(response.seatbid[0].bid[0].id, "result");
                        assert!(wire["ext"]["errors"][name].is_null());
                    }
                }
            }
        });
    }

    fn extract_targeting_keys(resp: &BidResponse) -> Vec<String> {
        resp.seatbid
            .iter()
            .flat_map(|seat| seat.bid.iter())
            .filter_map(|bid| bid.ext.as_ref())
            .filter_map(|ext| ext.prebid.as_ref())
            .flat_map(|prebid| prebid.targeting.keys().cloned())
            .collect()
    }

    fn wire_winners(response: &BidResponse) -> Vec<(&str, &str)> {
        response
            .seatbid
            .iter()
            .flat_map(|seat| {
                seat.bid.iter().filter_map(move |bid| {
                    bid.ext
                        .as_ref()?
                        .prebid
                        .as_ref()?
                        .targeting
                        .contains_key("hb_bidder")
                        .then_some((seat.seat.as_str(), bid.id.as_str()))
                })
            })
            .collect()
    }

    #[test]
    fn equal_prices_keep_the_first_bid_in_bidder_then_bid_order() {
        block_on(async {
            for (bidders, expected_bidder) in [
                (&["fake_bb"][..], "fake_bb"),
                (&["fake_aa", "fake_bb"][..], "fake_aa"),
            ] {
                let mut request = base_request();
                request.imp[0].ext.as_mut().unwrap().bidder =
                    JsonField::Value(serde_json::Value::Object(
                        bidders
                            .iter()
                            .map(|name| ((*name).into(), serde_json::json!({})))
                            .collect(),
                    ));
                let body = serde_json::to_vec(&serde_json::json!({
                    "id": "tie", "cur": "USD", "seatbid": [{"bid": [
                        {"id": "first", "impid": "imp-1", "price": 1.25_f64, "mtype": 1_i32},
                        {"id": "second", "impid": "imp-1", "price": 1.25_f64, "mtype": 1_i32}
                    ]}]
                }))
                .unwrap();
                let response =
                    drive_with_bidders(request, &AccountConfig::default(), body, bidders).await;
                assert_eq!(response.seatbid.len(), bidders.len());
                for seat in &response.seatbid {
                    assert_eq!(
                        seat.bid
                            .iter()
                            .map(|bid| bid.id.as_str())
                            .collect::<Vec<_>>(),
                        ["first", "second"]
                    );
                }
                assert_eq!(wire_winners(&response), [(expected_bidder, "first")]);
            }
        });
    }

    #[test]
    fn request_deal_preference_changes_only_the_selected_local_bid() {
        block_on(async {
            for (prefer_deals, expected_id) in [(false, "higher"), (true, "deal")] {
                let mut request = base_request();
                request.ext = Some(BidRequestExt {
                    prebid: Some(ExtRequestPrebid {
                        targeting: Some(ExtRequestTargeting {
                            preferdeals: Some(prefer_deals),
                            ..ExtRequestTargeting::default()
                        }),
                        ..ExtRequestPrebid::default()
                    }),
                    ..BidRequestExt::default()
                });
                let body = serde_json::to_vec(&serde_json::json!({
                    "id": "deal-preference", "cur": "USD", "seatbid": [{"bid": [
                        {"id": "higher", "impid": "imp-1", "price": 5.0_f64, "mtype": 1_i32},
                        {"id": "deal", "impid": "imp-1", "price": 1.0_f64, "dealid": "fake_deal", "mtype": 1_i32}
                    ]}]
                })).unwrap();
                let response = drive_with_body(request, &AccountConfig::default(), body).await;
                assert_eq!(
                    response.seatbid[0]
                        .bid
                        .iter()
                        .map(|bid| bid.id.as_str())
                        .collect::<Vec<_>>(),
                    ["higher", "deal"]
                );
                assert_eq!(wire_winners(&response), [("fake_bb", expected_id)]);
            }
        });
    }

    #[test]
    fn disabling_winner_keys_preserves_bids_and_bidder_keys() {
        block_on(async {
            for include_winners in [true, false] {
                let mut request = base_request();
                request.ext = Some(BidRequestExt {
                    prebid: Some(ExtRequestPrebid {
                        targeting: Some(ExtRequestTargeting {
                            includewinners: Some(include_winners),
                            includebidderkeys: Some(true),
                            ..ExtRequestTargeting::default()
                        }),
                        ..ExtRequestPrebid::default()
                    }),
                    ..BidRequestExt::default()
                });
                let body = serde_json::to_vec(&serde_json::json!({
                    "id": "winner-keys", "cur": "USD", "seatbid": [{"bid": [
                        {"id": "lower", "impid": "imp-1", "price": 1.0_f64, "mtype": 1_i32},
                        {"id": "higher", "impid": "imp-1", "price": 2.0_f64, "mtype": 1_i32}
                    ]}]
                }))
                .unwrap();
                let response = drive_with_body(request, &AccountConfig::default(), body).await;
                assert_eq!(response.seatbid.len(), 1);
                let bids = &response.seatbid[0].bid;
                assert_eq!(
                    bids.iter().map(|bid| bid.id.as_str()).collect::<Vec<_>>(),
                    ["lower", "higher"]
                );
                for (bid, price_bucket) in bids.iter().zip(["1.00", "2.00"]) {
                    let targeting = &bid.ext.as_ref().unwrap().prebid.as_ref().unwrap().targeting;
                    assert_eq!(
                        targeting.get("hb_bidder_fake_bb").map(String::as_str),
                        Some("fake_bb")
                    );
                    assert_eq!(
                        targeting.get("hb_pb_fake_bb").map(String::as_str),
                        Some(price_bucket)
                    );
                    assert_eq!(
                        targeting.get("hb_pb").map(String::as_str),
                        (include_winners && bid.id == "higher").then_some("2.00")
                    );
                }
                let expected = if include_winners {
                    vec![("fake_bb", "higher")]
                } else {
                    Vec::new()
                };
                assert_eq!(wire_winners(&response), expected);
            }
        });
    }

    async fn assert_auction_prefix(
        prefix: &str,
        truncate: Option<usize>,
        expected_prefix: &str,
        expect_fallback: bool,
    ) {
        let account = AccountConfig {
            targeting_prefix: "hb".into(),
            truncate_target_attribute: truncate,
            bid_rounding: "down".into(),
            ..AccountConfig::default()
        };
        let mut request = base_request();
        request.ext = Some(BidRequestExt {
            prebid: Some(ExtRequestPrebid {
                targeting: Some(ExtRequestTargeting {
                    prefix: Some(prefix.into()),
                    ..ExtRequestTargeting::default()
                }),
                ..ExtRequestPrebid::default()
            }),
            ..BidRequestExt::default()
        });
        let resp = Box::pin(drive(request, &account)).await;
        let warnings = resp
            .ext
            .as_ref()
            .and_then(|ext| ext.pointer("/warnings/fake_bb"))
            .and_then(serde_json::Value::as_array);
        let prefix_warnings: Vec<_> = warnings
            .into_iter()
            .flatten()
            .filter(|warning| warning["code"].as_u64() == Some(10015))
            .collect();
        assert_eq!(
            prefix_warnings.len(),
            usize::from(expect_fallback),
            "unexpected prefix warnings for {}-byte prefix: {:?}",
            prefix.len(),
            resp.ext,
        );
        for warning in prefix_warnings {
            let message = warning["message"].as_str().unwrap();
            assert!(message.contains("falling back"));
            assert!(message.len() <= 384);
        }

        let keys = extract_targeting_keys(&resp);
        let cap = truncate.unwrap_or(20);
        let key_prefix = format!("{expected_prefix}_");
        assert!(!keys.is_empty(), "the winning bid must carry targeting");
        assert!(
            keys.iter()
                .all(|key| key.starts_with(&key_prefix) && key.len() <= cap),
            "unexpected targeting keys for {}-byte prefix: {keys:?}",
            prefix.len(),
        );
        let targeting = &resp.seatbid[0].bid[0]
            .ext
            .as_ref()
            .unwrap()
            .prebid
            .as_ref()
            .unwrap()
            .targeting;
        for (suffix, value) in [
            ("_pb", "1.00"),
            ("_pb_fake_bb", "1.00"),
            ("_bidder", "fake_bb"),
            ("_size", "300x250"),
        ] {
            assert_eq!(
                targeting
                    .get(&format!("{expected_prefix}{suffix}"))
                    .map(String::as_str),
                Some(value),
            );
        }
    }

    #[test]
    fn invalid_custom_prices_fall_back_without_corrupt_hb_pb_targeting() {
        block_on(async {
            let account = AccountConfig {
                bid_rounding: "down".into(),
                ..AccountConfig::default()
            };
            let invalid_ranges = [
                serde_json::json!([{"min": 3.0_f64, "max": 2.0_f64, "increment": 0.1_f64}]),
                serde_json::json!([
                    {"max": 2.0_f64, "increment": 0.1_f64},
                    {"min": 0.0_f64, "max": 5.0_f64, "increment": 5.0_f64}
                ]),
                serde_json::json!([
                    {"max": 0.5_f64, "increment": 0.1_f64},
                    {"min": 2.0_f64, "max": 5.0_f64, "increment": 0.5_f64}
                ]),
                serde_json::json!([{"max": 2.0_f64, "increment": f64::from_bits(1)}]),
            ];
            for ranges in invalid_ranges {
                let mut request = base_request();
                request.ext = Some(BidRequestExt {
                    prebid: Some(ExtRequestPrebid {
                        targeting: Some(ExtRequestTargeting {
                            pricegranularity: Some(serde_json::json!({"ranges": ranges})),
                            ..ExtRequestTargeting::default()
                        }),
                        ..ExtRequestPrebid::default()
                    }),
                    ..BidRequestExt::default()
                });
                let response = Box::pin(drive(request, &account)).await;
                let targeting = &response.seatbid[0].bid[0]
                    .ext
                    .as_ref()
                    .unwrap()
                    .prebid
                    .as_ref()
                    .unwrap()
                    .targeting;
                for key in ["hb_pb", "hb_pb_fake_bb"] {
                    assert_eq!(targeting.get(key).map(String::as_str), Some("1.00"));
                }
            }
        });
    }

    fn request_with_price_granularity(
        granularity: serde_json::Value,
        media_override: bool,
    ) -> BidRequest {
        let mut targeting = ExtRequestTargeting {
            includeformat: Some(true),
            ..ExtRequestTargeting::default()
        };
        if media_override {
            targeting.pricegranularity = Some(serde_json::json!("low"));
            targeting.other.insert(
                "mediatypepricegranularity".into(),
                serde_json::json!({"banner": granularity}),
            );
        } else {
            targeting.pricegranularity = Some(granularity);
        }
        let mut request = base_request();
        request.ext = Some(BidRequestExt {
            prebid: Some(ExtRequestPrebid {
                targeting: Some(targeting),
                ..ExtRequestPrebid::default()
            }),
            ..BidRequestExt::default()
        });
        request
    }

    fn price_granularity_warnings(response: &BidResponse) -> Vec<&serde_json::Value> {
        response
            .ext
            .as_ref()
            .and_then(|ext| ext.get("warnings"))
            .and_then(serde_json::Value::as_object)
            .into_iter()
            .flat_map(|warnings| warnings.values())
            .filter_map(serde_json::Value::as_array)
            .flatten()
            .filter(|warning| warning["code"].as_u64() == Some(PRICE_GRANULARITY_WARNING_CODE))
            .collect()
    }

    fn assert_price_granularity_warnings(response: &BidResponse, scopes: &[&str]) {
        let all_warnings = price_granularity_warnings(response);
        assert_eq!(
            all_warnings.len(),
            scopes.len(),
            "expected one price granularity warning per invalid setting: {:?}",
            response.ext,
        );
        if scopes.is_empty() {
            return;
        }
        let prebid_warnings: Vec<_> = response
            .ext
            .as_ref()
            .and_then(|ext| ext.pointer("/warnings/prebid"))
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter(|warning| warning["code"].as_u64() == Some(PRICE_GRANULARITY_WARNING_CODE))
            .collect();
        assert_eq!(
            prebid_warnings.len(),
            all_warnings.len(),
            "price granularity warnings belong to the auction, not individual bidders",
        );
        for scope in scopes {
            let matching: Vec<_> = prebid_warnings
                .iter()
                .filter(|warning| warning["message"].as_str().unwrap().contains(scope))
                .collect();
            assert_eq!(matching.len(), 1, "missing or duplicated scope {scope}");
            let message = matching[0]["message"].as_str().unwrap();
            assert!(message.len() <= 384, "unbounded warning: {message}");
            assert!(message.contains("falling back"), "{message}");
            assert!(message.to_ascii_lowercase().contains("med"), "{message}");
            assert!(!message.contains("private-granularity-marker"), "{message}");
            assert!(
                !message.contains("diagnostic_payload"),
                "raw custom input: {message}"
            );
        }
    }

    fn priced_bid_body() -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "id": "r",
            "seatbid": [{
                "seat": "fake_bb",
                "bid": [{
                    "id": "b1", "impid": "imp-1", "price": 1.23_f64,
                    "w": 300_i32, "h": 250_i32, "dealid": "D42", "crid": "creative-1",
                    "ext": {"marker": "retained", "prebid": {"bidid": "prebid-b1"}},
                }],
            }],
            "cur": "USD",
        }))
        .unwrap()
    }

    fn invalid_price_granularity_inputs() -> Vec<serde_json::Value> {
        vec![
            serde_json::json!({}),
            serde_json::json!({"ranges": "private-granularity-marker"}),
            serde_json::json!({"ranges": []}),
            serde_json::json!({"ranges": [null, false, "private-granularity-marker"]}),
            serde_json::json!({"ranges": [
                {"min": 3.0_f64, "max": 2.0_f64, "increment": 0.1_f64},
            ]}),
            serde_json::json!({"ranges": [
                {"max": 2.0_f64, "increment": 0.01_f64},
                {"min": 5.0_f64, "max": 6.0_f64, "increment": 0.25_f64},
            ]}),
            serde_json::json!({"ranges": [
                {"max": 2.0_f64, "increment": 0.01_f64},
                {"min": 1.0_f64, "max": 3.0_f64, "increment": 0.25_f64},
            ]}),
            serde_json::json!({"ranges": [
                {"max": 2.0_f64, "increment": 0.01_f64},
                {"max": 5.0_f64, "increment": 0.0_f64},
                {"min": 5.0_f64, "max": 6.0_f64, "increment": 0.25_f64},
            ]}),
            serde_json::json!({"ranges": [
                {"max": 2.0_f64, "increment": 0.01_f64},
                {"max": 1.0_f64, "increment": 0.5_f64},
                {"min": 1.0_f64, "max": 3.0_f64, "increment": 0.25_f64},
            ]}),
            serde_json::json!({"ranges": [
                {"max": 2.0_f64, "increment": f64::from_bits(1)},
            ]}),
            serde_json::json!({"ranges": [
                {"max": 10.0_f64, "increment": f64::MIN_POSITIVE},
            ]}),
            serde_json::json!({"ranges": [
                {"max": f64::MAX, "increment": 1.0e308_f64},
            ]}),
        ]
    }

    async fn assert_invalid_price_granularity_warns_on_response(media_override: bool) {
        let account = AccountConfig {
            bid_rounding: "down".into(),
            ..AccountConfig::default()
        };
        let expected = drive_with_body(
            request_with_price_granularity(serde_json::json!("med"), media_override),
            &account,
            priced_bid_body(),
        )
        .await;
        for mut granularity in invalid_price_granularity_inputs() {
            granularity["diagnostic_payload"] =
                serde_json::json!("private-granularity-marker".repeat(256));
            let mut response = drive_with_body(
                request_with_price_granularity(granularity, media_override),
                &account,
                priced_bid_body(),
            )
            .await;
            let targeting = &response.seatbid[0].bid[0]
                .ext
                .as_ref()
                .unwrap()
                .prebid
                .as_ref()
                .unwrap()
                .targeting;
            for key in ["hb_pb", "hb_pb_fake_bb"] {
                assert_eq!(targeting.get(key).map(String::as_str), Some("1.20"));
            }
            let scope = if media_override {
                BANNER_PRICE_GRANULARITY_SCOPE
            } else {
                GLOBAL_PRICE_GRANULARITY_SCOPE
            };
            assert_price_granularity_warnings(&response, &[scope]);
            let warnings = response
                .ext
                .as_mut()
                .unwrap()
                .get_mut("warnings")
                .unwrap()
                .as_object_mut()
                .unwrap();
            let prebid_warnings = warnings.get_mut("prebid").unwrap().as_array_mut().unwrap();
            prebid_warnings
                .retain(|warning| warning["code"].as_u64() != Some(PRICE_GRANULARITY_WARNING_CODE));
            if prebid_warnings.is_empty() {
                warnings.remove("prebid");
            }
            if warnings.is_empty() {
                response
                    .ext
                    .as_mut()
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove("warnings");
            }
            assert_eq!(
                serde_json::to_value(&response).unwrap(),
                serde_json::to_value(&expected).unwrap(),
                "fallback may only add the warning, not change the Med auction response",
            );
        }
    }

    #[test]
    fn invalid_global_price_granularity_warns_once_on_auction_response() {
        block_on(assert_invalid_price_granularity_warns_on_response(false));
    }

    #[test]
    fn invalid_media_price_granularity_warns_once_on_auction_response() {
        block_on(assert_invalid_price_granularity_warns_on_response(true));
    }

    #[test]
    fn price_granularity_warnings_are_bounded_by_settings_not_bid_count() {
        block_on(async {
            let account = AccountConfig {
                bid_rounding: "down".into(),
                ..AccountConfig::default()
            };
            let invalid = serde_json::json!({
                "ranges": vec![serde_json::json!({
                    "min": "private-granularity-marker", "max": 3.0_f64,
                    "increment": 0.25_f64,
                }); 64],
            });
            let mut request = request_with_price_granularity(invalid.clone(), false);
            let targeting = request
                .ext
                .as_mut()
                .unwrap()
                .prebid
                .as_mut()
                .unwrap()
                .targeting
                .as_mut()
                .unwrap();
            let mut media = serde_json::Map::new();
            for key in ["banner", "video", "native", "audio"] {
                media.insert(key.into(), invalid.clone());
            }
            for index in 0_usize..32 {
                media.insert(
                    format!("private-granularity-marker-{index}"),
                    serde_json::json!("private-granularity-marker"),
                );
            }
            targeting.other.insert(
                "mediatypepricegranularity".into(),
                serde_json::Value::Object(media),
            );
            let mut template_imp = request.imp[0].clone();
            template_imp.ext.as_mut().unwrap().bidder.as_mut().unwrap()["fake_other"] =
                serde_json::json!({});
            request.imp = (0_usize..4)
                .map(|index| {
                    let mut imp = template_imp.clone();
                    imp.id = format!("imp-{index}");
                    imp
                })
                .collect();
            let bids: Vec<_> = request
                .imp
                .iter()
                .flat_map(|imp| {
                    (0_usize..8).map(move |index| {
                        serde_json::json!({
                            "id": format!("{}-bid-{index}", imp.id), "impid": imp.id,
                            "price": 1.23_f64, "w": 300_i32, "h": 250_i32,
                        })
                    })
                })
                .collect();
            let body = serde_json::to_vec(&serde_json::json!({
                "id": "r", "seatbid": [{"seat": "fake_bb", "bid": bids}], "cur": "USD",
            }))
            .unwrap();
            let response =
                drive_with_bidders(request, &account, body, &["fake_bb", "fake_other"]).await;
            assert_eq!(response.seatbid.len(), 2);
            for seat in &response.seatbid {
                assert_eq!(seat.bid.len(), 32);
                for bid in &seat.bid {
                    let bid_targeting =
                        &bid.ext.as_ref().unwrap().prebid.as_ref().unwrap().targeting;
                    assert_eq!(
                        bid_targeting
                            .get(&format!("hb_pb_{}", seat.seat))
                            .map(String::as_str),
                        Some("1.20"),
                    );
                }
            }
            assert_price_granularity_warnings(
                &response,
                &[
                    GLOBAL_PRICE_GRANULARITY_SCOPE,
                    BANNER_PRICE_GRANULARITY_SCOPE,
                    "ext.prebid.targeting.mediatypepricegranularity.video",
                    "ext.prebid.targeting.mediatypepricegranularity.native",
                    "ext.prebid.targeting.mediatypepricegranularity.audio",
                ],
            );
        });
    }

    #[test]
    fn valid_media_price_granularity_overrides_invalid_global_fallback() {
        block_on(async {
            let account = AccountConfig {
                bid_rounding: "down".into(),
                ..AccountConfig::default()
            };
            let mut request =
                request_with_price_granularity(serde_json::json!({"ranges": []}), false);
            request
                .ext
                .as_mut()
                .unwrap()
                .prebid
                .as_mut()
                .unwrap()
                .targeting
                .as_mut()
                .unwrap()
                .other
                .insert(
                    "mediatypepricegranularity".into(),
                    serde_json::json!({"banner": {
                        "precision": 3_u32,
                        "ranges": [{"max": 3.0_f64, "increment": 0.01_f64}],
                    }}),
                );
            let response = drive_with_body(request, &account, priced_bid_body()).await;
            let targeting = &response.seatbid[0].bid[0]
                .ext
                .as_ref()
                .unwrap()
                .prebid
                .as_ref()
                .unwrap()
                .targeting;
            for key in ["hb_pb", "hb_pb_fake_bb"] {
                assert_eq!(targeting.get(key).map(String::as_str), Some("1.230"));
            }
            assert_price_granularity_warnings(&response, &[GLOBAL_PRICE_GRANULARITY_SCOPE]);
        });
    }

    #[test]
    fn price_granularity_fallback_warning_survives_empty_bid_response() {
        block_on(async {
            let account = AccountConfig::default();
            for media_override in [false, true] {
                let response = drive_with_body(
                    request_with_price_granularity(
                        serde_json::json!({"ranges": []}),
                        media_override,
                    ),
                    &account,
                    serde_json::to_vec(&serde_json::json!({
                        "id": "r", "seatbid": [], "cur": "USD",
                    }))
                    .unwrap(),
                )
                .await;
                assert!(response.seatbid.is_empty());
                let scope = if media_override {
                    BANNER_PRICE_GRANULARITY_SCOPE
                } else {
                    GLOBAL_PRICE_GRANULARITY_SCOPE
                };
                assert_price_granularity_warnings(&response, &[scope]);
            }
        });
    }

    #[test]
    fn valid_price_granularity_settings_do_not_emit_fallback_warnings() {
        block_on(async {
            let account = AccountConfig::default();
            assert_price_granularity_warnings(
                &Box::pin(drive(base_request(), &account)).await,
                &[],
            );
            for granularity in [
                serde_json::json!("low"),
                serde_json::json!("med"),
                serde_json::json!("medium"),
                serde_json::json!("high"),
                serde_json::json!("auto"),
                serde_json::json!("dense"),
                serde_json::json!("MeDiUm"),
                serde_json::json!({"precision": 30_u32, "ranges": [
                    {"max": 2.0_f64, "increment": 0.01_f64},
                    {"max": 3.0_f64, "increment": 0.25_f64},
                ]}),
                serde_json::json!({"ranges": [
                    {"min": 2.0_f64, "max": 3.0_f64, "increment": 0.25_f64},
                ]}),
            ] {
                for media_override in [false, true] {
                    let response = drive_with_body(
                        request_with_price_granularity(granularity.clone(), media_override),
                        &account,
                        priced_bid_body(),
                    )
                    .await;
                    assert_eq!(response.seatbid.len(), 1);
                    assert_price_granularity_warnings(&response, &[]);
                }
            }
        });
    }

    #[test]
    fn dropped_middle_rows_preserve_price_granularity_without_fallback_warning() {
        block_on(async {
            let account = AccountConfig {
                bid_rounding: "down".into(),
                ..AccountConfig::default()
            };
            let mut bid_json: serde_json::Value =
                serde_json::from_slice(&priced_bid_body()).unwrap();
            bid_json["seatbid"][0]["bid"][0]["price"] = serde_json::json!(2.63_f64);
            let body = serde_json::to_vec(&bid_json).unwrap();
            for media_override in [false, true] {
                let expected = drive_with_body(
                    request_with_price_granularity(
                        serde_json::json!({"ranges": [
                            {"max": 2.0_f64, "increment": 0.01_f64},
                            {"max": 3.0_f64, "increment": 0.25_f64},
                        ]}),
                        media_override,
                    ),
                    &account,
                    body.clone(),
                )
                .await;
                for row in [
                    serde_json::json!(null),
                    serde_json::json!({"max": 5.0_f64, "increment": 0.0_f64}),
                    serde_json::json!({"max": 1.0_f64, "increment": 0.5_f64}),
                    serde_json::json!({"min": "private-granularity-marker", "max": 5.0_f64, "increment": 0.5_f64}),
                    serde_json::json!({"min": -1.0_f64, "max": 5.0_f64, "increment": 0.5_f64}),
                ] {
                    let response = drive_with_body(
                        request_with_price_granularity(
                            serde_json::json!({"ranges": [
                                {"max": 2.0_f64, "increment": 0.01_f64},
                                row,
                                {"max": 3.0_f64, "increment": 0.25_f64},
                            ]}),
                            media_override,
                        ),
                        &account,
                        body.clone(),
                    )
                    .await;
                    assert_price_granularity_warnings(&response, &[]);
                    assert_eq!(
                        serde_json::to_value(&response).unwrap(),
                        serde_json::to_value(&expected).unwrap()
                    );
                }
            }
        });
    }

    async fn assert_empty_price_bucket_preserves_auction(media_override: bool) {
        const WINNER_ID: &str = "winner";
        let body = serde_json::to_vec(&serde_json::json!({
            "id": "r",
            "seatbid": [{
                "seat": "fake_bb",
                "bid": [
                    {
                        "id": WINNER_ID, "impid": "imp-1", "price": 1.23_f64,
                        "w": 300_i32, "h": 250_i32, "dealid": "D42", "crid": "creative-1",
                        "ext": {"marker": "winner", "prebid": {"bidid": "prebid-winner"}},
                    },
                    {
                        "id": "runner-up", "impid": "imp-1", "price": 0.73_f64,
                        "w": 300_i32, "h": 250_i32, "dealid": "D43", "crid": "creative-2",
                        "ext": {"marker": "runner-up", "prebid": {"bidid": "prebid-runner-up"}},
                    },
                ],
            }],
            "cur": "USD",
        }))
        .unwrap();
        for rounding in ["down", "up", "nearest", "timesplit"] {
            let account = AccountConfig {
                bid_rounding: rounding.into(),
                ..AccountConfig::default()
            };
            let mut expected = drive_with_body(
                request_with_price_granularity(
                    serde_json::json!({
                        "precision": 3_u32,
                        "ranges": [{"min": 0.0_f64, "max": 3.0_f64, "increment": 0.25_f64}],
                    }),
                    media_override,
                ),
                &account,
                body.clone(),
            )
            .await;
            assert_eq!(expected.id, "auc-prefix");
            assert_eq!(expected.cur, "USD");
            assert_eq!(expected.seatbid.len(), 1);
            assert_eq!(expected.seatbid[0].seat, "fake_bb");
            assert_eq!(expected.seatbid[0].bid.len(), 2);
            for bid in &mut expected.seatbid[0].bid {
                let prebid = bid.ext.as_mut().unwrap().prebid.as_mut().unwrap();
                assert!(!prebid.targeting.remove("hb_pb_fake_bb").unwrap().is_empty());
                if bid.id == WINNER_ID {
                    assert!(!prebid.targeting.remove("hb_pb").unwrap().is_empty());
                    assert_eq!(
                        prebid.targeting.get("hb_bidder").map(String::as_str),
                        Some("fake_bb")
                    );
                    assert_eq!(
                        prebid.targeting.get("hb_deal").map(String::as_str),
                        Some("D42")
                    );
                } else {
                    assert_eq!(bid.id, "runner-up");
                    assert!(!prebid.targeting.contains_key("hb_pb"));
                    assert!(!prebid.targeting.contains_key("hb_bidder"));
                }
                for (key, value) in [
                    ("hb_bidder_fake_bb", "fake_bb"),
                    ("hb_size_fake_bb", "300x250"),
                    ("hb_format_fake_bb", "banner"),
                ] {
                    assert_eq!(prebid.targeting.get(key).map(String::as_str), Some(value));
                }
            }
            let response = drive_with_body(
                request_with_price_granularity(
                    serde_json::json!({
                        "precision": 3_u32,
                        "ranges": [{"min": 2.0_f64, "max": 3.0_f64, "increment": 0.25_f64}],
                    }),
                    media_override,
                ),
                &account,
                body.clone(),
            )
            .await;
            assert_eq!(
                serde_json::to_value(&response).unwrap(),
                serde_json::to_value(&expected).unwrap(),
                "only price targeting may change: {rounding}, media override: {media_override}",
            );
        }
    }

    #[test]
    fn empty_global_price_bucket_preserves_other_targeting_bids_and_identity() {
        block_on(assert_empty_price_bucket_preserves_auction(false));
    }

    #[test]
    fn empty_media_price_bucket_preserves_other_targeting_bids_and_identity() {
        block_on(assert_empty_price_bucket_preserves_auction(true));
    }

    async fn assert_malformed_price_row_preserves_auction_targeting(media_override: bool) {
        let account = AccountConfig {
            bid_rounding: "down".into(),
            ..AccountConfig::default()
        };
        let granularity = serde_json::json!({
            "ranges": [
                {"max": 2.0_f64, "increment": 0.01_f64},
                {"min": -1.0_f64, "max": 3.0_f64, "increment": 0.1_f64},
            ],
        });
        let mut request_targeting = ExtRequestTargeting::default();
        if media_override {
            request_targeting.pricegranularity = Some(serde_json::json!("low"));
            request_targeting.other.insert(
                "mediatypepricegranularity".into(),
                serde_json::json!({"banner": granularity}),
            );
        } else {
            request_targeting.pricegranularity = Some(granularity);
        }
        let mut request = base_request();
        request.ext = Some(BidRequestExt {
            prebid: Some(ExtRequestPrebid {
                targeting: Some(request_targeting),
                ..ExtRequestPrebid::default()
            }),
            ..BidRequestExt::default()
        });
        let body = serde_json::to_vec(&serde_json::json!({
            "id": "r",
            "seatbid": [{
                "seat": "fake_bb",
                "bid": [{
                    "id": "b1", "impid": "imp-1", "price": 1.23_f64,
                    "w": 300_i32, "h": 250_i32,
                }],
            }],
            "cur": "USD",
        }))
        .unwrap();
        let response = drive_with_body(request, &account, body).await;
        let targeting = &response.seatbid[0].bid[0]
            .ext
            .as_ref()
            .unwrap()
            .prebid
            .as_ref()
            .unwrap()
            .targeting;
        for key in ["hb_pb", "hb_pb_fake_bb"] {
            assert_eq!(targeting.get(key).map(String::as_str), Some("1.23"));
        }
    }

    #[test]
    fn malformed_global_price_row_preserves_winner_and_bidder_targeting() {
        block_on(assert_malformed_price_row_preserves_auction_targeting(
            false,
        ));
    }

    #[test]
    fn malformed_media_price_row_preserves_winner_and_bidder_targeting() {
        block_on(assert_malformed_price_row_preserves_auction_targeting(true));
    }

    #[test]
    fn auction_accepts_eight_byte_prefix_at_default_cap_without_warning() {
        block_on(assert_auction_prefix("abcdefgh", None, "abcdefgh", false));
    }

    #[test]
    fn auction_falls_back_for_nine_byte_prefix_at_default_cap_with_warning() {
        block_on(assert_auction_prefix("abcdefghi", None, "hb", true));
    }

    #[test]
    fn auction_accepts_four_byte_prefix_at_sixteen_byte_cap_without_warning() {
        block_on(assert_auction_prefix("abcd", Some(16), "abcd", false));
    }

    #[test]
    fn auction_falls_back_for_five_byte_prefix_at_sixteen_byte_cap_with_warning() {
        block_on(assert_auction_prefix("abcde", Some(16), "hb", true));
    }

    #[test]
    fn auction_resolves_multibyte_prefixes_by_byte_length() {
        block_on(async {
            for (prefix, truncate, expected, fallback) in [
                ("\u{1f600}\u{1f600}", None, "\u{1f600}\u{1f600}", false),
                ("\u{e9}\u{e9}\u{e9}\u{20ac}", None, "hb", true),
                ("\u{1f600}", Some(16), "\u{1f600}", false),
                ("\u{e9}\u{20ac}", Some(16), "hb", true),
            ] {
                Box::pin(assert_auction_prefix(prefix, truncate, expected, fallback)).await;
            }
        });
    }

    #[test]
    fn account_targeting_prefix_used_when_request_omits_it() {
        block_on(async {
            let account = AccountConfig {
                targeting_prefix: "myprefix".into(),
                bid_rounding: "down".into(),
                ..AccountConfig::default()
            };
            // Request has no ext.prebid.targeting.prefix — account
            // fallback must win.
            let resp = Box::pin(drive(base_request(), &account)).await;
            let keys = extract_targeting_keys(&resp);
            assert!(
                keys.iter().any(|k| k.starts_with("myprefix_")),
                "expected account prefix `myprefix_*` in emitted keys, got {keys:?}",
            );
        });
    }

    #[test]
    fn request_prefix_overrides_account_prefix() {
        block_on(async {
            let account = AccountConfig {
                targeting_prefix: "account".into(),
                bid_rounding: "down".into(),
                ..AccountConfig::default()
            };
            let mut request = base_request();
            request.ext = Some(BidRequestExt {
                prebid: Some(ExtRequestPrebid {
                    targeting: Some(ExtRequestTargeting {
                        prefix: Some("req".into()),
                        ..ExtRequestTargeting::default()
                    }),
                    ..ExtRequestPrebid::default()
                }),
                ..BidRequestExt::default()
            });
            let resp = Box::pin(drive(request, &account)).await;
            let keys = extract_targeting_keys(&resp);
            assert!(
                keys.iter().any(|k| k.starts_with("req_")),
                "expected request prefix `req_*` to win, got {keys:?}",
            );
            assert!(
                !keys.iter().any(|k| k.starts_with("account_")),
                "account prefix must NOT appear when the request overrides, got {keys:?}",
            );
        });
    }
}
