//! Shared auction recording harness for privacy and configuration regressions.

pub use crate::support::response_budgets;

use crate::fake_bidder::{FakeBidder, RequestMode};

use async_trait::async_trait;
use auctioneer_bidder::{
    Bidder, BidderError, BidderHttpRequest, BidderHttpResponse, BidderName, BidderRegistry,
    BidderRequestInfo, BidderResponse, DebugBodySchema, OutboundRequestMeta, OutboundRequestSink,
};
use auctioneer_config_data::types::{
    AccountConfig, BidderInfo, HostConfig, PurposeEnforcement, ResourceLimits, TriState,
};
use auctioneer_exchange::scrub::DisclosureAudit;
use auctioneer_exchange::{
    AccountSource, AuctionContext, AuctionDeadline, AuctionError, AuctionPlan, BidderCall,
    BidderCallResult, BidderFanout, ExchangeFeatures, FanoutInternalError, ResolvedAccount,
    resolve_aliases, run_auction,
};
use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::BidResponse};
use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare};
use auctioneer_privacy::gcl::{CmpRecord, Gcl};
use auctioneer_privacy::gvl::{Gvl, GvlStore};
use auctioneer_privacy::tcf::parse_tcf_v2;
use auctioneer_runtime::{Clock as _, FakeClock, Rng as _, SeededRng};
use futures::executor::block_on;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use time::macros::datetime;

pub const ACCOUNT_ID: &str = "fake_account";
pub const CLIENT_V4: &str = "8.8.4.4";
pub const CLIENT_V6: &str = "2001:4860:4860::8844";
pub const CONSENT: &str =
    include_str!("../../../auctioneer-privacy/tests/data/tcf_v2/valid_full_consent.txt");

pub const IDENTIFIER_PATHS: &[&str] = &[
    "/user/id",
    "/user/buyeruid",
    "/user/yob",
    "/user/gender",
    "/user/eids",
    "/user/ext/eids",
    "/device/ifa",
    "/device/dpidsha1",
    "/device/dpidmd5",
    "/device/didsha1",
    "/device/didmd5",
    "/device/macsha1",
    "/device/macmd5",
];
pub const UNKNOWN_PATHS: &[&str] = &["/device/uid2", "/device/metadata", "/device/ext/context"];
pub const ID_PATHS: &[&str] = &[
    "/user/id",
    "/user/buyeruid",
    "/user/yob",
    "/user/gender",
    "/user/eids",
    "/user/ext/eids",
    "/device/ifa",
    "/device/dpidsha1",
    "/device/dpidmd5",
    "/device/didsha1",
    "/device/didmd5",
    "/device/macsha1",
    "/device/macmd5",
    "/device/uid2",
    "/device/metadata",
    "/device/ext/context",
];
pub const FPD_PATHS: &[&str] = &[
    "/user/data",
    "/user/keywords",
    "/user/kwarray",
    "/user/ext/data",
];

#[derive(Clone, Debug)]
pub struct Construction {
    pub bidder: String,
    pub core: String,
    pub input: Value,
}

#[derive(Clone, Debug)]
pub struct ResponseHook {
    pub bidder: String,
    pub input: Value,
    pub covered_imp_ids: Vec<String>,
}

struct RecordingBidder {
    builds: Arc<Mutex<Vec<Construction>>>,
    response_hooks: Arc<Mutex<Vec<ResponseHook>>>,
    per_imp: bool,
}

impl Bidder for RecordingBidder {
    fn debug_body_schema(&self) -> DebugBodySchema {
        DebugBodySchema::Unsupported
    }

