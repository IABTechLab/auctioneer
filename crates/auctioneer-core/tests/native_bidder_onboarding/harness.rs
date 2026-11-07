use super::fixtures::{self, Flow};
use auctioneer_bidder::{
    BidderHttpRequest, BidderHttpResponse, ValidatedRegistryPlan, outbound_meta_bytes,
    schema::controlled_native_costs,
};
use auctioneer_config_data::types::{Compression as CfgCompression, MediaType};
use auctioneer_core::{
    App as CoreApp,
    auction::audit::construction_costs,
    config::{
        AuctioneerAppConfig,
        validation::{PolicyValidationError, validate_effective_operator_policy},
    },
    handlers::TestFanoutHandle,
    host_primitives::{
        BootEnvelope, DelegateToDevStrategy, DevRng, StderrBootLog, SystemClock, TestClockHandle,
        TestMonotonicClockHandle, TestRngHandle, boot_gates, bootstrap_adapter_with_envelope,
        construction_diagnostic_costs,
    },
    test_support::config_store::runtime_registry,
};
use auctioneer_exchange::{
    AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
};
use auctioneer_runtime::{
    Clock as _, FakeClock, HeadroomClass, ImmediateResolveTimer, NoopDecodeCooperator,
    ResolvedIsolateLimits, SeededRng, TargetPlatformConstants,
    bidder_credentials::MAX_BIDDER_CREDENTIAL_BYTES, minimum_headroom_bytes,
};
use bytes::Bytes;
use edgezero_core::{
    PlatformMetadata,
    app::App,
    body::Body,
    config_store::BoundedStoreRead,
    http::{Method, StatusCode, request_builder},
    ingress::{IngressDispatchOutcome, IngressFraming, IngressHeadAccounting},
    secret_store::{SecretError, SecretHandle, SecretStore},
    store_registry::{BoundSecretStore, SecretRegistry, StoreRegistry},
    time::{Deadline, MonotonicClock, MonotonicInstant},
};
use flate2::read::GzDecoder;
use futures::executor::block_on;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read as _,
    mem,
    num::NonZeroU64,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use time::OffsetDateTime;
use url::Url;

pub(super) fn boot() -> App {
    assert!(
        boot_gates().is_none(),
        "this target owns its process-global bootstrap"
    );
    let mut limits = fixtures::limits();
    // Portable acceptance ceiling declared before execution. Provider-profile
    // physical fit is a separate parent-owned test, not inferred from this row.
    // The unchanged 4 GiB Axum row is tested by the adapter envelope suite;
    // importing that adapter here would create an unnecessary dependency cycle.
    limits.isolate_total_bytes = 1_u64 << 40_u32;
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
    let costs = controlled_native_costs(construction_diagnostic_costs(
        limits.max_bidders_per_auction,
    ));
    assert_eq!(
        (
            costs.resident_bytes,
            costs.initialization_bytes,
            costs.schema_validation_per_auction_bytes
        ),
        (16_u64 << 20_u32, 8_u64 << 20_u32, 512_u64 << 10_u32)
    );
    assert!(costs.context_per_auction_bytes > 0);
    bootstrap_adapter_with_envelope(
        BootEnvelope {
            limits: &limits,
            target: &target,
            outbound_meta_bytes: outbound_meta_bytes(limits.max_impressions_per_auction),
            construction_costs: construction_costs(),
            catalog_costs: costs,
            log_sink: &StderrBootLog,
        },
        || Arc::new(SystemClock),
        || Arc::new(DevRng::new()),
        || Arc::new(ImmediateResolveTimer),
        || Arc::new(NoopDecodeCooperator),
        || Arc::new(DelegateToDevStrategy),
    )
    .expect("portable native reservation installs");
    let gates = boot_gates().unwrap();
    assert_eq!(gates.catalog_costs, costs);
    assert_eq!(gates.native_schema_scratch_bytes(), 512_u64 << 10_u32);
    assert_eq!(gates.rederive_budget().unwrap(), gates.derived);
    assert_eq!(
        gates.derived.isolate_terms.catalog_resident_bytes,
        16_u64 << 20_u32
    );
    assert_eq!(
        gates.derived.isolate_terms.catalog_initialization_bytes,
        8_u64 << 20_u32
    );
    let headroom = minimum_headroom_bytes(
        NonZeroU64::new(limits.isolate_total_bytes).unwrap(),
        HeadroomClass::Native,
    );
    assert!(gates.derived.total_bytes.checked_add(headroom).unwrap() <= limits.isolate_total_bytes);
    App::build::<CoreApp>(PlatformMetadata::default())
        .expect("generated App builds after funded bootstrap")
}

