//! Snapshot fixtures for GDPR-in-scope traffic carrying a TCF v2 string.
//!
//! Handler and outbound rows over one deployment blob each:
//!
//! 1. `regs.gdpr = 1` with a usable TC string. The consenting vendor-32
//!    base is authorized and bids. The no-vendor base is refused by the
//!    purpose-2 gate and leaves a warning plus a non-bid row. A HOST-declared
//!    alias whose base the account opts into basic enforcement bids without
//!    consulting the GVL for vendor consent. The row fails if a config-consuming
//!    route resolves the host without the runtime `[gdpr]` and `[aliases]`
//!    overlays: with the baked host alone, the no-vendor base would not be
//!    dropped and the basic-enforcement alias would not resolve.
//! 2. The same request under `regs.gpp_sid = [2]`. An absent envelope
//!    permits legacy fallback, but a present-empty or invalid active
//!    section 2 is authoritative and refuses bidders requiring consent.
//!    The paired controls prevent fallback from bypassing that denial.
//! 3. The same section-2 shape with purpose 2 enforcement OFF, so one
//!    bidder survives to fan-out and the identifier scrub is observable
//!    on the serialized outbound body.
//! 4. Row 3's sibling with special feature 1 enforced, which additionally
//!    coarsens geo and masks the client IP on that same wire body.
//! 5. All four resolved GDPR ID/geo combinations over the outbound fixture,
//!    including an ID-permitted geo-only row and a fully permitted row.
//!    Typed host purpose-2 vendor exceptions enable ID disclosure; SF1
//!    enforcement independently controls geo. No activity decision is mocked.
//!
//! Rows 3 through 5 drive `run_auction` directly with a recording
//! `BidderFanout` on the `AuctionContext`, because the assertion is about
//! what a bidder RECEIVES rather than what the auction answers. The
//! recording boundary carries already-built, already-serialized adapter
//! requests. The canonical test-only fake serializes the scoped `OpenRTB`
//! request as the call body, so those rows bind one synthetic bidder imp.
//! These are core config-resolution/exchange integrations, not HTTP-handler
//! wire tests; the handler snapshot rows above remain separate.
//!
//! The locally authored handler JSON/TOML and goldens use symbolic roles.
//! Serving metadata selects the consenting vendor, no-vendor base and distinct
//! basic-enforcement base; parameters come from their bidder-owned fixtures.
//! All roles are host-declared aliases, so requests and literal expected output
//! keep fake-prefixed identities. Vendor 32 and its pinned consent remain
//! load-bearing. Common response goldens are supplemented only with independent
//! bidder-owned additive banner metadata, never observations of bidder output.
//!
//! Every row pins the clock to a `FakeClock` at the UNIX epoch. That
//! fixes `ext.prebid.auctiontimestamp`, every response-time figure, the
//! GVL/CMP authorization instant, and the whole-auction deadline; a real
//! clock would let a slow runner overrun `tmax` and record an
//! auction-wide timeout entry into the expectation.
//!
//! # The `user.consent` string the request fixtures carry
//!
//! A TCF v2 core segment plus the mandatory Disclosed Vendors segment,
//! declaring: CMP 300 (registered in the bundled Global CMP List),
//! `vendorListVersion = 174` and `tcfPolicyVersion = 5` (a pinned
//! revision in the bundled GVL archive), purpose consent for 1-4,
//! purpose-2 legitimate-interest transparency, and both vendor consent
//! and vendor legitimate interest exclusively for GVL vendor 32.
//! Vendor 32 declares purpose 2 under legitimate interest
//! (flexible) in the real vendor list, so the legitimate-interest leg is
//! what opens its `AllowBidRequest` gate — and it is load-bearing:
//! without it the vendor is denied even though the purpose-2 consent bit
//! and vendor consent are both set. The basic-opted base grants on
//! either the purpose-2 consent bit or the
//! legitimate-interest transparency bit, so its grant is
//! over-determined here; no single bit is solely responsible. It never
//! reads a vendor declaration.
//!
//! The string names a concrete archive revision, so a GVL refresh that
//! ages revision 174 out of the bundle will make consent unusable and
//! these rows fail. The fix is re-encoding the core segment against a
//! bundled revision, not relaxing the expectation.

mod support;

#[cfg(test)]
mod config_funding_support;
use config_funding_support as config_funding;

#[cfg(test)]
pub use auctioneer_core::test_support::bidder_fixture;