    fn make_requests(
        &self,
        request: &BidRequest,
        info: &BidderRequestInfo<'_>,
        sink: &mut OutboundRequestSink<'_>,
    ) -> Result<(), Vec<BidderError>> {
        let input = serde_json::to_value(request)
            .map_err(|error| vec![BidderError::failed_to_request_bids(error.to_string())])?;
        self.builds
            .lock()
            .map_err(|error| vec![BidderError::failed_to_request_bids(error.to_string())])?
            .push(Construction {
                bidder: info.bidder.as_str().to_owned(),
                core: info.core_name.as_str().to_owned(),
                input,
            });
        let mut headers = vec![("X-Test-Bidder", info.bidder.as_str())];
        if let Some(device) = &request.device {
            for (name, value) in [
                ("X-Test-Device-Ip", &device.ip),
                ("X-Test-Device-Ipv6", &device.ipv6),
            ] {
                if let Some(ip) = value {
                    headers.push((name, ip.as_str()));
                }
            }
        }
        FakeBidder {
            request_mode: if self.per_imp {
                RequestMode::Split
            } else {
                RequestMode::Aggregate
            },
            headers: &headers,
            debug_schema: DebugBodySchema::Unsupported,
            ..FakeBidder::default()
        }
        .make_requests(request, info, sink)
    }

    fn make_bids(
        &self,
        request: &BidRequest,
        sent: &OutboundRequestMeta,
        response: &BidderHttpResponse,
    ) -> Result<BidderResponse, Vec<BidderError>> {
        let input = serde_json::to_value(request)
            .map_err(|error| vec![BidderError::failed_to_unmarshal(error.to_string())])?;
        let bidder = sent
            .headers()
            .find(|(name, _)| name.eq_ignore_ascii_case("X-Test-Bidder"))
            .expect("the recording adapter tags every outbound request")
            .1
            .to_owned();
        self.response_hooks
            .lock()
            .map_err(|error| vec![BidderError::failed_to_unmarshal(error.to_string())])?
            .push(ResponseHook {
                bidder,
                input,
                covered_imp_ids: sent.covered_imp_ids().map(str::to_owned).collect(),
            });
        FakeBidder::default().make_bids(request, sent, response)
    }
}

#[derive(Default)]
struct RecordingFanout(Mutex<Vec<BidderHttpRequest>>);

#[async_trait(?Send)]
impl BidderFanout for RecordingFanout {
    async fn send_all(
        &self,
        calls: Vec<BidderCall>,
        _deadline: AuctionDeadline,
    ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
        let mut recorded = self.0.lock().unwrap();
        Ok(calls
            .into_iter()
            .map(|call| {
                let bids: Vec<Value> = call.request.covered_imp_ids().map(|imp| json!({
                "id": format!("bid-{imp}"), "impid": imp, "price": 1.0_f64, "w": 300_i32, "h": 250_i32
            })).collect();
                recorded.push(call.request);
                BidderCallResult {
                    result: Ok(BidderHttpResponse::new(
                        200,
                        vec![],
                        serde_json::to_vec(&json!({
                            "id": "fake_req01", "cur": "USD",
                            "seatbid": [{"seat": "fake_adapter", "bid": bids}]
                        }))
                        .unwrap(),
                    )),
                    elapsed: Duration::from_millis(1),
                }
            })
            .collect())
    }
}

pub struct Harness {
    pub host: HostConfig,
    pub account: AccountConfig,
    pub account_id: String,
    pub disabled: BTreeSet<BidderName>,
    pub sample_rates: BTreeMap<BidderName, f64>,
    pub elapsed: Duration,
    pub baked_disabled: BTreeSet<String>,
    pub multi_request: bool,
    pub headers: RequestHeaders<'static>,
}

impl Default for Harness {
    fn default() -> Self {
        Self {
            host: HostConfig {
                resource_limits: ResourceLimits {
                    default_tmax_ms: 1000,
                    max_tmax_ms: 5000,
                    max_bidder_response_bytes: 0x0004_0000,
                    max_bidders_per_auction: 50,
                    max_outbound_requests: 100,
                    max_transport_bytes: 0x0004_0000,
                    max_decoded_bytes: 0x0004_0000,
                    max_parsed_nodes: 0x0001_0000,
                    ..ResourceLimits::default()
                },
                ..HostConfig::default()
            },
            account: AccountConfig::default(),
            account_id: ACCOUNT_ID.into(),
            disabled: BTreeSet::new(),
            sample_rates: BTreeMap::new(),
            elapsed: Duration::ZERO,
            baked_disabled: BTreeSet::new(),
            multi_request: false,
            headers: RequestHeaders::default(),
        }
    }
}

impl Harness {
    pub fn activities(&mut self, activities: Value) {
        self.account.privacy.allowactivities = serde_json::from_value(activities).unwrap();
    }

