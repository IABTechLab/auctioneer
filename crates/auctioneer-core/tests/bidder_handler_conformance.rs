//! Generic bidder dispatch through core's explicit test-only HTTP entry point.

#[cfg(test)]
pub use auctioneer_core::test_support::bidder_fixture;

#[cfg(test)]
mod fanout_limits;
use fanout_limits as envelope;

#[cfg(test)]
mod tests {
    use super::bidder_fixture::fake_bidder::FakeBidder;
    use super::envelope;
    use auctioneer_bidder::{
        BidderHttpRequest, BidderHttpResponse, BidderRegistry, BidderRequestInfo,
    };
    use auctioneer_config_data::types::{BidderInfo, Compression};
    use auctioneer_core::auction::audit::construction_costs;
    use auctioneer_core::auction::{
        AuctionFunding, AuctionResourceInputs, AuctionResourceProfile, SchemaAllowance,
    };
    use auctioneer_core::handlers::{TestFanoutHandle, auction::auction_for_conformance};
    use auctioneer_core::host_primitives::{RequestClock, resolve_budget_inputs};
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_runtime::{
        AdmissionGate, AuctionBudget, CatalogResourceCosts, FakeClock, SeededRng,
        TargetPlatformConstants, bounded_json::JsonFieldShape,
    };
    use bytes::Bytes;
    use edgezero_core::{
        body::Body,
        config_store::BoundedStoreRead,
        context::RequestContext,
        http::{Response, StatusCode},
        params::PathParams,
        secret_store::{SecretError, SecretHandle, SecretStore},
        store_registry::{BoundSecretStore, SecretRegistry, StoreRegistry},
        time::{Deadline, MonotonicClock},
    };
    use flate2::read::GzDecoder;
    use futures::executor::block_on;
    use serde_json::{Value, json};
    use std::io::Read as _;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use time::OffsetDateTime;

    const HOST: &str = "bidder.example";
    const CONFIG: &str = "schema_version = 1\n[host.gdpr]\nenabled=false\n";
    const CREDENTIAL_CONFIG: &str = "schema_version = 1\n[host.gdpr]\nenabled=false\n[host.resource_limits]\nmax_bidders_per_auction=2\nmax_outbound_requests=2\n[host.aliases]\nfake_alias='fake_bidder'\n";

    #[derive(Default)]
    struct CapturedEndpoints(Mutex<Vec<BidderHttpRequest>>);

