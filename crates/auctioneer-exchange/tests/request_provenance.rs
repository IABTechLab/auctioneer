//! Trusted caller context through normalization, privacy and bidder scoping.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

#[cfg(test)]
mod support;

#[cfg(test)]
mod tests {
    use crate::fake_bidder::FakeBidder;
    use crate::support::response_budgets;
    use async_trait::async_trait;
    use auctioneer_bidder::{
        BidderRegistry, BidderRequestInfo, DebugBodySchema, EntryPoint, FloorConversion,
    };
    use auctioneer_config::currency::FloorCurrencyConfig;
    use auctioneer_config::types::{AccountConfig, BidderInfo, HostConfig, ResourceLimits};
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::{
        AccountSource, AuctionContext, AuctionDeadline, AuctionPlan, BidderCall, BidderCallResult,
        BidderFanout, ExchangeFeatures, FanoutInternalError, ResolvedAccount, resolve_aliases,
        run_auction,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_openrtb::{NormalizedBidRequest, PrepareLimits, RequestHeaders, prepare};
    use auctioneer_privacy::{gcl::Gcl, gvl::GvlStore};
    use auctioneer_runtime::bounded_json::{JsonFieldShape, count_json_nodes_with_field};
    use auctioneer_runtime::{Clock as _, FakeClock, SeededRng};
    use futures::executor::block_on;
    use serde_json::{Value, json};
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const BASE: &str = "fake_fx";
    const ALIASES: [&str; 2] = ["fake_first", "fake_second"];
    const NODE_CAP: u32 = 1_024;

    #[test]
    fn both_aliases_receive_original_root_shape_after_normalization_and_privacy_scrubbing() {
        for (ext, expected) in [
            (None, JsonFieldShape::Absent),
            (Some(Value::Null), JsonFieldShape::Null),
            (Some(json!({})), JsonFieldShape::Object),
        ] {
            let (request, original_shape) = prepare_raw(&raw_request(ext));
            assert_eq!(original_shape, expected);
            // Absent and explicit null share the same typed representation.
            assert_eq!(request.ext.is_some(), expected == JsonFieldShape::Object);
            drive(&request, Some(original_shape));
        }
    }

    #[test]
    fn original_object_shape_survives_removal_of_the_typed_root_extension() {
        let (request, original_shape) =
            prepare_raw(&raw_request(Some(json!({"original": {"marker": true}}))));
        assert_eq!(original_shape, JsonFieldShape::Object);
        let mut transformed = request.into_inner();
        assert!(transformed.ext.take().is_some());
        let normalized_transformed = prepare_typed(transformed);
        assert!(normalized_transformed.ext.is_none());
        drive(&normalized_transformed, Some(original_shape));
    }

    #[test]
    fn typed_only_context_leaves_original_shape_unavailable_instead_of_inferring_absence() {
        let typed: BidRequest = serde_json::from_value(raw_request(None)).unwrap();
        let request = prepare_typed(typed);
        assert!(request.ext.is_none());
        drive(&request, None);
    }

    #[test]
    fn trusted_entrypoint_reaches_both_aliases_after_privacy_and_scoping() {
        for entry_point in [EntryPoint::Auction, EntryPoint::Amp, EntryPoint::Video] {
            for hint in [None, Some("amp"), Some("video")] {
                for video in [false, true] {
                    for full_constructor in [false, true] {
                        let raw = request_with_entrypoint_hints(hint, video);
                        let (request, shape) = prepare_raw(&raw);
                        drive_with_entrypoint(
                            &request,
                            Some(shape),
                            Some(entry_point),
                            full_constructor,
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn body_hints_and_video_inventory_cannot_change_default_auction_context() {
        for hint in [None, Some("amp"), Some("video")] {
            for video in [false, true] {
                for full_constructor in [false, true] {
                    let raw = request_with_entrypoint_hints(hint, video);
                    let (request, shape) = prepare_raw(&raw);
                    drive_with_entrypoint(&request, Some(shape), None, full_constructor);
                }
            }
        }
    }

    #[test]
    fn operator_floor_rates_reach_each_alias_through_both_context_constructors() {
        let (request, shape) = prepare_raw(&raw_request(None));
        for full_constructor in [false, true] {
            for (amount, rate, expected) in [
                (2.0_f64, 0.25_f64, 0.5_f64),
                (4.0_f64, 0.25_f64, 1.0_f64),
                (2.0_f64, 2.5_f64, 5.0_f64),
                (4.0_f64, 2.5_f64, 10.0_f64),
            ] {
                let snapshot = FloorCurrencyConfig {
                    expires_at: 2_000,
                    usd_per_unit: BTreeMap::from([("QAA".to_owned(), rate)]),
                };
                drive_scoped(
                    &request,
                    Some(shape),
                    None,
                    full_constructor,
                    Some((&snapshot, amount, expected)),
                );
            }
        }
    }

    fn request_with_entrypoint_hints(hint: Option<&str>, video: bool) -> Value {
        let mut raw = raw_request(hint.map(|value| json!({"entrypoint": value})));
        if let Some(value) = hint {
            raw["entry_point"] = json!(value);
        }
        if video {
            raw["imp"][0]["video"] = json!({"mimes": ["video/mp4"]});
        }
        raw
    }

    fn raw_request(ext: Option<Value>) -> Value {
        let mut request = json!({
            "id": "request-provenance",
            "site": {"id": "site"},
            "regs": {"gdpr": 0_i32, "coppa": 1_i32},
            "user": {"id": "private-user", "ext": {"uid2": "private-extension-id"}},
            "imp": [{
                "id": "imp",
                "banner": {"w": 300_i32, "h": 250_i32},
                "ext": {"prebid": {"bidder": {
                    (ALIASES[0]): {"placement": ALIASES[0]},
                    (ALIASES[1]): {"placement": ALIASES[1]}
                }}}
            }]
        });
        if let Some(value) = ext {
            request["ext"] = value;
        }
        request
    }

    fn prepare_raw(value: &Value) -> (NormalizedBidRequest, JsonFieldShape) {
        let bytes = serde_json::to_vec(value).unwrap();
        let (_, shape) = count_json_nodes_with_field(&bytes, NODE_CAP, "ext").unwrap();
        let typed = serde_json::from_slice(&bytes).unwrap();
        (prepare_typed(typed), shape)
    }

    fn prepare_typed(request: BidRequest) -> NormalizedBidRequest {
        prepare(
            request,
            &RequestHeaders::default(),
            &PrepareLimits {
                max_impressions: 1,
                ..PrepareLimits::default()
            },
        )
        .unwrap()
    }

    struct ProvenanceCheck {
        expected_shape: JsonFieldShape,
        expected_entrypoint: EntryPoint,
        root_ext_present: bool,
        calls: Arc<[AtomicUsize; 2]>,
        floor_case: Option<(f64, f64)>,
    }

    impl ProvenanceCheck {
        fn observe(&self, request: &BidRequest, info: &BidderRequestInfo<'_>) {
            assert_eq!(info.original_request_ext_shape, self.expected_shape);
            assert_eq!(info.entry_point, self.expected_entrypoint);
            if let Some((amount, expected)) = self.floor_case {
                assert_eq!(
                    info.floor_currency.convert_floor_to_usd(amount, "QAA"),
                    Ok(expected)
                );
            }
            assert_eq!(info.core_name.as_str(), BASE);
            assert!(ALIASES.contains(&info.bidder.as_str()));
            assert_eq!(request.ext.is_some(), self.root_ext_present);
            assert_eq!(request.imp.len(), 1);
            for imp in &request.imp {
                assert_eq!(
                    imp.ext.as_ref().and_then(|ext| ext.bidder.as_ref()),
                    Some(&json!({"placement": info.bidder.as_str()})),
                    "bidder scoping rewrites the impression extension"
                );
            }
            assert!(
                request.user.as_ref().is_some_and(|user| {
                    user.id.is_none() && user.ext.as_ref().is_some_and(|ext| ext.other.is_empty())
                }),
                "COPPA must remove the original user and extension identifiers"
            );
            for (alias, calls) in ALIASES.iter().zip(self.calls.iter()) {
                if *alias == info.bidder.as_str() {
                    let _previous = calls.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }

    struct NoFanout;

    #[async_trait(?Send)]
    impl BidderFanout for NoFanout {
        async fn send_all(
            &self,
            _calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            panic!("no-call spy must not dispatch transport requests");
        }
    }

    fn drive(request: &NormalizedBidRequest, original_shape: Option<JsonFieldShape>) {
        drive_with_entrypoint(request, original_shape, None, false);
    }

    fn drive_with_entrypoint(
        request: &NormalizedBidRequest,
        original_shape: Option<JsonFieldShape>,
        entry_point: Option<EntryPoint>,
        full_constructor: bool,
    ) {
        drive_scoped(request, original_shape, entry_point, full_constructor, None);
    }

    fn observed_registry(check: ProvenanceCheck) -> BidderRegistry {
        let observer = Box::leak(Box::new(
            move |request: &BidRequest, info: &BidderRequestInfo<'_>| check.observe(request, info),
        ));
        let mut registry = BidderRegistry::new();
        registry
            .register(
                Box::leak(Box::new(BidderInfo {
                    name: BASE.into(),
                    endpoint: "https://bidder.example.test/auction".into(),
                    uses_https: true,
                    ..BidderInfo::default()
                })),
                Box::new(FakeBidder {
                    request_observer: Some(observer),
                    request_coverage: Some(Vec::new()),
                    debug_schema: DebugBodySchema::Unsupported,
                    ..FakeBidder::default()
                }),
            )
            .expect("unique test bidder");
        registry
    }

    fn drive_scoped(
        request: &NormalizedBidRequest,
        original_shape: Option<JsonFieldShape>,
        entry_point: Option<EntryPoint>,
        full_constructor: bool,
        floor_case: Option<(&FloorCurrencyConfig, f64, f64)>,
    ) {
        let calls = Arc::new([AtomicUsize::new(0), AtomicUsize::new(0)]);
        let registry = observed_registry(ProvenanceCheck {
            expected_shape: original_shape.unwrap_or(JsonFieldShape::Unavailable),
            expected_entrypoint: entry_point.unwrap_or(EntryPoint::Auction),
            root_ext_present: request.ext.is_some(),
            calls: Arc::clone(&calls),
            floor_case: floor_case.map(|(_, amount, expected)| (amount, expected)),
        });
        let host = HostConfig {
            resource_limits: ResourceLimits {
                default_tmax_ms: 1_000,
                max_tmax_ms: 2_000,
                max_bidder_response_bytes: 4_096,
                max_bidders_per_auction: 2,
                max_outbound_requests: 2,
                max_outbound_raw_bytes: 4_096,
                max_outbound_request_bytes: 4_096,
                max_transport_bytes: 4_096,
                max_decoded_bytes: 4_096,
                max_parsed_nodes: NODE_CAP,
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
        let mut ctx = AuctionContext::with_request_start(
            clock.now_monotonic(),
            &host,
            &account,
            &registry,
            &NoFanout,
            &clock,
            &rng,
            GvlStore::baked(),
            Gcl::baked(),
            clock.now(),
            false,
        );
        let overrides = BTreeMap::new();
        if full_constructor {
            ctx = AuctionContext::with_request_start_ext(
                clock.now_monotonic(),
                &host,
                &account,
                &registry,
                &NoFanout,
                &clock,
                &rng,
                GvlStore::baked(),
                Gcl::baked(),
                clock.now(),
                false,
                ExchangeFeatures::default(),
                &overrides,
            );
        }
        assert_eq!(ctx.original_request_ext_shape, JsonFieldShape::Unavailable);
        assert_eq!(ctx.entry_point, EntryPoint::Auction);
        assert_eq!(
            ctx.floor_currency.convert_floor_to_usd(2.0, "USD"),
            Ok(2.0_f64)
        );
        if let Some((snapshot, _, _)) = floor_case {
            ctx.floor_currency = FloorConversion::new(Some(snapshot), clock.now().unix_timestamp());
        }
        if let Some(selected) = entry_point {
            ctx.entry_point = selected;
        }
        if let Some(shape) = original_shape {
            ctx.original_request_ext_shape = shape;
        }
        let host_aliases = ALIASES
            .into_iter()
            .map(|alias| (alias.to_owned(), BASE.to_owned()))
            .collect();
        let aliases = resolve_aliases(request, &registry, &BTreeSet::new(), &host_aliases).unwrap();
        let plan = AuctionPlan::new(
            aliases,
            BTreeSet::new(),
            response_budgets(&registry, &host),
            BTreeMap::new(),
        );
        let mut audit = DisclosureAudit::default();
        let response = block_on(run_auction(request, &ctx, &plan, &mut audit)).unwrap();
        assert_eq!(response.id, request.id);
        assert!(response.seatbid.is_empty());
        assert_eq!(
            calls.each_ref().map(|count| count.load(Ordering::Relaxed)),
            [1, 1],
            "each alias must reach the spy exactly once"
        );
    }
}