    pub fn enforce_gdpr(&mut self) {
        self.host.gdpr.enabled = true;
        self.host.gdpr.default_value = "0".into();
        self.host.gdpr.tcf2.enabled = true;
        for id in 1..=10 {
            self.host.gdpr.tcf2.purposes.insert(
                id,
                PurposeEnforcement {
                    enforce_purpose: TriState::Enabled,
                    enforce_vendors: TriState::Enabled,
                    ..PurposeEnforcement::default()
                },
            );
        }
    }

    pub fn run(&self, request_value: Value) -> Observed {
        let request: BidRequest = serde_json::from_value(request_value).expect("typed fixture");
        let normalized = prepare(
            request,
            &self.headers,
            &PrepareLimits {
                max_impressions: 100,
                ..PrepareLimits::default()
            },
        )
        .expect("fixture passes real prepare");
        let original = serde_json::to_value(&*normalized).unwrap();
        let builds = Arc::new(Mutex::new(Vec::new()));
        let response_hooks = Arc::new(Mutex::new(Vec::new()));
        let mut registry = BidderRegistry::new();
        for (name, gpp_supported) in [("fake_bb", false), ("fake_ccc", true), ("fake_aaa", false)] {
            let mut info = BidderInfo {
                name: name.into(),
                endpoint: "https://bidder.example.test/openrtb".into(),
                uses_https: true,
                disabled: self.baked_disabled.contains(name),
                ..BidderInfo::default()
            };
            info.openrtb.gpp_supported = gpp_supported;
            registry
                .register(
                    Box::leak(Box::new(info)),
                    Box::new(RecordingBidder {
                        builds: Arc::clone(&builds),
                        response_hooks: Arc::clone(&response_hooks),
                        per_imp: self.multi_request,
                    }),
                )
                .expect("unique test bidder");
        }
        let aliases = resolve_aliases(&normalized, &registry, &self.disabled, &self.host.aliases)
            .expect("fixture aliases resolve before auction");
        let plan = AuctionPlan::new(
            aliases,
            self.disabled.clone(),
            response_budgets(&registry, &self.host),
            self.sample_rates.clone(),
        );
        let account = ResolvedAccount {
            id: self.account_id.clone(),
            source: AccountSource::DefaultFallback,
            config: &self.account,
        };
        let fanout = RecordingFanout::default();
        let clock = FakeClock::new(datetime!(2026-09-01 00:00 UTC));
        let rng = SeededRng::new(42);
        let overrides = BTreeMap::new();
        let (gvl, gcl) = consent_authorities();
        let start = clock.now_monotonic();
        clock.advance(self.elapsed);
        let ctx = AuctionContext::with_request_start_ext(
            start,
            &self.host,
            &account,
            &registry,
            &fanout,
            &clock,
            &rng,
            &gvl,
            &gcl,
            clock.now(),
            false,
            ExchangeFeatures {
                returnallbidstatus_default: true,
                ..ExchangeFeatures::default()
            },
            &overrides,
        );
        let mut audit = DisclosureAudit::default();
        let result = block_on(run_auction(&normalized, &ctx, &plan, &mut audit))
            .and_then(auctioneer_exchange::AuctionOutcome::into_bid_response);
        assert_eq!(
            serde_json::to_value(&*normalized).unwrap(),
            original,
            "bidder projection must not mutate the prepared request"
        );
        let constructions = builds.lock().unwrap().clone();
        let recorded_responses = response_hooks.lock().unwrap().clone();
        Observed {
            result,
            audit,
            original,
            constructions,
            response_hooks: recorded_responses,
            outbound: fanout.0.into_inner().unwrap(),
            rng_next: rng.next_u64(),
        }
    }
}