    #[async_trait::async_trait(?Send)]
    impl BidderFanout for CapturedEndpoints {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            let Ok(mut endpoints) = self.0.lock() else {
                return Err(FanoutInternalError::Internal(
                    "endpoint observer poisoned".into(),
                ));
            };
            let response = serde_json::to_vec(&json!({
                "id": "upstream-id", "cur": "USD", "seatbid": [{
                    "seat": "fake_bidder", "bid": [{
                        "id": "handler-bid", "impid": "i", "price": 1.25_f64,
                        "mtype": 1_u32, "w": 300_u32, "h": 250_u32,
                        "adm": "<div>creative</div>"
                    }]
                }]
            }))
            .map_err(|_error| {
                FanoutInternalError::Internal("response fixture encoding failed".into())
            })?;
            Ok(calls
                .into_iter()
                .map(|call| {
                    endpoints.push(call.request);
                    BidderCallResult {
                        result: Ok(BidderHttpResponse::new(200, vec![], response.clone())),
                        elapsed: Duration::ZERO,
                    }
                })
                .collect())
        }
    }

    fn registry(bidder: FakeBidder<'static>, gzip: bool, host: &str) -> BidderRegistry {
        let mut registry = BidderRegistry::new();
        registry
            .register_with_implementation(
                Box::leak(Box::new(BidderInfo {
                    name: "fake_bidder".into(),
                    endpoint: Some("https://{{.Host}}/auction".into()),
                    uses_https: true,
                    endpoint_compression: gzip.then_some(Compression::Gzip),
                    host_allowlist: Some(vec![host.to_owned()]),
                    ..BidderInfo::default()
                })),
                "fake_implementation",
                Box::new(bidder),
            )
            .expect("unique test bidder");
        registry
    }

    fn input() -> Value {
        json!({
            "id": "wire-handler", "cur": ["USD"], "tmax": 1000_i64,
            "site": {"id": "s", "publisher": {"id": "handler-activities"}},
            "imp": [{"id": "i", "banner": {"w": 300_i32, "h": 250_i32},
                "ext": {"tid": "imp-tid", "prebid": {"bidder": {"fake_bidder": {}}}}}],
        })
    }

    fn send(
        input: &Value,
        config: &str,
        registry: &BidderRegistry,
        fanout: Arc<CapturedEndpoints>,
    ) -> (StatusCode, Value) {
        send_with_services(input, config, registry, fanout, None)
    }

    fn send_with_services(
        input: &Value,
        config: &str,
        registry: &BidderRegistry,
        fanout: Arc<CapturedEndpoints>,
        secrets: Option<SecretRegistry>,
    ) -> (StatusCode, Value) {
        let resources = resource_profile();
        let budget = Arc::new(AuctionBudget::new(resources.derived().per_auction_peak));
        let gate = AdmissionGate::new(resources.inputs().limits.max_concurrent_auctions);
        let permit = gate.try_acquire().unwrap();
        let funding = AuctionFunding::prepay(&resources, &budget, permit).unwrap();
        assert_eq!(budget.in_use(), budget.limit());
        assert_eq!(gate.in_use(), 1);
        let response = send_with_funding(
            input, config, registry, fanout, secrets, &resources, funding,
        );
        assert_eq!(budget.in_use(), 0);
        assert_eq!(gate.in_use(), 0);
        (
            response.status(),
            serde_json::from_slice(&response.into_body().into_bytes().unwrap()).unwrap(),
        )
    }

    fn send_with_funding(
        input: &Value,
        config: &str,
        registry: &BidderRegistry,
        fanout: Arc<CapturedEndpoints>,
        secrets: Option<SecretRegistry>,
        resources: &AuctionResourceProfile,
        funding: AuctionFunding,
    ) -> Response {
        let mut request = http::Request::builder()
            .method("POST")
            .uri("/openrtb2/auction")
            .body(Body::from_bytes(serde_json::to_vec(input).unwrap()))
            .unwrap();
        request.extensions_mut().insert(TestFanoutHandle(fanout));
        if let Some(binding) = secrets {
            request.extensions_mut().insert(binding);
        }
        let context = RequestContext::new(request, PathParams::default());
        let clock = Arc::new(RequestClock::new(
            context.monotonic_clock(),
            Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH)),
        ));
        block_on(auction_for_conformance(
            context,
            toml::from_str(config).unwrap(),
            clock,
            Arc::new(SeededRng::new(42)),
            registry,
            resources,
            funding,
        ))
        .unwrap()
    }

    struct FakeSecretStore {
        snapshots: Vec<(&'static str, Option<Value>)>,
        reads: Mutex<Vec<String>>,
    }

    impl FakeSecretStore {
        fn new(snapshots: Vec<(&'static str, Option<Value>)>) -> Arc<Self> {
            Arc::new(Self {
                snapshots,
                reads: Mutex::new(Vec::new()),
            })
        }
    }

    #[async_trait::async_trait(?Send)]
    impl SecretStore for FakeSecretStore {
        async fn get_bytes(&self, _: &str, _: &str) -> Result<Option<Bytes>, SecretError> {
            panic!("credential dispatch must use bounded secret reads");
        }

        async fn get_bytes_bounded(
            &self,
            store_name: &str,
            key: &str,
            clock: &MonotonicClock,
            deadline: Deadline,
            max_backend_bytes: u64,
            max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<Bytes>, SecretError> {
            assert_eq!(store_name, "fake_physical_store");
            assert_eq!((max_backend_bytes, max_value_bytes), (4096, 4096));
            assert!(!deadline.is_expired_at(clock.now()));
            let mut reads = self.reads.lock().unwrap();
            let (expected_key, snapshot) = &self.snapshots[reads.len()];
            assert_eq!(key, *expected_key);
            reads.push(key.to_owned());
            let value = snapshot
                .as_ref()
                .map(|value| Bytes::from(serde_json::to_vec(value).unwrap()));
            let backend_bytes = value
                .as_ref()
                .map_or(0, |bytes| u64::try_from(bytes.len()).unwrap());
            assert!(backend_bytes <= max_backend_bytes && backend_bytes <= max_value_bytes);
            Ok(BoundedStoreRead {
                backend_bytes,
                value,
            })
        }
    }

    fn fake_bundle(username: &str, password: &str, tracker: &str) -> Value {
        json!({
            "schema_version": 1_u8,
            "username": username,
            "password": password,
            "tracker": tracker,
        })
    }

    fn credential_registry(
        declarations: &Arc<Mutex<Vec<String>>>,
        constructions: &Arc<Mutex<Vec<Value>>>,
    ) -> BidderRegistry {
        let declared = Arc::clone(declarations);
        let preflight = Box::leak(Box::new(
            move |request: &BidRequest, info: &BidderRequestInfo<'_>| {
                assert_eq!(request.tmax, Some(1_000));
                assert_eq!(info.account_id, "fake_account");
                assert_eq!(info.publisher_id, Some("fake_account"));
                assert!(info.credentials.is_none());
                declared
                    .lock()
                    .unwrap()
                    .push(info.bidder.as_str().to_owned());
            },
        ));
        let constructed = Arc::clone(constructions);
        let observer = Box::leak(Box::new(move |info: &BidderRequestInfo<'_>| {
            let credentials = info.credentials.unwrap();
            constructed.lock().unwrap().push(json!({
                "bidder": info.bidder.as_str(), "base": info.core_name.as_str(),
                "username": credentials.username(), "tracker": credentials.tracker(),
                "authorization": credentials.authorization(),
            }));
        }));
        let mut registry = BidderRegistry::new();
        for (name, reference) in [
            ("fake_bidder", "fake_credentials"),
            ("fake_sibling", "fake_sibling_credentials"),
            ("fake_unrequested", "fake_unrequested_credentials"),
        ] {
            registry
                .register_with_implementation(
                    Box::leak(Box::new(BidderInfo {
                        name: name.into(),
                        endpoint: Some(format!("https://bidder.example/{name}")),
                        uses_https: true,
                        ..BidderInfo::default()
                    })),
                    "fake_implementation",
                    Box::new(FakeBidder {
                        credential_reference: Some(reference),
                        credential_preflight_observer: Some(preflight),
                        construction_observer: Some(observer),
                        ..FakeBidder::default()
                    }),
                )
                .expect("unique test bidder");
        }
        registry
    }

    fn credential_input(bidders: Value) -> Value {
        let mut input = input();
        input["site"]["publisher"]["id"] = json!("fake_account");
        input["imp"][0]["ext"]["prebid"]["bidder"] = bidders;
        input
    }

    fn resource_profile() -> AuctionResourceProfile {
        let inputs = resolve_budget_inputs(&envelope::limits()).unwrap();
        let target = TargetPlatformConstants {
            platform_concurrent_ceiling: 384,
            header_entry_overhead_bytes: 64,
            parser_header_ceiling_bytes: 21_632,
            parser_header_knob_bytes: 21_633,
            max_request_target_bytes: 768,
            h1_max_buf_size_bytes: 24_576,
            h1_max_headers: 128,
            parser_max_header_entries: 676,
            pseudoheader_allowance_bytes: 1024,
            parser_transient_bytes: 0x4000,
            egress_handoff_factor: 2,
        };
        AuctionResourceProfile::new(
            AuctionResourceInputs {
                limits: inputs.limits,
                target,
                outbound_meta_bytes: auctioneer_bidder::outbound_meta_bytes(
                    inputs.limits.max_impressions_per_auction,
                ),
                construction_costs: construction_costs(),
                product_default_bytes: inputs.baked_config_bytes,
                catalog_costs: CatalogResourceCosts::UNCHANGED,
            },
            SchemaAllowance::Disabled,
        )
        .unwrap()
    }

    fn credential_binding(store: &Arc<FakeSecretStore>) -> SecretRegistry {
        let shared: Arc<dyn SecretStore> = Arc::<FakeSecretStore>::clone(store);
        StoreRegistry::single_id(
            "fake_logical_binding".into(),
            BoundSecretStore::new(SecretHandle::new(shared), "fake_physical_store".into()),
        )
    }

    fn send_with_credentials(
        input: &Value,
        config: &str,
        registry: &BidderRegistry,
        fanout: Arc<CapturedEndpoints>,
        store: &Arc<FakeSecretStore>,
    ) -> (StatusCode, Value) {
        send_with_services(
            input,
            config,
            registry,
            fanout,
            Some(credential_binding(store)),
        )
    }

    fn assert_credential_seats(wire: &Value, expected: &[&str]) {
        let mut seats: Vec<_> = wire["seatbid"]
            .as_array()
            .unwrap()
            .iter()
            .map(|seat| {
                assert_eq!(seat["bid"][0]["id"], "handler-bid");
                assert_eq!(seat["bid"][0]["impid"], "i");
                assert_eq!(seat["bid"][0]["price"], json!(1.25_f64));
                seat["seat"].as_str().unwrap()
            })
            .collect();
        seats.sort_unstable();
        assert_eq!(seats, expected);
    }

    #[test]
    fn request_secret_registry_supplies_one_coherent_snapshot_per_requested_identity() {
        for alias in [false, true] {
            let declarations = Arc::new(Mutex::new(Vec::new()));
            let constructions = Arc::new(Mutex::new(Vec::new()));
            let registry = credential_registry(&declarations, &constructions);
            let (first, second, second_reference, second_bundle, second_base) = if alias {
                (
                    "fake_alias",
                    "fake_bidder",
                    "fake_credentials",
                    fake_bundle("fake_user", "fake_password_2", "fake_tracker_2"),
                    "fake_bidder",
                )
            } else {
                (
                    "fake_bidder",
                    "fake_sibling",
                    "fake_sibling_credentials",
                    fake_bundle(
                        "fake_sibling_user",
                        "fake_sibling_password",
                        "fake_sibling_tracker",
                    ),
                    "fake_sibling",
                )
            };
            // The canonical fake declares one fixed reference per implementation;
            // alias/base reads therefore use successive rotations of the same key.
            let store = FakeSecretStore::new(vec![
                (
                    "fake_credentials",
                    Some(fake_bundle(
                        "fake_user",
                        "fake_password_1",
                        "fake_tracker_1",
                    )),
                ),
                (second_reference, Some(second_bundle)),
            ]);
            let fanout = Arc::new(CapturedEndpoints::default());
            let (status, wire) = send_with_credentials(
                &credential_input(json!({(first): {}, (second): {}})),
                CREDENTIAL_CONFIG,
                &registry,
                Arc::clone(&fanout),
                &store,
            );
            assert_eq!(status, StatusCode::OK);
            assert_eq!(
                *store.reads.lock().unwrap(),
                ["fake_credentials", second_reference]
            );
            assert_eq!(*declarations.lock().unwrap(), [first, second]);
            assert_eq!(
                *constructions.lock().unwrap(),
                [
                    json!({"bidder": first, "base": "fake_bidder", "username": "fake_user",
                    "tracker": "fake_tracker_1", "authorization": "Basic ZmFrZV91c2VyOmZha2VfcGFzc3dvcmRfMQ=="}),
                    if alias {
                        json!({"bidder": second, "base": second_base, "username": "fake_user",
                        "tracker": "fake_tracker_2", "authorization": "Basic ZmFrZV91c2VyOmZha2VfcGFzc3dvcmRfMg=="})
                    } else {
                        json!({"bidder": second, "base": second_base, "username": "fake_sibling_user",
                        "tracker": "fake_sibling_tracker", "authorization": "Basic ZmFrZV9zaWJsaW5nX3VzZXI6ZmFrZV9zaWJsaW5nX3Bhc3N3b3Jk"})
                    },
                ]
            );
            let sent = fanout.0.lock().unwrap();
            assert_eq!(sent.len(), 2);
            assert_eq!(sent[0].url(), "https://bidder.example/fake_bidder");
            assert_eq!(
                sent[1].url(),
                format!("https://bidder.example/{second_base}")
            );
            assert!(wire["ext"]["errors"].is_null(), "{wire}");
            assert_credential_seats(&wire, &[first, second]);
        }
    }

    #[test]
    fn missing_or_malformed_request_secret_refuses_only_the_affected_bidder() {
        for snapshot in [
            None,
            Some(json!({
                "schema_version": 1_u8,
                "username": "fake_user",
                "password": null,
                "tracker": "fake_tracker",
            })),
        ] {
            let declarations = Arc::new(Mutex::new(Vec::new()));
            let constructions = Arc::new(Mutex::new(Vec::new()));
            let registry = credential_registry(&declarations, &constructions);
            let store = FakeSecretStore::new(vec![
                ("fake_credentials", snapshot),
                (
                    "fake_sibling_credentials",
                    Some(fake_bundle(
                        "fake_sibling_user",
                        "fake_sibling_password",
                        "fake_sibling_tracker",
                    )),
                ),
            ]);
            let fanout = Arc::new(CapturedEndpoints::default());
            let (status, wire) = send_with_credentials(
                &credential_input(json!({"fake_bidder": {}, "fake_sibling": {}})),
                CREDENTIAL_CONFIG,
                &registry,
                Arc::clone(&fanout),
                &store,
            );
            assert_eq!(status, StatusCode::OK);
            assert_eq!(
                *store.reads.lock().unwrap(),
                ["fake_credentials", "fake_sibling_credentials"]
            );
            assert_eq!(
                *declarations.lock().unwrap(),
                ["fake_bidder", "fake_sibling"]
            );
            let constructed = constructions.lock().unwrap();
            assert_eq!(constructed.len(), 1);
            assert_eq!(constructed[0]["bidder"], "fake_sibling");
            assert_eq!(constructed[0]["tracker"], "fake_sibling_tracker");
            let sent = fanout.0.lock().unwrap();
            assert_eq!(sent.len(), 1);
            assert_eq!(sent[0].url(), "https://bidder.example/fake_sibling");
            assert_eq!(
                wire["ext"]["errors"],
                json!({"fake_bidder": [{
                    "code": 5_u32, "message": "bidder credentials unavailable"
                }]})
            );
            assert_credential_seats(&wire, &["fake_sibling"]);
        }
    }

    #[test]
    fn privacy_denied_http_identity_never_polls_request_secrets() {
        for allowed_sibling in [false, true] {
            let declarations = Arc::new(Mutex::new(Vec::new()));
            let constructions = Arc::new(Mutex::new(Vec::new()));
            let registry = credential_registry(&declarations, &constructions);
            let mut bidders = json!({"fake_alias": {}});
            let mut snapshots = Vec::new();
            if allowed_sibling {
                bidders["fake_sibling"] = json!({});
                snapshots.push((
                    "fake_sibling_credentials",
                    Some(fake_bundle(
                        "fake_sibling_user",
                        "fake_sibling_password",
                        "fake_sibling_tracker",
                    )),
                ));
            }
            let store = FakeSecretStore::new(snapshots);
            let fanout = Arc::new(CapturedEndpoints::default());
            let config = format!(
                "{CREDENTIAL_CONFIG}[accounts.fake_account.privacy.allowactivities.fetchBids]\ndefault=true\nrules=[{{allow=false,condition={{componentName=['fake_alias']}}}}]\n"
            );
            let (status, wire) = send_with_credentials(
                &credential_input(bidders),
                &config,
                &registry,
                Arc::clone(&fanout),
                &store,
            );
            assert_eq!(status, StatusCode::OK);
            let expected = if allowed_sibling {
                vec!["fake_sibling"]
            } else {
                vec![]
            };
            let expected_reads = if allowed_sibling {
                vec!["fake_sibling_credentials"]
            } else {
                vec![]
            };
            assert_eq!(*store.reads.lock().unwrap(), expected_reads);
            assert_eq!(*declarations.lock().unwrap(), expected);
            let constructed = constructions.lock().unwrap();
            assert_eq!(constructed.len(), usize::from(allowed_sibling));
            let sent = fanout.0.lock().unwrap();
            assert_eq!(sent.len(), usize::from(allowed_sibling));
            if allowed_sibling {
                assert_eq!(constructed[0]["bidder"], "fake_sibling");
                assert_eq!(constructed[0]["tracker"], "fake_sibling_tracker");
                assert_eq!(sent[0].url(), "https://bidder.example/fake_sibling");
                assert_credential_seats(&wire, &["fake_sibling"]);
            } else {
                assert!(wire["seatbid"].as_array().is_none_or(Vec::is_empty));
            }
            assert!(wire["ext"]["errors"].is_null(), "{wire}");
            assert_eq!(wire["ext"]["warnings"]["fake_alias"][0]["code"], 10017_u32);
        }
    }

    #[test]
    fn insufficient_original_funding_refuses_before_credential_preparation() {
        let declarations = Arc::new(Mutex::new(Vec::new()));
        let constructions = Arc::new(Mutex::new(Vec::new()));
        let registry = credential_registry(&declarations, &constructions);
        let store = FakeSecretStore::new(vec![]);
        let fanout = Arc::new(CapturedEndpoints::default());
        let original = resource_profile();
        let budget = Arc::new(AuctionBudget::new(original.derived().per_auction_peak));
        let gate = AdmissionGate::new(original.inputs().limits.max_concurrent_auctions);
        let permit = gate.try_acquire().unwrap();
        let funding = AuctionFunding::prepay(&original, &budget, permit).unwrap();
        assert_eq!(budget.in_use(), budget.limit());
        assert_eq!(gate.in_use(), 1);
        // Preserve every workload limit while requiring one more projection byte
        // than the original, already-prepaid receipt can supply.
        let mut facts = *original.inputs();
        facts.catalog_costs.context_per_auction_bytes += 1;
        let required = AuctionResourceProfile::new(facts, SchemaAllowance::Disabled).unwrap();
        assert_eq!(
            required.derived().auction_charges.projection,
            original.derived().auction_charges.projection + 1,
        );
        let response = send_with_funding(
            &credential_input(json!({"fake_bidder": {}})),
            CREDENTIAL_CONFIG,
            &registry,
            Arc::clone(&fanout),
            Some(credential_binding(&store)),
            &required,
            funding,
        );
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response.headers()["content-type"],
            "text/plain; charset=utf-8"
        );
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(
            response.into_body().into_bytes().unwrap().as_ref(),
            b"overloaded\n"
        );
        assert!(store.reads.lock().unwrap().is_empty());
        assert!(declarations.lock().unwrap().is_empty());
        assert!(constructions.lock().unwrap().is_empty());
        assert!(fanout.0.lock().unwrap().is_empty());
        assert_eq!(budget.in_use(), 0);
        assert_eq!(gate.in_use(), 0);
    }

    fn assert_normalized_bid(wire: &Value) {
        assert_eq!(wire["id"], "wire-handler");
        assert_eq!(wire["cur"], "USD");
        assert!(wire["ext"]["errors"].is_null(), "{wire}");
        assert_eq!(wire["seatbid"].as_array().unwrap().len(), 1);
        assert_eq!(wire["seatbid"][0]["seat"], "fake_bidder");
        assert_eq!(wire["seatbid"][0]["bid"].as_array().unwrap().len(), 1);
        let bid = &wire["seatbid"][0]["bid"][0];
        assert_eq!(bid["id"], "handler-bid");
        assert_eq!(bid["impid"], "i");
        assert_eq!(bid["price"], json!(1.25_f64));
        assert_eq!(bid["adm"], "<div>creative</div>");
        assert_eq!(bid["ext"]["prebid"]["type"], "banner");
    }

    #[test]
    fn raw_extension_presence_reaches_construction_before_normalization() {
        for host in [HOST, "alternate.bidder.example"] {
            for (extension, shape) in [
                (None, JsonFieldShape::Absent),
                (Some(Value::Null), JsonFieldShape::Null),
                (Some(json!({"keep": true})), JsonFieldShape::Object),
            ] {
                let shapes = Arc::new(Mutex::new(Vec::new()));
                let captured = Arc::clone(&shapes);
                let callback = Box::leak(Box::new(move |info: &BidderRequestInfo<'_>| {
                    captured
                        .lock()
                        .unwrap()
                        .push(info.original_request_ext_shape);
                }));
                let registry = registry(
                    FakeBidder {
                        construction_observer: Some(callback),
                        selected_host: Some(host),
                        ..FakeBidder::default()
                    },
                    false,
                    host,
                );
                let fanout = Arc::new(CapturedEndpoints::default());
                let mut input = input();
                if let Some(value) = extension {
                    input["ext"] = value;
                }
                let (status, wire) = send(&input, CONFIG, &registry, Arc::clone(&fanout));
                assert_eq!(status, StatusCode::OK);
                assert_eq!(*shapes.lock().unwrap(), [shape]);
                assert_normalized_bid(&wire);
                let sent = fanout.0.lock().unwrap();
                assert_eq!(sent.len(), 1);
                assert_eq!(sent[0].url(), format!("https://{host}/auction"));
                assert_eq!(sent[0].covered_imp_ids().collect::<Vec<_>>(), ["i"]);
                let outbound: Value = serde_json::from_slice(sent[0].body()).unwrap();
                assert_eq!(outbound["ext"]["keep"], input["ext"]["keep"]);
            }
        }
    }

    #[test]
    fn operator_floor_rates_survive_config_consumption_into_actual_handler_dispatch() {
        for (amount, rate, expected) in [
            (1.0_f64, 2.5_f64, 2.5_f64),
            (4.0_f64, 2.5_f64, 10.0_f64),
            (1.0_f64, 0.25_f64, 0.25_f64),
            (4.0_f64, 0.25_f64, 1.0_f64),
        ] {
            let observations = Arc::new(Mutex::new(Vec::new()));
            let observed = Arc::clone(&observations);
            let callback = Box::leak(Box::new(move |info: &BidderRequestInfo<'_>| {
                assert_eq!(info.currency, ["USD"]);
                observed
                    .lock()
                    .unwrap()
                    .push(info.floor_currency.convert_floor_to_usd(amount, "QAA"));
            }));
            let registry = registry(
                FakeBidder {
                    selected_host: Some(HOST),
                    construction_observer: Some(callback),
                    ..FakeBidder::default()
                },
                false,
                HOST,
            );
            let config = format!(
                "{CONFIG}[floor_currency]\nexpires_at=2000\n[floor_currency.usd_per_unit]\nQAA={rate}\n"
            );
            let fanout = Arc::new(CapturedEndpoints::default());
            let (status, wire) = send(&input(), &config, &registry, Arc::clone(&fanout));
            assert_eq!(status, StatusCode::OK);
            assert_normalized_bid(&wire);
            assert_eq!(*observations.lock().unwrap(), [Ok(expected)]);
            assert_eq!(fanout.0.lock().unwrap().len(), 1);
        }
    }

    #[test]
    fn unsafe_endpoint_hosts_are_refused_before_dispatch() {
        for (selected, allowed) in [
            ("unlisted.bidder.example", HOST),
            // Selection succeeds, but the final validator must still reject private IPs.
            ("10.0.0.1", "10.0.0.1"),
        ] {
            let registry = registry(
                FakeBidder {
                    selected_host: Some(selected),
                    ..FakeBidder::default()
                },
                false,
                allowed,
            );
            let fanout = Arc::new(CapturedEndpoints::default());
            let (status, wire) = send(&input(), CONFIG, &registry, Arc::clone(&fanout));
            assert_eq!(status, StatusCode::OK);
            assert!(fanout.0.lock().unwrap().is_empty());
            assert!(wire["seatbid"].as_array().is_none_or(Vec::is_empty));
            assert_eq!(wire["ext"]["errors"]["fake_bidder"][0]["code"], 5_u32);
        }
    }

    #[test]
    fn finalized_calls_compose_gzip_with_independent_activity_axes_and_normalized_bids() {
        for ufpd in [false, true] {
            for geo in [false, true] {
                for tid in [false, true] {
                    for gzip in [false, true] {
                        let registry = registry(
                            FakeBidder {
                                selected_host: Some(HOST),
                                ..FakeBidder::default()
                            },
                            gzip,
                            HOST,
                        );
                        let fanout = Arc::new(CapturedEndpoints::default());
                        let mut input = input();
                        input["source"] = json!({"tid": "source-tid"});
                        input["user"] = json!({"id": "user-id", "buyeruid": "buyer-id",
                            "keywords": "interest", "gender": "F", "yob": 1980_i32,
                            "geo": {"country": "USA"}});
                        input["device"] = json!({"ip": "8.8.8.37", "ifa": "device-id",
                            "geo": {"lat": 37.123_f64, "lon": -122.123_f64,
                                "country": "USA", "zip": "94107"}});
                        input["ext"] = json!({"keep": true});
                        let config = format!(
                            "{CONFIG}[accounts.handler-activities.privacy.ipv4]\nanon_keep_bits=24\n[accounts.handler-activities.privacy.allowactivities.transmitUfpd]\ndefault={ufpd}\n[accounts.handler-activities.privacy.allowactivities.transmitPreciseGeo]\ndefault={geo}\n[accounts.handler-activities.privacy.allowactivities.transmitTid]\ndefault={tid}\n"
                        );
                        let (status, wire) = send(&input, &config, &registry, Arc::clone(&fanout));
                        assert_eq!(status, StatusCode::OK);
                        assert_normalized_bid(&wire);
                        let sent = fanout.0.lock().unwrap();
                        assert_eq!(sent.len(), 1);
                        assert_outbound(&sent[0], gzip, ufpd, geo, tid);
                    }
                }
            }
        }
    }

    fn assert_outbound(call: &BidderHttpRequest, gzip: bool, ufpd: bool, geo: bool, tid: bool) {
        assert_eq!(call.url(), format!("https://{HOST}/auction"));
        assert_eq!(call.covered_imp_ids().collect::<Vec<_>>(), ["i"]);
        let encoding: Vec<_> = call
            .headers()
            .filter(|(key, _value)| key.eq_ignore_ascii_case("content-encoding"))
            .map(|(_key, value)| value)
            .collect();
        assert_eq!(encoding, if gzip { vec!["gzip"] } else { vec![] });
        let bytes = if gzip {
            assert_eq!(call.body().get(..2), Some([0x1f, 0x8b].as_slice()));
            let mut decoded = Vec::new();
            GzDecoder::new(call.body())
                .read_to_end(&mut decoded)
                .unwrap();
            decoded
        } else {
            call.body().to_vec()
        };
        let outbound: Value = serde_json::from_slice(&bytes).unwrap();
        for field in ["id", "buyeruid", "keywords", "gender", "yob"] {
            assert_eq!(
                outbound["user"].get(field).is_some(),
                ufpd,
                "{field} ufpd={ufpd} geo={geo} tid={tid} gzip={gzip}"
            );
        }
        assert_eq!(outbound["device"].get("ifa").is_some(), ufpd);
        assert_eq!(
            outbound["device"]["ip"],
            if geo { "8.8.8.37" } else { "8.8.8.0" }
        );
        assert_eq!(outbound["device"]["geo"]["country"], "USA");
        assert_eq!(outbound["device"]["geo"].get("zip").is_some(), geo);
        assert_eq!(
            outbound["device"]["geo"]["lat"],
            json!(if geo { 37.123_f64 } else { 37.12_f64 })
        );
        assert_eq!(
            outbound["device"]["geo"]["lon"],
            json!(if geo { -122.123_f64 } else { -122.12_f64 })
        );
        assert_eq!(outbound["source"].get("tid").is_some(), tid);
        assert_eq!(outbound["imp"][0]["ext"].get("tid").is_some(), tid);
        assert_eq!(outbound["ext"]["keep"], true);
    }
}