struct PlannedCall {
    owner: String,
    ordinal: usize,
    coverage: Vec<String>,
    method: String,
    url: String,
    body: Value,
    required_headers: Vec<(String, String)>,
    absent_headers: Vec<String>,
    gzip: bool,
    status: u16,
    response: Vec<u8>,
    offered: BTreeMap<String, BTreeSet<String>>,
}

pub(super) struct ObservedCall {
    pub owner: String,
    pub ordinal: usize,
    pub coverage: Vec<String>,
    pub gzip: bool,
    pub correlation: Vec<Option<(MediaType, u32)>>,
    pub body: Value,
    pub url: String,
    pub headers: Vec<(String, String)>,
}

#[derive(Clone, Copy)]
enum OracleMode {
    Exact,
    Privacy,
}

struct Transport {
    planned: Vec<PlannedCall>,
    calls: Mutex<Vec<ObservedCall>>,
    received: AtomicUsize,
    batches: AtomicUsize,
    limits: ResolvedIsolateLimits,
    start: MonotonicInstant,
    mode: OracleMode,
}

impl Transport {
    fn new(flows: &[&Flow], input: &Value, start: MonotonicInstant, mode: OracleMode) -> Self {
        let planned = flows
            .iter()
            .flat_map(|flow| {
                flow.case
                    .outbound
                    .iter()
                    .enumerate()
                    .map(|(ordinal, outbound)| {
                        let offered = flow.case.request["imp"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|imp| {
                                let id = flow.ids[imp["id"].as_str().unwrap()].clone();
                                let media = ["banner", "video", "audio", "native"]
                                    .into_iter()
                                    .filter(|media| {
                                        imp.get(*media).is_some_and(|value| !value.is_null())
                                    })
                                    .map(str::to_owned)
                                    .collect();
                                (id, media)
                            })
                            .collect();
                        PlannedCall {
                            owner: flow.name().to_owned(),
                            ordinal,
                            coverage: flow.covered(ordinal),
                            method: outbound.method.clone(),
                            url: fixtures::expected_url(flow, ordinal),
                            body: fixtures::expected_body(flow, ordinal, input),
                            required_headers: outbound.required_headers.clone(),
                            absent_headers: outbound.absent_headers.clone(),
                            gzip: flow.info.endpoint_compression == Some(CfgCompression::Gzip),
                            status: flow.responses[ordinal].status,
                            response: serde_json::to_vec(&flow.responses[ordinal].body).unwrap(),
                            offered,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        Self {
            planned,
            calls: Mutex::new(Vec::new()),
            received: AtomicUsize::new(0),
            batches: AtomicUsize::new(0),
            limits: boot_gates().unwrap().limits,
            start,
            mode,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl BidderFanout for Transport {
    async fn send_all(
        &self,
        calls: Vec<BidderCall>,
        deadline: AuctionDeadline,
    ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
        self.batches.fetch_add(1, Ordering::SeqCst);
        assert!(calls.len() <= usize::try_from(self.limits.max_outbound_requests).unwrap());
        assert!(deadline.deadline_at > self.start);
        assert!(
            deadline.deadline_at <= self.start.checked_add(Duration::from_millis(1000)).unwrap()
        );
        let mut results = Vec::new();
        for call in calls {
            let coverage: Vec<_> = call.request.covered_imp_ids().map(str::to_owned).collect();
            let matches: Vec<_> = self
                .planned
                .iter()
                .filter(|planned| planned.coverage == coverage)
                .collect();
            assert_eq!(
                matches.len(),
                1,
                "each received call has independent exact owner coverage"
            );
            let planned = matches[0];
            assert_eq!(call.request.method(), planned.method);
            let url = Url::parse(call.request.url()).unwrap();
            assert_eq!(url.scheme(), "https");
            assert!(url.host_str().is_some());
            assert_eq!(
                url.as_str(),
                planned.url,
                "independent owner URL oracle: {}",
                planned.owner
            );
            assert!(call.timeout > Duration::ZERO && call.timeout <= Duration::from_millis(1000));
            assert!(
                call.max_transport_bytes > 0
                    && call.max_transport_bytes <= self.limits.max_transport_bytes
            );
            assert!(
                call.max_decoded_bytes > 0
                    && call.max_decoded_bytes <= self.limits.max_decoded_bytes
            );
            let headers: Vec<_> = call.request.headers().collect();
            assert_call_headers(planned, &headers);
            let body = decode_call_body(&call.request, planned.gzip, &self.limits);
            assert_body_oracle(&body, &planned.body, self.mode, &planned.owner);
            let body_ids: Vec<_> = body["imp"]
                .as_array()
                .expect("actual outbound impressions")
                .iter()
                .map(|imp| imp["id"].as_str().unwrap())
                .collect();
            assert_eq!(
                body_ids,
                coverage.iter().map(String::as_str).collect::<Vec<_>>()
            );
            let correlation = call_correlation(&call.request, &coverage, &planned.offered);
            self.calls.lock().unwrap().push(ObservedCall {
                owner: planned.owner.clone(),
                ordinal: planned.ordinal,
                coverage,
                gzip: planned.gzip,
                correlation,
                body,
                url: url.to_string(),
                headers: headers
                    .into_iter()
                    .map(|(name, value)| (name.to_owned(), value.to_owned()))
                    .collect(),
            });
            let response = BidderHttpResponse::new(
                planned.status,
                vec![("content-type".into(), "application/json".into())],
                planned.response.clone(),
            );
            assert_eq!(
                response.max_parsed_nodes(),
                0,
                "only production exchange attaches the response budget"
            );
            self.received.fetch_add(1, Ordering::SeqCst);
            results.push(BidderCallResult {
                result: Ok(response),
                elapsed: Duration::from_millis(1),
            });
        }
        Ok(results)
    }
}

fn assert_call_headers(planned: &PlannedCall, headers: &[(&str, &str)]) {
    for (name, expected) in &planned.required_headers {
        let values: Vec<_> = headers
            .iter()
            .filter(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| *value)
            .collect();
        assert_eq!(
            values,
            [expected.as_str()],
            "required owner header for {}",
            planned.owner
        );
    }
    for name in &planned.absent_headers {
        assert!(
            !headers
                .iter()
                .any(|(key, _)| key.eq_ignore_ascii_case(name))
        );
    }
    let encodings: Vec<_> = headers
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case("content-encoding"))
        .map(|(_, value)| *value)
        .collect();
    assert_eq!(encodings, if planned.gzip { vec!["gzip"] } else { vec![] });
}

fn decode_call_body(
    request: &BidderHttpRequest,
    gzip: bool,
    limits: &ResolvedIsolateLimits,
) -> Value {
    let bytes = if gzip {
        assert_eq!(request.body().get(..2), Some([0x1f, 0x8b].as_slice()));
        let mut decoded = Vec::new();
        GzDecoder::new(request.body())
            .take(limits.max_outbound_raw_bytes.checked_add(1).unwrap())
            .read_to_end(&mut decoded)
            .unwrap();
        decoded
    } else {
        request.body().to_vec()
    };
    assert!(u64::try_from(bytes.len()).unwrap() <= limits.max_outbound_raw_bytes);
    assert!(u64::try_from(request.body().len()).unwrap() <= limits.max_outbound_request_bytes);
    serde_json::from_slice(&bytes)
        .expect("actual finalized request is JSON after transport decoding")
}

fn call_correlation(
    request: &BidderHttpRequest,
    coverage: &[String],
    offered: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<Option<(MediaType, u32)>> {
    let correlation = coverage
        .iter()
        .enumerate()
        .map(|(ordinal, id)| {
            request
                .meta()
                .imp_correlation(u16::try_from(ordinal).unwrap())
                .map(|row| {
                    let media: MediaType = row.media().into();
                    let name = match media {
                        MediaType::Banner => "banner",
                        MediaType::Video => "video",
                        MediaType::Audio => "audio",
                        MediaType::Native => "native",
                    };
                    assert!(
                        offered[id].contains(name),
                        "correlation is call-local and offered"
                    );
                    (media, row.token())
                })
        })
        .collect();
    assert!(
        request
            .meta()
            .imp_correlation(u16::try_from(coverage.len()).unwrap())
            .is_none()
    );
    correlation
}

fn assert_body_oracle(actual_input: &Value, expected_input: &Value, mode: OracleMode, owner: &str) {
    let mut actual = actual_input.clone();
    let mut expected = expected_input.clone();
    if matches!(mode, OracleMode::Privacy) {
        // Only the dedicated privacy rows admit these post-policy/protocol
        // surfaces. Their wire assertions run in privacy.rs, not this oracle.
        for value in [&mut actual, &mut expected] {
            for key in ["user", "device", "regs", "site", "app"] {
                value.as_object_mut().unwrap().remove(key);
            }
            if let Some(source) = value.get_mut("source").and_then(Value::as_object_mut) {
                source.remove("tid");
            }
            for imp in value["imp"].as_array_mut().unwrap() {
                if let Some(ext) = imp.get_mut("ext").and_then(Value::as_object_mut) {
                    ext.remove("tid");
                    ext.remove("data");
                    if ext.is_empty() {
                        imp.as_object_mut().unwrap().remove("ext");
                    }
                }
            }
        }
    }
    assert_eq!(actual, expected, "independent owner body oracle: {owner}");
}

pub(super) struct Secrets {
    pub bundles: BTreeMap<String, Bytes>,
    pub reads: Mutex<Vec<String>>,
}

impl Secrets {
    pub fn new(flows: &[&Flow], missing: bool) -> Arc<Self> {
        let bundles = flows
            .iter()
            .filter_map(|flow| flow.credential())
            .filter(|_| !missing)
            .map(|(reference, bundle)| {
                (
                    reference.to_owned(),
                    Bytes::from(serde_json::to_vec(bundle).unwrap()),
                )
            })
            .collect();
        Arc::new(Self {
            bundles,
            reads: Mutex::new(Vec::new()),
        })
    }
}

#[async_trait::async_trait(?Send)]
impl SecretStore for Secrets {
    async fn get_bytes(&self, _: &str, _: &str) -> Result<Option<Bytes>, SecretError> {
        panic!("real credential dispatch must use the bounded secret operation")
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
        assert_eq!(store_name, "onboarding_fixture_store");
        let cap = u64::try_from(MAX_BIDDER_CREDENTIAL_BYTES).unwrap();
        assert_eq!((max_backend_bytes, max_value_bytes), (cap, cap));
        assert!(!deadline.is_expired_at(clock.now()));
        self.reads.lock().unwrap().push(key.to_owned());
        let value = self.bundles.get(key).cloned();
        let backend_bytes = value
            .as_ref()
            .map_or(0, |bytes| u64::try_from(bytes.len()).unwrap());
        assert!(backend_bytes <= cap);
        Ok(BoundedStoreRead {
            backend_bytes,
            value,
        })
    }
}

pub(super) struct Answer {
    pub status: StatusCode,
    pub wire: Value,
    pub calls: Vec<ObservedCall>,
    pub received: usize,
}

pub(super) fn send(
    app: &App,
    flows: &[&Flow],
    input: &Value,
    config: &Value,
    secrets: &Arc<Secrets>,
) -> Answer {
    send_mode(
        app,
        flows,
        input,
        config,
        secrets,
        OracleMode::Exact,
        Ok(()),
    )
}

pub(super) fn send_rejected_policy(
    app: &App,
    flows: &[&Flow],
    input: &Value,
    config: &Value,
    secrets: &Arc<Secrets>,
    expected: PolicyValidationError,
) -> Answer {
    send_mode(
        app,
        flows,
        input,
        config,
        secrets,
        OracleMode::Exact,
        Err(expected),
    )
}

pub(super) fn send_privacy(
    app: &App,
    flows: &[&Flow],
    input: &Value,
    config: &Value,
    secrets: &Arc<Secrets>,
) -> Answer {
    send_mode(
        app,
        flows,
        input,
        config,
        secrets,
        OracleMode::Privacy,
        Ok(()),
    )
}

fn assert_policy_admission(
    flows: &[&Flow],
    config: &Value,
    expected: Result<(), PolicyValidationError>,
) {
    let case_ids: Vec<_> = flows.iter().map(|flow| flow.case.name.as_str()).collect();
    let policy: AuctioneerAppConfig =
        serde_json::from_value(config.clone()).unwrap_or_else(|_error| {
            panic!("native onboarding policy shape precondition for cases {case_ids:?}: Structure");
        });
    let catalog = ValidatedRegistryPlan::compiled().unwrap_or_else(|_error| {
        panic!("native onboarding catalog precondition for cases {case_ids:?}");
    });
    assert_eq!(
        validate_effective_operator_policy(
            &policy,
            auctioneer_config_data::product_host_default(),
            auctioneer_config_data::product_account_default(),
            &catalog,
        ),
        expected,
        "native onboarding policy precondition for cases {case_ids:?}",
    );
}

fn send_mode(
    app: &App,
    flows: &[&Flow],
    input: &Value,
    config: &Value,
    secrets: &Arc<Secrets>,
    mode: OracleMode,
    expected: Result<(), PolicyValidationError>,
) -> Answer {
    assert_policy_admission(flows, config, expected);
    let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
    let fanout = Arc::new(Transport::new(flows, input, clock.now_monotonic(), mode));
    let builder = request_builder()
        .method(Method::POST)
        .uri("/openrtb2/auction")
        .header("content-type", "application/json");
    let ingress_builder = if matches!(mode, OracleMode::Privacy) {
        builder.header("x-forwarded-for", "8.8.4.123")
    } else {
        builder
    };
    let mut request = ingress_builder
        .body(Body::from_bytes(serde_json::to_vec(input).unwrap()))
        .unwrap();
    request
        .extensions_mut()
        .insert(TestClockHandle(Arc::<FakeClock>::clone(&clock)));
    let monotonic = Arc::clone(&clock);
    request
        .extensions_mut()
        .insert(TestMonotonicClockHandle(MonotonicClock::new(move || {
            monotonic.now_monotonic()
        })));
    request
        .extensions_mut()
        .insert(TestRngHandle(Arc::new(SeededRng::new(42))));
    request
        .extensions_mut()
        .insert(TestFanoutHandle(Arc::<Transport>::clone(&fanout)));
    request.extensions_mut().insert(runtime_registry(config));
    let store: Arc<dyn SecretStore> = Arc::<Secrets>::clone(secrets);
    let registry: SecretRegistry = StoreRegistry::single_id(
        "onboarding_credentials".into(),
        BoundSecretStore::new(SecretHandle::new(store), "onboarding_fixture_store".into()),
    );
    request.extensions_mut().insert(registry);
    let outcome = block_on(app.dispatch_ingress(
        request,
        MonotonicInstant::now(),
        IngressHeadAccounting::HostManaged,
        IngressFraming::HostManaged,
    ))
    .unwrap();
    let IngressDispatchOutcome::Response(mut envelope) = outcome else {
        panic!("bounded generated-App onboarding request must produce an egress envelope");
    };
    let status = envelope.response_mut().status();
    let bytes = envelope
        .response_mut()
        .body_mut()
        .as_bytes()
        .expect("buffered auction response")
        .to_vec();
    assert!(
        u64::try_from(bytes.len()).unwrap() <= boot_gates().unwrap().limits.final_response_bytes
    );
    let wire = if status == StatusCode::OK {
        serde_json::from_slice(&bytes).expect("actual generated-App auction response JSON")
    } else {
        Value::String(String::from_utf8(bytes).expect("bounded production error text"))
    };
    drop(envelope);
    let calls = mem::take(&mut *fanout.calls.lock().unwrap());
    let received = fanout.received.load(Ordering::SeqCst);
    assert_eq!(calls.len(), received, "independent transport receive count");
    assert_eq!(fanout.batches.load(Ordering::SeqCst) > 0, !calls.is_empty());
    Answer {
        status,
        wire,
        calls,
        received,
    }
}

pub(super) fn assert_success(answer: &Answer, allowed: &[&Flow]) {
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.wire);
    let expected_calls: BTreeSet<_> = allowed
        .iter()
        .flat_map(|flow| {
            (0..flow.case.outbound.len()).map(move |call| (flow.name().to_owned(), call))
        })
        .collect();
    let actual_calls: BTreeSet<_> = answer
        .calls
        .iter()
        .map(|call| (call.owner.clone(), call.ordinal))
        .collect();
    assert_eq!(
        actual_calls, expected_calls,
        "actual dispatch must retain precisely the allowed identities: {}",
        answer.wire
    );
    assert_eq!(answer.received, expected_calls.len());
    let expected: BTreeSet<_> = allowed
        .iter()
        .flat_map(|flow| {
            flow.responses.iter().flat_map(move |response| {
                response.expected.bids.iter().map(move |bid| {
                    (
                        flow.name().to_owned(),
                        bid.id.clone(),
                        bid.impid.clone(),
                        bid.media_type.clone(),
                    )
                })
            })
        })
        .collect();
    let actual: Vec<_> = answer.wire["seatbid"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|seat| {
            seat["bid"].as_array().unwrap().iter().map(move |bid| {
                let owner = seat["seat"].as_str().unwrap();
                let imp_id = bid["impid"].as_str().unwrap();
                assert!(
                    bid["price"]
                        .as_f64()
                        .is_some_and(|price| price.is_finite() && price > 0.0_f64),
                    "acceptance requires actual positive-CPM bids"
                );
                assert!(
                    answer.calls.iter().any(|call| call.owner == owner
                        && call.coverage.iter().any(|covered| covered == imp_id)),
                    "accepted bid must reference an independently observed sent call"
                );
                (
                    owner.to_owned(),
                    bid["id"].as_str().unwrap().to_owned(),
                    imp_id.to_owned(),
                    bid.pointer("/ext/prebid/type")
                        .and_then(Value::as_str)
                        .expect("production assembly publishes the classified media type")
                        .to_owned(),
                )
            })
        })
        .collect();
    assert_eq!(
        actual.len(),
        expected.len(),
        "no missing or duplicate normalized bids: {}",
        answer.wire
    );
    assert_eq!(
        actual.into_iter().collect::<BTreeSet<_>>(),
        expected,
        "{}",
        answer.wire
    );
    for flow in allowed {
        assert!(
            answer.wire["ext"]["errors"].get(flow.name()).is_none(),
            "allowed sibling errors: {}",
            answer.wire
        );
    }
    assert_eq!(
        actual_calls
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<BTreeSet<_>>(),
        fixtures::selected_names(allowed)
    );
}

pub(super) fn assert_disabled(answer: &Answer, flow: &Flow) {
    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    let source_id = flow.case.request["imp"][0]["id"].as_str().unwrap();
    assert_eq!(
        answer.wire,
        Value::String(format!(
            "Invalid request: imp {} has no valid bidders\n",
            flow.ids[source_id],
        ))
    );
    assert!(answer.calls.is_empty());
    assert_eq!(answer.received, 0);
}

#[test]
#[should_panic(expected = "independent owner body oracle")]
fn exact_oracle_rejects_a_changed_protocol_parameter() {
    let expected = serde_json::json!({"imp":[{"id":"i","ext":{"bidder":{"slot":"expected"}}}]});
    let mut changed = expected.clone();
    changed["imp"][0]["ext"]["bidder"]["slot"] = serde_json::json!("wrong");
    assert_body_oracle(&changed, &expected, OracleMode::Exact, "fake_owner");
}

#[test]
#[should_panic(expected = "independent owner body oracle")]
fn privacy_oracle_still_rejects_a_changed_protocol_parameter() {
    let expected = serde_json::json!({"imp":[{"id":"i","ext":{"bidder":{"slot":"expected"}}}]});
    let mut changed = expected.clone();
    changed["imp"][0]["ext"]["bidder"]["slot"] = serde_json::json!("wrong");
    changed["user"] = serde_json::json!({"id":"allowed-only-in-privacy-mode"});
    assert_body_oracle(&changed, &expected, OracleMode::Privacy, "fake_owner");
}