// Reuse the existing valid TC fixture, with deterministic matching GVL/GCL and time.
fn consent_authorities() -> (GvlStore, Gcl) {
    let tcf = parse_tcf_v2(CONSENT.trim()).unwrap();
    let purposes: Map<String, Value> = (1_u8..=10_u8)
        .map(|id| {
            (
                id.to_string(),
                json!({"id": id, "name": format!("Purpose {id}")}),
            )
        })
        .collect();
    let gvl = Gvl::load_from_json(
        &json!({
            "gvlSpecificationVersion": 3_i32, "tcfPolicyVersion": 5_i32,
            "vendorListVersion": tcf.vendor_list_version, "lastUpdated": "2026-07-01T00:00:00Z",
            "purposes": purposes, "specialPurposes": {}, "features": {},
            "specialFeatures": {"1": {"id": 1_i32, "name": "Geolocation"}},
            "vendors": {"8": {"id": 8_i32, "name": "Vendor", "purposes": [1_i32,2_i32,3_i32,4_i32,5_i32,6_i32,7_i32,8_i32,9_i32,10_i32],
                "legIntPurposes": [], "flexiblePurposes": [], "specialFeatures": [1_i32]}}
        })
        .to_string(),
        None,
    )
    .unwrap();
    (
        GvlStore::for_test(vec![(3, u32::from(tcf.vendor_list_version), gvl)]),
        Gcl::for_test(
            vec![CmpRecord {
                id: tcf.cmp_id,
                deleted_date: None,
            }],
            datetime!(2027-01-01 00:00 UTC),
        ),
    )
}

pub struct Observed {
    pub result: Result<BidResponse, AuctionError>,
    pub audit: DisclosureAudit,
    pub original: Value,
    pub constructions: Vec<Construction>,
    pub response_hooks: Vec<ResponseHook>,
    pub outbound: Vec<BidderHttpRequest>,
    pub rng_next: u64,
}

impl Observed {
    pub fn response(&self) -> &BidResponse {
        self.result
            .as_ref()
            .expect("valid auction returns a response")
    }

    pub fn ext(&self) -> &Value {
        self.response().ext.as_ref().expect("response bookkeeping")
    }

    pub fn body(&self, bidder: &str) -> Value {
        let requests = self.requests(bidder);
        assert_eq!(requests.len(), 1, "one outbound request for {bidder}");
        serde_json::from_slice(requests[0].body()).unwrap()
    }

    pub fn requests(&self, bidder: &str) -> Vec<&BidderHttpRequest> {
        self.outbound
            .iter()
            .filter(|request| {
                request.headers().any(|(name, value)| {
                    name.eq_ignore_ascii_case("X-Test-Bidder") && value == bidder
                })
            })
            .collect()
    }

    pub fn assert_built(&self, expected: &[&str]) {
        let mut actual: Vec<&str> = self
            .constructions
            .iter()
            .map(|build| build.bidder.as_str())
            .collect();
        let mut expected_names = expected.to_vec();
        actual.sort_unstable();
        expected_names.sort_unstable();
        assert_eq!(
            actual, expected_names,
            "adapter construction counts/identities"
        );
    }

    pub fn assert_blocked(&self, bidder: &str, imp_ids: &[&str]) {
        assert!(
            self.constructions
                .iter()
                .all(|build| build.bidder != bidder)
        );
        assert!(self.requests(bidder).is_empty());
        assert_eq!(
            self.ext()["warnings"][bidder],
            json!([{
                "code": 10_017_i32, "message": format!("bidder blocked by privacy settings: {bidder}")
            }]),
            "exactly one privacy warning"
        );
        let rows: Vec<&Value> = self.ext()["prebid"]["seatnonbid"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["seat"] == bidder)
            .collect();
        assert_eq!(rows.len(), 1, "one seat exclusion, even if both gates deny");
        let expected: Vec<Value> = imp_ids
            .iter()
            .map(|id| json!({"impid": id, "statuscode": 300_i32}))
            .collect();
        assert_eq!(rows[0]["nonbid"], json!(expected));
        assert!(
            self.response()
                .seatbid
                .iter()
                .all(|seat| seat.seat != bidder)
        );
    }

    pub fn warnings(&self) -> Vec<Value> {
        self.ext()
            .pointer("/warnings/prebid")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    }

    pub fn assert_bad_input(&self, message: &str) {
        assert!(
            matches!(&self.result, Err(AuctionError::BadInput(actual)) if actual == message),
            "expected fixed BadInput {message:?}, got {:?}",
            self.result
        );
        self.assert_built(&[]);
        assert!(
            self.outbound.is_empty(),
            "hard privacy errors cannot dispatch"
        );
    }
}