#[cfg(test)]
use auctioneer_bidder::test_support::serving_catalog;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use crate::bidder_fixture::fake_bidder::FakeBidder;
    use crate::config_funding;
    use crate::serving_catalog::{self, ServingBidder};
    use crate::support::ctx_with_config;
    use async_trait::async_trait;
    use auctioneer_bidder::{BidderHttpResponse, BidderRegistry};
    use auctioneer_config_data::types::{BidderInfo, Capabilities, MediaType, TriState};
    use auctioneer_core::config::AuctioneerAppConfig;
    use auctioneer_core::config::budgets::resolve_budgets;
    use auctioneer_core::config::resolve::{resolve_account, resolve_host};
    use auctioneer_core::handlers::{TestFanoutHandle, auction};
    use auctioneer_core::host_primitives::{
        TestClockHandle, TestMonotonicClockHandle, TestRngHandle,
    };
    use auctioneer_core::test_support::config_store::auction_context_for_route;
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::{
        AuctionContext, AuctionDeadline, AuctionPlan, BidderCall, BidderCallResult, BidderFanout,
        FanoutError, FanoutInternalError, ResolvedAccount, ResolvedAliases, TimeoutCause,
        resolve_aliases, run_auction,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare};
    use auctioneer_privacy::gcl::Gcl;
    use auctioneer_privacy::gvl::GvlStore;
    use auctioneer_privacy::{AuctionPermissions, PrivacyContext, ResolveOptions};
    use auctioneer_runtime::{Clock, FakeClock, Rng, SeededRng};
    use edgezero_core::body::Body;
    use edgezero_core::http::StatusCode;
    use edgezero_core::response_egress::ResponseEgressEnvelope;
    use edgezero_core::time::MonotonicClock;
    use futures::executor::block_on;
    use http::{Method, Request, Uri};
    use serde_json::{Value, json};
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::mem;
    use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
    use std::time::Duration;
    use time::OffsetDateTime;

    /// Response cap wide enough that nothing here trips it.
    const WIDE_CAP: usize = 1 << 20;
    const OUTBOUND_BIDDER: &str = "fake_bidder";

    static AUCTIONS: Mutex<()> = Mutex::new(());

    /// Impression cap wide enough that the fixtures below never reach
    /// it; the cap itself is exercised by the max-impressions tests in
    /// `auctioneer-openrtb`.
    const IMP_CAP_NOT_UNDER_TEST: PrepareLimits = PrepareLimits {
        max_impressions: 100,
        max_copied_header_bytes: 4_096,
    };

    /// The device-id keys that have no typed `Device` field and ride in
    /// the flattened catch-all instead.
    const LEGACY_DEVICE_ID_KEYS: [&str; 6] = [
        "dpidsha1", "dpidmd5", "didsha1", "didmd5", "macsha1", "macmd5",
    ];

    fn fixture(name: &str) -> String {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("fixture {path} unreadable: {err}"))
    }

    fn handler_roles() -> BTreeMap<&'static str, ServingBidder> {
        let rows = serving_catalog::default_active_site_media(&[MediaType::Banner]);
        let consented = rows
            .iter()
            .find(|row| row.info.gvl_vendor_id == Some(32))
            .unwrap();
        let no_vendor = rows
            .iter()
            .find(|row| row.info.gvl_vendor_id.unwrap_or(0) == 0)
            .unwrap();
        let basic = rows
            .iter()
            .find(|row| {
                row.info
                    .gvl_vendor_id
                    .is_some_and(|vendor| vendor != 0 && vendor != 32)
            })
            .unwrap();
        assert_ne!(consented.name(), basic.name());
        BTreeMap::from([
            ("fake_consented_bidder", consented.clone()),
            ("fake_bidder", no_vendor.clone()),
            ("fake_alias", basic.clone()),
        ])
    }

    fn handler_fixture(name: &str) -> String {
        let roles = handler_roles();
        if name == "gdpr_applies.toml" {
            let mut config: toml::Value = toml::from_str(&fixture(name)).unwrap();
            let aliases = config["host"]["aliases"].as_table_mut().unwrap();
            assert_eq!(aliases.len(), roles.len());
            for (role, row) in &roles {
                let base = aliases.get_mut(*role).unwrap();
                assert!(base.as_str().unwrap().starts_with("fake_"));
                *base = row.name().into();
            }
            let basic = &mut config["accounts"]["acct-1"]["gdpr"]["basic_enforcement_vendors"];
            assert_eq!(
                basic.as_array().unwrap(),
                &[toml::Value::String("fake_base".into())]
            );
            *basic = toml::Value::Array(vec![roles["fake_alias"].name().into()]);
            return toml::to_string(&config).unwrap();
        }
        let mut source: Value = serde_json::from_str(&fixture(name)).unwrap();
        let mut used = BTreeSet::new();
        for imp in source["imp"].as_array_mut().unwrap() {
            for (role, placeholder) in imp["ext"]["prebid"]["bidder"].as_object_mut().unwrap() {
                assert_eq!(
                    *placeholder,
                    json!({}),
                    "parameters belong to the bidder owner"
                );
                assert!(used.insert(role.clone()));
                *placeholder = roles[role.as_str()].params.clone();
            }
        }
        assert_eq!(used, roles.keys().map(|role| (*role).to_owned()).collect());
        serde_json::to_string(&source).unwrap()
    }

    // ───────── the handler-path rows ─────────

    /// Answers each dispatched call with one banner bid per imp the call
    /// covers, so a bidder's seat row carries exactly the imps it was
    /// actually asked about.
    struct BiddingFanout;

    #[async_trait(?Send)]
    impl BidderFanout for BiddingFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            Ok(calls
                .iter()
                .map(|call| {
                    let bids: Vec<Value> = call
                        .request
                        .covered_imp_ids()
                        .map(|imp_id| {
                            json!({
                                "id": format!("bid-{imp_id}"),
                                "impid": imp_id,
                                "price": 1.5_f64,
                                "mtype": 1_i32,
                                "w": 300_i32,
                                "h": 250_i32,
                                "crid": format!("creative-{imp_id}")
                            })
                        })
                        .collect();
                    BidderCallResult {
                        result: Ok(BidderHttpResponse::new(
                            200,
                            vec![],
                            serde_json::to_vec(&json!({
                                "id": "r",
                                "seatbid": [{"seat": "s", "bid": bids}],
                                "cur": "USD"
                            }))
                            .expect("bid response serializes"),
                        )),
                        elapsed: Duration::from_millis(10),
                    }
                })
                .collect())
        }
    }

    /// POST a fixture body through the real auction handler with the
    /// given deployment blob installed.
    /// The returned guard must outlive the paired terminal envelope.
    fn drive_handler(
        config_toml: &str,
        request_body: &str,
    ) -> (
        MutexGuard<'static, ()>,
        StatusCode,
        serde_json::Value,
        ResponseEgressEnvelope,
    ) {
        let serialized = AUCTIONS.lock().unwrap_or_else(PoisonError::into_inner);
        config_funding::install();
        let fake = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let clock: Arc<dyn Clock + Send + Sync> = Arc::<FakeClock>::clone(&fake);
        let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));
        let mut request = Request::builder()
            .method(Method::POST)
            .uri(Uri::from_static("/openrtb2/auction"))
            .header("content-type", "application/json")
            .body(Body::from_bytes(request_body.as_bytes().to_vec()))
            .expect("request builds");
        request
            .extensions_mut()
            .insert(TestFanoutHandle(Arc::new(BiddingFanout)));
        request.extensions_mut().insert(TestClockHandle(clock));
        // `TestClockHandle` governs wall-clock reads only: after the
        // clock-domain migration the auction path takes monotonic time
        // from `ctx.monotonic_clock()` and never consults it. Without
        // this handle `responsetimemillis` sums a frozen
        // `max_call_elapsed` with REAL elapsed time for the other two
        // terms, so a recorded response containing it drifts by a millisecond
        // under load.
        let monotonic_source = Arc::<FakeClock>::clone(&fake);
        request
            .extensions_mut()
            .insert(TestMonotonicClockHandle(MonotonicClock::new(move || {
                monotonic_source.now_monotonic()
            })));
        request.extensions_mut().insert(TestRngHandle(rng));
        let (ctx, mut envelope) = auction_context_for_route(
            ctx_with_config(config_toml, request)
                .into_request()
                .unwrap(),
            "/openrtb2/auction",
            MonotonicClock::default(),
        );

        block_on(async {
            *envelope.response_mut() = auction(ctx).await.expect("the handler answers");
            let response = mem::take(envelope.response_mut());
            let status = response.status();
            let (_parts, body) = response.into_parts();
            let bytes = body
                .into_bytes_bounded(WIDE_CAP)
                .await
                .expect("response body reads");
            (
                serialized,
                status,
                serde_json::from_slice(&bytes).unwrap_or_else(|err| {
                    panic!(
                        "the response is JSON: {err}; body {}",
                        String::from_utf8_lossy(&bytes),
                    )
                }),
                envelope,
            )
        })
    }

    fn expected_response(expected_name: &str) -> Value {
        let mut expected: Value =
            serde_json::from_str(&fixture(expected_name)).expect("the expectation is JSON");
        let Some(seats) = expected.get_mut("seatbid") else {
            return expected;
        };
        let roles = handler_roles();
        for seat in seats.as_array_mut().expect("recorded seats are an array") {
            let role = seat["seat"].as_str().unwrap();
            let facts = roles[role].privacy_wire_contract().unwrap();
            for bid in seat["bid"].as_array_mut().unwrap() {
                let prebid = bid["ext"]["prebid"].as_object_mut().unwrap();
                for (name, value) in facts.banner_response_prebid() {
                    assert!(
                        prebid.insert(name.clone(), value.clone()).is_none(),
                        "protocol facts cannot replace common response expectations"
                    );
                }
            }
        }
        expected
    }

    fn assert_matches_recorded(actual: &serde_json::Value, expected_name: &str) {
        let expected = expected_response(expected_name);
        assert_eq!(
            *actual,
            expected,
            "recorded answer drifted; actual was {}",
            serde_json::to_string_pretty(actual).expect("actual re-serialises"),
        );
        // A recorded auction-wide timeout would mean the run overran
        // `tmax`, which a pinned clock makes impossible — and a recorded response
        // that enshrined one would assert the wrong contract forever.
        assert!(
            !serde_json::to_string(actual)
                .expect("actual re-serialises")
                .contains("\"code\":12"),
            "the pinned clock must never let an auction-wide timeout reach the recorded response",
        );
    }

    /// The `user.consent` core segment hard-codes this list version, and
    /// the archive's 180-day retention will eventually age it out. Check
    /// it up front: past that point the resolver falls back to
    /// enforced-but-empty and the row fails on a multi-kilobyte response
    /// diff that reads like a privacy regression, when the real fix is a
    /// one-line re-encode.
    const PINNED_GVL_LIST: u32 = 174;

    #[test]
    fn gdpr_applies_with_usable_consent_grants_authorized_bidders_only() {
        assert!(
            GvlStore::baked().get(3, PINNED_GVL_LIST).is_some(),
            "this fixture pins GVL spec 3 list {PINNED_GVL_LIST}, which the bundled archive \
             no longer carries -- the retention window aged it out. Re-encode the core \
             segment of `user.consent` in gdpr_applies_request.json against a bundled \
             revision; the response diff below is a symptom, not the cause.",
        );
        let (_serialized, status, actual, _envelope) = drive_handler(
            &handler_fixture("gdpr_applies.toml"),
            &handler_fixture("gdpr_applies_request.json"),
        );
        assert_eq!(status, StatusCode::OK);
        assert_matches_recorded(&actual, "gdpr_applies_expected.json");
    }

    #[test]
    fn gpp_sid_section_two_without_an_envelope_uses_legacy_consent() {
        let (_serialized, status, actual, _envelope) = drive_handler(
            &handler_fixture("gdpr_applies.toml"),
            &handler_fixture("gdpr_applies_section2_request.json"),
        );
        assert_eq!(status, StatusCode::OK);
        let mut expected = expected_response("gdpr_applies_expected.json");
        expected["id"] = json!("auc-gdpr-section2");
        expected["ext"]["warnings"]["prebid"] = json!([{
            "code": 10001_u32,
            "message": "regs.gdpr set together with regs.gpp_sid; gpp_sid presence disambiguates when non-empty, else regs.gdpr wins"
        }]);
        assert_eq!(actual, expected);
    }

    #[test]
    fn empty_active_gpp_section_two_never_retries_granting_legacy_consent() {
        let mut request: Value =
            serde_json::from_str(&handler_fixture("gdpr_applies_section2_request.json"))
                .expect("request fixture is JSON");
        request["regs"]["gpp"] = json!("DBABM~");
        let (_serialized, status, actual, _envelope) = drive_handler(
            &handler_fixture("gdpr_applies.toml"),
            &serde_json::to_string(&request).expect("request serializes"),
        );
        assert_eq!(status, StatusCode::OK);
        assert_matches_recorded(&actual, "gdpr_applies_section2_expected.json");
    }

    #[test]
    fn invalid_active_gpp_section_two_never_retries_granting_legacy_consent() {
        let mut request: Value =
            serde_json::from_str(&handler_fixture("gdpr_applies_section2_request.json"))
                .expect("request fixture is JSON");
        request["regs"]["gpp"] = json!("DBABM~invalid");
        let (_serialized, status, actual, _envelope) = drive_handler(
            &handler_fixture("gdpr_applies.toml"),
            &serde_json::to_string(&request).expect("request serializes"),
        );
        assert_eq!(status, StatusCode::OK);
        let mut expected: Value =
            serde_json::from_str(&fixture("gdpr_applies_section2_expected.json"))
                .expect("denied-consent expectation is JSON");
        expected["ext"]["warnings"]["prebid"]
            .as_array_mut()
            .expect("prebid warnings are an array")
            .extend([
                json!({
                    "code": 10001_u32,
                    "message": "regs.gpp section 2 and user.consent conflict; using section 2"
                }),
                json!({
                    "code": 10001_u32,
                    "message": "regs.gpp section 2 is not a valid TCF v2 string; treating as no consent"
                }),
            ]);
        assert_eq!(actual, expected);
    }

    // ───────── the outbound-body rows ─────────

    /// Keeps every dispatched call's serialized body and answers with a
    /// benign per-bidder timeout, so the auction still completes and the
    /// bodies stay inspectable afterwards.
    #[derive(Default)]
    struct RecordingFanout {
        bodies: Mutex<Vec<Vec<u8>>>,
    }

    #[async_trait(?Send)]
    impl BidderFanout for RecordingFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            {
                let mut recorded = self
                    .bodies
                    .lock()
                    .expect("the recording lock is uncontended");
                for call in &calls {
                    recorded.push(call.request.body().to_vec());
                }
            }
            Ok(calls
                .iter()
                .map(|_| BidderCallResult {
                    result: Err(FanoutError::Timeout(TimeoutCause::Bidder)),
                    elapsed: Duration::ZERO,
                })
                .collect())
        }
    }

    fn outbound_registry() -> BidderRegistry {
        let mut registry = BidderRegistry::new();
        let info = Box::leak(Box::new(BidderInfo {
            name: OUTBOUND_BIDDER.into(),
            endpoint: "https://bidder.example/auction".into(),
            uses_https: true,
            capabilities: Capabilities {
                site: vec![MediaType::Banner],
                ..Capabilities::default()
            },
            ..BidderInfo::default()
        }));
        registry
            .register(info, Box::<FakeBidder>::default())
            .expect("unique test bidder");
        registry
    }

    /// Run the fixture request straight through `run_auction` with a
    /// recording fanout on the context, and hand back the one outbound
    /// `BidRequest` the canonical fake serialized.
    ///
    /// The deployment blob is resolved to its `HostConfig` +
    /// `ResolvedAccount` here, exactly as the handler would, because this
    /// row bypasses the HTTP layer to reach the fan-out boundary.
    fn outbound_request(config_toml: &str, request_body: &str) -> BidRequest {
        let app: AuctioneerAppConfig = toml::from_str(config_toml).expect("fixture TOML parses");
        let (body, _) = outbound_body(&app, request_body);
        serde_json::from_value(body).expect("the recorded body is a request")
    }

    /// Capture actual serialized transport bytes after core config resolution.
    fn outbound_body(app: &AuctioneerAppConfig, request_body: &str) -> (Value, AuctionPermissions) {
        let _serialized = AUCTIONS.lock().unwrap_or_else(PoisonError::into_inner);
        let host = resolve_host(app, auctioneer_config_data::host_default());
        let owned = resolve_account(
            request_body.as_bytes(),
            app,
            auctioneer_config_data::account_default(),
            u32::MAX,
        )
        .expect("the fixture account is not disabled");
        let account = ResolvedAccount {
            id: owned.id.clone(),
            source: owned.source,
            config: &owned.config,
        };
        let registry = outbound_registry();
        let fanout = RecordingFanout::default();
        let clock = FakeClock::new(OffsetDateTime::UNIX_EPOCH);
        let rng = SeededRng::new(42);
        let raw: BidRequest = serde_json::from_str(request_body).expect("the fixture is a request");
        let normalized = prepare(raw, &RequestHeaders::default(), &IMP_CAP_NOT_UNDER_TEST)
            .expect("the fixture is valid");
        let permissions = PrivacyContext::resolve(
            &normalized,
            account.config,
            &host,
            GvlStore::baked(),
            Gcl::baked(),
            clock.now(),
            ResolveOptions::default(),
        )
        .expect("the fixture's privacy signals resolve")
        .context
        .permissions(OUTBOUND_BIDDER, OUTBOUND_BIDDER);

        block_on(async {
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
                resolve_aliases(&normalized, &registry, &BTreeSet::new(), &BTreeMap::new())
                    .expect("the fixture declares no aliases"),
                BTreeSet::new(),
                resolve_budgets(
                    &AuctioneerAppConfig::default(),
                    &host,
                    "",
                    account.source,
                    &registry,
                    &ResolvedAliases::empty(),
                ),
                BTreeMap::new(),
            );
            let mut audit = DisclosureAudit::default();
            run_auction(&normalized, &ctx, &plan, &mut audit)
                .await
                .expect("the auction completes")
                .into_bid_response()
                .expect("the response projects");
        });

        let recorded = fanout
            .bodies
            .into_inner()
            .expect("the recording lock is uncontended");
        assert_eq!(
            recorded.len(),
            1,
            "the surviving bidder must reach fan-out exactly once; a drop here would mean the \
             purpose-2 gate refused it and nothing would be left to scrub",
        );
        let body = recorded.first().expect("one recorded call body");
        (
            serde_json::from_slice(body).expect("the canonical fake serializes the scoped request"),
            permissions,
        )
    }

    /// Every field `scrub_gdpr_id` clears, asserted on a wire body.
    fn assert_identifiers_scrubbed(request: &BidRequest) {
        let original: BidRequest =
            serde_json::from_str(&fixture("gdpr_section2_scrub_request.json"))
                .expect("loaded pre-scrub request");
        let original_device = original.device.as_ref().expect("loaded pre-scrub device");
        for key in LEGACY_DEVICE_ID_KEYS {
            assert!(
                original_device.other.contains_key(key),
                "fixture must contain {key} before scrub"
            );
        }
        let user = request.user.as_ref().expect("the request carries a user");
        assert_eq!(user.id, None);
        assert_eq!(user.buyeruid, None);
        assert_eq!(user.yob, None);
        assert_eq!(user.gender, None);
        assert!(user.eids.is_empty(), "native 2.6 eids survived the scrub");
        assert_eq!(
            user.ext.as_ref().and_then(|ext| ext.eids.clone()),
            None,
            "the legacy `user.ext.eids` location survived the scrub",
        );
        let device = request
            .device
            .as_ref()
            .expect("the request carries a device");
        assert_eq!(device.ifa, None);
        for key in LEGACY_DEVICE_ID_KEYS {
            assert!(
                !device.other.contains_key(key),
                "legacy device id `{key}` survived the scrub",
            );
        }
    }

    /// Render a coordinate as its decimal text, so the expectation reads
    /// as a literal and no float comparison is involved.
    fn coord(value: Option<f64>) -> String {
        value.map_or_else(|| "<absent>".to_owned(), |number| number.to_string())
    }

    #[test]
    fn section_two_scrubs_every_identifier_on_the_outbound_body() {
        let request = outbound_request(
            &fixture("gdpr_section2_scrub.toml"),
            &fixture("gdpr_section2_scrub_request.json"),
        );
        assert_identifiers_scrubbed(&request);

        // Geo and the client IP belong to the OTHER gate, which this
        // deployment does not enforce: they must arrive untouched.
        let device = request
            .device
            .as_ref()
            .expect("the request carries a device");
        assert_eq!(device.ip.as_deref(), Some("93.184.216.34"));
        assert_eq!(
            device.ipv6.as_deref(),
            Some("2606:2800:220:abcd:1234:5678:9abc:def0"),
        );
        let device_geo = device.geo.as_ref().expect("device geo survives");
        assert_eq!(coord(device_geo.lat), "52.526789");
        assert_eq!(coord(device_geo.lon), "13.412345");
        let user_geo = request
            .user
            .as_ref()
            .and_then(|user| user.geo.as_ref())
            .expect("user geo survives");
        assert_eq!(coord(user_geo.lat), "-33.868821");
        assert_eq!(coord(user_geo.lon), "151.209295");
    }

    #[test]
    fn special_feature_one_coarsens_geo_and_masks_the_ip_on_the_outbound_body() {
        let request = outbound_request(
            &fixture("gdpr_section2_geo_scrub.toml"),
            &fixture("gdpr_section2_scrub_request.json"),
        );
        // The identifier gate is independent and still fires.
        assert_identifiers_scrubbed(&request);

        let device = request
            .device
            .as_ref()
            .expect("the request carries a device");
        assert_eq!(
            device.ip.as_deref(),
            Some("93.184.216.0"),
            "IPv4 is masked to the account's keep-bits",
        );
        assert_eq!(
            device.ipv6.as_deref(),
            Some("2606:2800:220:ab00::"),
            "IPv6 is masked to the account's keep-bits",
        );
        let device_geo = device.geo.as_ref().expect("device geo survives coarsening");
        assert_eq!(coord(device_geo.lat), "52.53");
        assert_eq!(coord(device_geo.lon), "13.41");
        let user_geo = request
            .user
            .as_ref()
            .and_then(|user| user.geo.as_ref())
            .expect("user geo survives coarsening");
        assert_eq!(coord(user_geo.lat), "-33.87");
        assert_eq!(coord(user_geo.lon), "151.21");
    }

    fn matrix_config(pass_id: bool, pass_geo: bool) -> AuctioneerAppConfig {
        let mut app: AuctioneerAppConfig =
            toml::from_str(&fixture("gdpr_section2_geo_scrub.toml")).expect("fixture TOML parses");
        let tcf2 = app
            .host
            .gdpr
            .as_mut()
            .expect("fixture enables GDPR")
            .tcf2
            .as_mut()
            .expect("fixture enables TCF2");
        // PassID forces purpose/vendor enforcement even when the config disables
        // it. An explicit exception, with usable consent, opens this gate instead.
        tcf2.purposes
            .as_mut()
            .expect("fixture configures purposes")
            .get_mut(&2)
            .expect("fixture configures purpose 2")
            .vendor_exceptions = Some(if pass_id {
            vec![OUTBOUND_BIDDER.into()]
        } else {
            vec![]
        });
        tcf2.special_features
            .as_mut()
            .expect("fixture configures special features")
            .get_mut(&1)
            .expect("fixture configures SF1")
            .enforce = Some(if pass_geo {
            TriState::Disabled
        } else {
            TriState::Enabled
        });
        app
    }

    fn matrix_request() -> Value {
        let mut request: Value = serde_json::from_str(&fixture("gdpr_section2_scrub_request.json"))
            .expect("fixture request is JSON");
        let device = request["device"]
            .as_object_mut()
            .expect("fixture has device");
        let context = json!({
            "dnt": 1_i32, "lmt": 0_i32, "os": "OS", "devicetype": 1_i32,
            "make": "maker", "model": "model", "osv": "1", "hwv": "hw",
            "h": 800_i32, "w": 600_i32, "ppi": 120_i32, "pxratio": 2_i32,
            "js": 1_i32, "geofetch": 1_i32, "flashver": "1", "language": "en",
            "langb": "en-GB", "carrier": "carrier", "mccmnc": "123456",
            "connectiontype": 2_i32,
            "sua": {"mobile": 1_i32, "source": 2_i32, "architecture": "arm",
                "bitness": "64", "model": "model",
                "browsers": [{"brand": "browser", "version": ["1"],
                    "ext": {"private": "browser-ext"}, "unknown": "browser-unknown"}],
                "platform": {"brand": "platform", "version": ["1"],
                    "unknown": "platform-unknown"},
                "ext": {"private": "sua-ext"}, "unknown": "sua-unknown"},
            "ext": {"atts": 2_i32, "prebid": {"interstitial": {
                "minwidthperc": 70_i32, "minheightperc": 60_i32,
                "unknown": "interstitial-unknown"}, "unknown": "prebid-unknown"},
                "unknown": "device-ext-unknown"},
            "unknown": [{"private": "device-unknown"}]
        });
        device.extend(context.as_object().expect("context is an object").clone());
        request["user"]["customdata"] = json!("custom-id");
        request["user"]["keywords"] = json!("news");
        request["user"]["data"] = json!([{"id": "fpd", "segment": [{"id": "segment"}]}]);
        request["user"]["ext"]["data"] = json!({"owned": "user-fpd"});
        request["user"]["ext"]["context"] = json!({"private": "not-owned-fpd"});
        request["user"]["ext"]["unknown"] = json!("user-ext-unknown");
        request["user"]["unknown"] = json!({"private": "user-unknown"});
        for path in ["/user/eids/0", "/user/ext/eids/0"] {
            let eid = request.pointer_mut(path).expect("fixture has EID");
            eid["inserter"] = json!("inserter");
            eid["matcher"] = json!("matcher");
            eid["mm"] = json!(1_i32);
            eid["ext"] = json!({"private": "eid-ext"});
            eid["unknown"] = json!("eid-unknown");
            eid["uids"][0]["atype"] = json!(1_i32);
            eid["uids"][0]["ext"] = json!({"private": "uid-ext"});
            eid["uids"][0]["unknown"] = json!("uid-unknown");
        }
        for path in ["/user/geo", "/device/geo"] {
            let geo = request.pointer_mut(path).expect("fixture has geo");
            geo["country"] = json!("DEU");
            geo["city"] = json!("Berlin");
            geo["type"] = json!(2_i32);
            geo["zip"] = json!("postcode");
            geo["accuracy"] = json!(1_i32);
            geo["ext"] = json!({"private": "geo-ext"});
            geo["unknown"] = json!("geo-unknown");
        }
        request
    }

    fn assert_wire_field(body: &Value, original: &Value, path: &str, permitted: bool) {
        let before = original
            .pointer(path)
            .unwrap_or_else(|| panic!("fixture lacks {path}"));
        assert_eq!(body.pointer(path), permitted.then_some(before), "{path}");
    }

    fn assert_matrix_context(body: &Value, original: &Value) {
        for key in [
            "ua",
            "dnt",
            "lmt",
            "os",
            "devicetype",
            "make",
            "model",
            "osv",
            "hwv",
            "h",
            "w",
            "ppi",
            "pxratio",
            "js",
            "geofetch",
            "flashver",
            "language",
            "langb",
            "carrier",
            "mccmnc",
            "connectiontype",
        ] {
            assert_wire_field(body, original, &format!("/device/{key}"), true);
        }
        for path in [
            "/device/sua/mobile",
            "/device/sua/source",
            "/device/sua/architecture",
            "/device/sua/bitness",
            "/device/sua/model",
            "/device/sua/browsers/0/brand",
            "/device/sua/browsers/0/version",
            "/device/sua/platform/brand",
            "/device/sua/platform/version",
            "/device/ext/atts",
            "/device/ext/prebid/interstitial/minwidthperc",
            "/device/ext/prebid/interstitial/minheightperc",
            "/user/consent",
            "/user/keywords",
            "/user/data",
            "/user/ext/data",
            "/user/geo/country",
            "/user/geo/city",
            "/user/geo/type",
            "/device/geo/country",
            "/device/geo/city",
            "/device/geo/type",
        ] {
            assert_wire_field(body, original, path, true);
        }
    }

    fn assert_matrix_identifiers(body: &Value, original: &Value, pass_id: bool) {
        for path in [
            "/user/id",
            "/user/buyeruid",
            "/user/yob",
            "/user/gender",
            "/user/customdata",
            "/device/ifa",
            "/device/dpidsha1",
            "/device/dpidmd5",
            "/device/didsha1",
            "/device/didmd5",
            "/device/macsha1",
            "/device/macmd5",
        ] {
            assert_wire_field(body, original, path, pass_id);
        }
        for parent in ["/user/eids", "/user/ext/eids"] {
            if pass_id {
                for key in [
                    "source",
                    "inserter",
                    "matcher",
                    "mm",
                    "uids/0/id",
                    "uids/0/atype",
                ] {
                    assert_wire_field(body, original, &format!("{parent}/0/{key}"), true);
                }
            } else {
                assert_wire_field(body, original, parent, false);
            }
        }
    }

    fn assert_matrix_geo(body: &Value, original: &Value, pass_geo: bool) {
        if pass_geo {
            for path in [
                "/device/ip",
                "/device/ipv6",
                "/user/geo/lat",
                "/user/geo/lon",
                "/device/geo/lat",
                "/device/geo/lon",
            ] {
                assert_wire_field(body, original, path, true);
            }
        } else {
            assert_eq!(body["device"]["ip"], json!("93.184.216.0"));
            assert_eq!(body["device"]["ipv6"], json!("2606:2800:220:ab00::"));
            assert_eq!(body["user"]["geo"]["lat"], json!(-33.87_f64));
            assert_eq!(body["user"]["geo"]["lon"], json!(151.21_f64));
            assert_eq!(body["device"]["geo"]["lat"], json!(52.53_f64));
            assert_eq!(body["device"]["geo"]["lon"], json!(13.41_f64));
        }
        for path in [
            "/user/geo/zip",
            "/user/geo/accuracy",
            "/device/geo/zip",
            "/device/geo/accuracy",
        ] {
            assert_wire_field(body, original, path, pass_geo);
        }
    }

    fn assert_matrix_unknowns(body: &Value, original: &Value, unrestricted: bool) {
        for path in [
            "/user/unknown",
            "/user/ext/unknown",
            "/user/ext/context",
            "/device/unknown",
            "/device/ext/unknown",
            "/device/ext/prebid/unknown",
            "/device/ext/prebid/interstitial/unknown",
            "/device/sua/ext",
            "/device/sua/unknown",
            "/device/sua/browsers/0/ext",
            "/device/sua/browsers/0/unknown",
            "/device/sua/platform/unknown",
            "/user/geo/ext",
            "/user/geo/unknown",
            "/device/geo/ext",
            "/device/geo/unknown",
        ] {
            assert_wire_field(body, original, path, unrestricted);
        }
        for parent in [
            "/user/eids/0",
            "/user/ext/eids/0",
            "/user/eids/0/uids/0",
            "/user/ext/eids/0/uids/0",
        ] {
            for key in ["ext", "unknown"] {
                assert_wire_field(body, original, &format!("{parent}/{key}"), unrestricted);
            }
        }
    }

    #[test]
    fn all_four_resolved_gdpr_gates_reach_the_serialized_outbound_body() {
        let original = matrix_request();
        let request_body = serde_json::to_string(&original).expect("matrix request serializes");
        for (pass_id, pass_geo) in [(false, true), (false, false), (true, false), (true, true)] {
            let (body, permissions) =
                outbound_body(&matrix_config(pass_id, pass_geo), &request_body);
            let expected = if pass_id && pass_geo {
                AuctionPermissions::AllowAll
            } else {
                AuctionPermissions::Partial {
                    allow_bid_request: true,
                    pass_id,
                    pass_geo,
                }
            };
            assert_eq!(
                permissions, expected,
                "resolved GDPR row ({pass_id}, {pass_geo})"
            );
            assert_matrix_context(&body, &original);
            assert_matrix_identifiers(&body, &original, pass_id);
            assert_matrix_geo(&body, &original, pass_geo);
            assert_matrix_unknowns(&body, &original, pass_id && pass_geo);
            if pass_id && pass_geo {
                assert_eq!(
                    body["user"], original["user"],
                    "fully permitted user is untouched"
                );
                assert_eq!(
                    body["device"], original["device"],
                    "fully permitted device is untouched"
                );
            }
        }
    }
}