pub fn request(names: &[&str]) -> Value {
    let bidders: Map<String, Value> = names
        .iter()
        .map(|name| ((*name).into(), json!({})))
        .collect();
    json!({
        "id": "fake_req01", "cur": ["USD"], "tmax": 1000,
        "site": {"id": "site-context", "ext": {"data": {"section": "news"}}},
        "imp": [{"id": "imp-1", "banner": {"w": 300, "h": 250},
            "ext": {"bidder": bidders, "tid": "imp-tid", "data": {"slot": "top"}}}],
        "source": {"tid": "source-tid", "ext": {"schain": {"ver": "1.0", "complete": 1}}},
        "user": {"id": "user-id", "buyeruid": "buyer-id", "yob": 1980, "gender": "F",
            "keywords": "private-keyword", "kwarray": ["private-array"],
            "data": [{"id": "private-segment"}],
            "eids": [{"source": "native.example", "uids": [{"id": "native-eid"}]}],
            "ext": {"data": {"audience": "private-audience"},
                "eids": [{"source": "legacy.example", "uids": [{"id": "legacy-eid"}]}],
                "context": "keep-user-extension"},
            "geo": full_geo()},
        "device": {"ip": CLIENT_V4, "ipv6": CLIENT_V6, "ifa": "ifa-id",
            "dpidsha1": "dpid-sha1", "dpidmd5": "dpid-md5", "didsha1": "did-sha1",
            "didmd5": "did-md5", "macsha1": "mac-sha1", "macmd5": "mac-md5",
            "uid2": "opaque-root-device-id", "metadata": {"sha256email": "opaque-email"},
            "geo": full_geo(), "ext": {"context": "keep-device-extension"}},
        "regs": {}, "ext": {"prebid": {"returnallbidstatus": true}}
    })
}

fn full_geo() -> Value {
    json!({"lat": 48.858_844, "lon": -1.125, "country": "USA", "region": "CA",
        "regionfips104": "US06", "metro": "807", "city": "San Francisco", "zip": "94107",
        "type": 1, "accuracy": 7, "lastfix": 10, "ipservice": 3, "utcoffset": -420,
        "ext": {"private_location": "precise"}, "extra_geo": "also-private"})
}

pub fn second_imp(value: &mut Value) {
    let mut imp = value["imp"][0].clone();
    imp["id"] = json!("imp-2");
    imp["ext"]["tid"] = json!("second-imp-tid");
    value["imp"].as_array_mut().unwrap().push(imp);
}

pub fn absent(body: &Value, paths: &[&str]) {
    for path in paths {
        assert!(
            body.pointer(path).is_none(),
            "prohibited {path} survived adapter serialization"
        );
    }
}

pub fn unchanged(body: &Value, original: &Value, paths: &[&str]) {
    for path in paths {
        assert!(
            original.pointer(path).is_some(),
            "control fixture lacks {path}"
        );
        assert_eq!(
            body.pointer(path),
            original.pointer(path),
            "{path} must survive"
        );
    }
}

pub fn coarse_geo(body: &Value) {
    assert_eq!(body["device"]["ip"], "8.8.4.0");
    assert_eq!(body["device"]["ipv6"], "2001:4860:4860::");
    for owner in ["user", "device"] {
        assert_eq!(
            body[owner]["geo"],
            json!({"lat": 48.86_f64, "lon": -1.12_f64, "country": "USA",
                "region": "CA", "regionfips104": "US06", "metro": "807",
                "city": "San Francisco", "type": 1_i32, "lastfix": 10_i32,
                "ipservice": 3_i32, "utcoffset": -420_i32}),
            "coarsening retains reviewed context, omits precision/unknowns and rounds ties half up"
        );
    }
}

pub fn no_raw_ips(requests: &[BidderHttpRequest]) {
    for request in requests {
        let body = String::from_utf8(request.body().to_vec()).unwrap();
        for raw in [CLIENT_V4, CLIENT_V6] {
            assert!(!body.contains(raw), "raw IP in serialized body");
            assert!(!request.url().contains(raw), "raw IP in outbound URL");
            for (name, value) in request.headers() {
                assert!(!value.contains(raw), "raw IP resurrected in {name}");
            }
        }
    }
}
