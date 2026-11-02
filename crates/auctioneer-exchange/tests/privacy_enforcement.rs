//! `run_auction`'s privacy wiring, driven end to end.
//!
//! Every row here goes through the real driver rather than calling
//! `PrivacyContext` directly: the point of the integration is that the
//! resolved permissions actually gate fan-out, actually scrub the
//! bidder-scoped request, and actually reach the response — a resolver
//! test cannot show any of that.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

mod support;

#[cfg(test)]
mod tests {
    use crate::fake_bidder::FakeBidder;
    use crate::support::response_budgets;
    use async_trait::async_trait;
    use futures::executor::block_on;

    use auctioneer_bidder::{BidderHttpResponse, BidderRegistry};
    use auctioneer_config::types::{
        AccountConfig, BidderInfo, GdprConfig, HostConfig, PurposeEnforcement, PurposeOneTreatment,
        ResourceLimits, SpecialFeatureEnforcement, Tcf2Config, TriState,
    };
    use auctioneer_exchange::scrub::DisclosureAudit;
    use auctioneer_exchange::{
        AccountSource, AuctionContext, AuctionDeadline, AuctionPlan, BidderCall, BidderCallResult,
        BidderFanout, ExchangeFeatures, FanoutInternalError, ResolvedAccount, ResolvedAliases,
        UNRESOLVED_ACCOUNT_ID, run_auction,
    };
    use auctioneer_openrtb::ext::imp::ImpExt;
    use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, Imp};
    use auctioneer_openrtb::types::bid_response::BidResponse;
    use auctioneer_openrtb::types::device::{Device, Geo};
    use auctioneer_openrtb::types::field::JsonField;
    use auctioneer_openrtb::types::regs::Regs;
    use auctioneer_openrtb::types::user::User;
    use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare};

    const IMP_CAP_NOT_UNDER_TEST: PrepareLimits = PrepareLimits {
        max_impressions: 100,
        max_copied_header_bytes: 4_096,
    };
    use auctioneer_privacy::gcl::Gcl;
    use auctioneer_privacy::gvl::GvlStore;
    use auctioneer_runtime::{Clock as _, FakeClock, SeededRng};
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Mutex;
    use std::time::Duration;

    const ACCOUNT_ID: &str = "acct-1";

    /// Public literals — `prepare` strips private / reserved addresses
    /// from the request body, so a masked-IP assertion needs an address
    /// that survives ingest.
    const CLIENT_V4: &str = "8.8.4.4";
    const CLIENT_V6: &str = "2001:4860:4860::8844";
    /// The same addresses under the default account keep-bits (/24, /56).
    const MASKED_V4: &str = "8.8.4.0";
    const MASKED_V6: &str = "2001:4860:4860::";

    /// Records every outbound body so a test can inspect what the
    /// bidder would actually have received after scrubbing.
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
            let mut recorded = self.bodies.lock().expect("recording mutex is not poisoned");
            for call in &calls {
                recorded.push(call.request.body().to_vec());
            }
            Ok(calls
                .iter()
                .map(|_| BidderCallResult {
                    // Raw byte-string fixture: this is a wire body, a `&[u8]`/`Vec<u8>`.
                    // `serde_json::json!` yields a `Value`, so converting would mean
                    // `.to_string().into_bytes()` at the call site for no gain.
                    result: Ok(BidderHttpResponse::new(200, vec![], br#"{"id":"r","seatbid":[{"seat":"fake_bb","bid":[{"id":"b1","impid":"imp-1","price":1.0,"w":300,"h":250}]}],"cur":"USD"}"#.to_vec())),
                    elapsed: Duration::from_millis(5),
                })
                .collect())
        }
    }

    impl RecordingFanout {
        fn sole_body(&self) -> String {
            let recorded = self.bodies.lock().expect("recording mutex is not poisoned");
            assert_eq!(recorded.len(), 1, "exactly one outbound call is expected");
            String::from_utf8(recorded[0].clone()).expect("the outbound body is UTF-8 JSON")
        }

        fn call_count(&self) -> usize {
            self.bodies
                .lock()
                .expect("recording mutex is not poisoned")
                .len()
        }
    }

    fn leak_info(name: &'static str) -> &'static BidderInfo {
        Box::leak(Box::new(BidderInfo {
            name: name.into(),
            endpoint: "https://demo.example.test/openrtb".into(),
            uses_https: true,
            ..BidderInfo::default()
        }))
    }

    fn registry_of(names: &[&'static str]) -> BidderRegistry {
        let mut registry = BidderRegistry::new();
        for name in names {
            registry
                .register(leak_info(name), Box::new(FakeBidder::default()))
                .expect("unique test bidder");
        }
        registry
    }

    fn imp_for(bidders: &[&str]) -> Imp {
        let mut obj = serde_json::Map::new();
        for bidder in bidders {
            obj.insert((*bidder).to_owned(), serde_json::json!({}));
        }
        Imp {
            id: "imp-1".to_owned(),
            banner: Some(Banner {
                w: Some(300_i32),
                h: Some(250_i32),
                ..Banner::default()
            }),
            ext: Some(ImpExt {
                bidder: JsonField::Value(serde_json::Value::Object(obj)),
                ..ImpExt::default()
            }),
            ..Imp::default()
        }
    }

    /// An in-scope request (`regs.gdpr = 1`) carrying the identifiers
    /// and geo the two GDPR scrubs act on.
    fn gdpr_request(bidders: &[&str]) -> BidRequest {
        BidRequest {
            id: "auc-privacy".to_owned(),
            imp: vec![imp_for(bidders)],
            cur: vec!["USD".to_owned()],
            tmax: Some(1000_i64),
            regs: Some(Regs {
                gdpr: Some(1_i32),
                ..Regs::default()
            }),
            user: Some(User {
                id: Some("user-1".to_owned()),
                buyeruid: Some("buyer-1".to_owned()),
                ..User::default()
            }),
            device: Some(Device {
                // Public literals: `prepare` clears private / reserved
                // addresses from the body before the auction sees them.
                ip: Some(CLIENT_V4.to_owned()),
                ipv6: Some(CLIENT_V6.to_owned()),
                geo: Some(Geo {
                    lat: Some(48.858_844_f64),
                    lon: Some(2.294_351_f64),
                    ..Geo::default()
                }),
                ..Device::default()
            }),
            ..BidRequest::default()
        }
    }

    /// Enforcement on, TCF2 channel gate on. `enforce_p2` decides
    /// whether the unusable-consent default row allows the bid request
    /// at all; `enforce_sf1` decides whether it passes geo.
    fn enforcing_host(
        non_standard_publishers: Vec<String>,
        enforce_p2: bool,
        enforce_sf1: bool,
    ) -> HostConfig {
        let mut purposes: BTreeMap<u8, PurposeEnforcement> = BTreeMap::new();
        purposes.insert(
            2_u8,
            PurposeEnforcement {
                enforce_purpose: if enforce_p2 {
                    TriState::Enabled
                } else {
                    TriState::Disabled
                },
                ..PurposeEnforcement::default()
            },
        );
        let mut special_features: BTreeMap<u8, SpecialFeatureEnforcement> = BTreeMap::new();
        special_features.insert(
            1_u8,
            SpecialFeatureEnforcement {
                enforce: if enforce_sf1 {
                    TriState::Enabled
                } else {
                    TriState::Disabled
                },
                vendor_exceptions: vec![],
            },
        );
        HostConfig {
            gdpr: GdprConfig {
                enabled: true,
                default_value: "0".to_owned(),
                host_vendor_id: None,
                tcf2: Tcf2Config {
                    enabled: true,
                    purposes,
                    special_features,
                    purpose_one_treatment: PurposeOneTreatment::default(),
                },
                non_standard_publishers,
            },
            resource_limits: ResourceLimits {
                default_tmax_ms: 1000,
                max_tmax_ms: 5000,
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

    struct Driven {
        response: BidResponse,
        ext: serde_json::Value,
    }

    /// Drive the real `run_auction` with `seatnonbid` reporting on, so
    /// both halves of a privacy drop (the warning and the non-bid row)
    /// are observable on the wire.
    fn drive(
        base_host: &HostConfig,
        request: BidRequest,
        fanout: &RecordingFanout,
        cap: usize,
        advance: Duration,
    ) -> Driven {
        drive_as(ACCOUNT_ID, base_host, request, fanout, cap, advance)
    }

    /// Same as [`drive`] but with the resolved account id under test —
    /// the non-standard-publisher gate keys off exactly that string.
    fn drive_as(
        account_id: &str,
        base_host: &HostConfig,
        request: BidRequest,
        fanout: &RecordingFanout,
        cap: usize,
        advance: Duration,
    ) -> Driven {
        let mut host = base_host.clone();
        host.resource_limits.max_bidders_per_auction = cap;
        let account = AccountConfig::default();
        let resolved = ResolvedAccount {
            id: account_id.to_owned(),
            source: AccountSource::DefaultFallback,
            config: &account,
        };
        let registry = registry_of(&["fake_bb", "fake_ccc"]);
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let rng = SeededRng::new(42);
        let start = clock.now_monotonic();
        clock.advance(advance);
        let overrides: BTreeMap<auctioneer_bidder::BidderName, u32> = BTreeMap::new();
        let features = ExchangeFeatures {
            returnallbidstatus_default: true,
            ..ExchangeFeatures::default()
        };
        let normalized = prepare(request, &RequestHeaders::default(), &IMP_CAP_NOT_UNDER_TEST)
            .expect("fixture prepares");
        let ctx = AuctionContext::with_request_start_ext(
            start,
            &host,
            &resolved,
            &registry,
            fanout,
            &clock,
            &rng,
            GvlStore::baked(),
            Gcl::baked(),
            clock.now(),
            false,
            features,
            &overrides,
        );
        let plan = AuctionPlan::new(
            ResolvedAliases::empty(),
            BTreeSet::new(),
            response_budgets(&registry, &host),
            BTreeMap::new(),
        );
        let mut audit = DisclosureAudit::default();
        let response = block_on(run_auction(&normalized, &ctx, &plan, &mut audit))
            .expect("run_auction answers");
        let ext = response.ext.clone().unwrap_or(serde_json::Value::Null);
        Driven { response, ext }
    }

    #[test]
    fn unlisted_account_is_blocked_by_privacy_with_code_10017_and_nonbid_300() {
        let host = enforcing_host(vec![], true, false);
        let fanout = RecordingFanout::default();
        let driven = drive(
            &host,
            gdpr_request(&["fake_bb"]),
            &fanout,
            50,
            Duration::ZERO,
        );

        assert_eq!(
            fanout.call_count(),
            0,
            "a privacy-denied bidder must never reach fan-out; ext {}",
            driven.ext,
        );
        assert_eq!(
            driven.ext.pointer("/warnings/fake_bb"),
            Some(&serde_json::json!([{
                "code": 10017_u32,
                "message": "bidder blocked by privacy settings: fake_bb",
            }])),
            "the denied bidder needs its code-10017 warning; ext {}",
            driven.ext,
        );
        let seatnonbid = driven
            .ext
            .pointer("/prebid/seatnonbid")
            .expect("seatnonbid is reported");
        assert_eq!(
            seatnonbid,
            &serde_json::json!([{
                "seat": "fake_bb",
                "nonbid": [{ "impid": "imp-1", "statuscode": 300_u32 }],
            }]),
            "the privacy drop needs its statuscode-300 non-bid row",
        );
        assert!(
            driven.response.seatbid.is_empty(),
            "a denied bidder produces no seat row",
        );
    }

    #[test]
    fn non_standard_publisher_account_short_circuits_to_allow_all() {
        // Identical to the row above in every respect EXCEPT that the
        // host lists this account. The resolver would deny in both
        // cases, so only `run_auction`'s own gate can explain the
        // difference.
        let host = enforcing_host(vec![ACCOUNT_ID.to_owned()], true, false);
        let fanout = RecordingFanout::default();
        let driven = drive(
            &host,
            gdpr_request(&["fake_bb"]),
            &fanout,
            50,
            Duration::ZERO,
        );

        assert_eq!(
            fanout.call_count(),
            1,
            "a listed account opts out of enforcement, so the bidder is dispatched; ext {}",
            driven.ext,
        );
        assert_eq!(
            driven.ext.pointer("/warnings/fake_bb"),
            None,
            "an allow-all account emits no privacy block warning; ext {}",
            driven.ext,
        );
        let seats: Vec<&str> = driven
            .response
            .seatbid
            .iter()
            .map(|seat| seat.seat.as_str())
            .collect();
        assert_eq!(seats, vec!["fake_bb"], "the bidder bid normally");

        // Allow-all also means NO scrub: the identifiers survive.
        let body = fanout.sole_body();
        assert!(
            body.contains("buyer-1"),
            "an out-of-scope account must not be scrubbed; body {body}",
        );
    }

    /// The synthesized fallback id is not a publisher identity, so the
    /// host gate must not match it. Otherwise one line of host config
    /// (`non_standard_publishers = ["unknown"]`) would opt every request
    /// that names no publisher out of GDPR enforcement — the exact
    /// failure the reserved id exists to prevent.
    #[test]
    fn unresolved_account_id_never_matches_a_non_standard_publisher_entry() {
        let host = enforcing_host(vec![UNRESOLVED_ACCOUNT_ID.to_owned()], true, false);
        let fanout = RecordingFanout::default();
        let driven = drive_as(
            UNRESOLVED_ACCOUNT_ID,
            &host,
            gdpr_request(&["fake_bb"]),
            &fanout,
            50,
            Duration::ZERO,
        );

        assert_eq!(
            fanout.call_count(),
            0,
            "listing the reserved fallback id must NOT opt publisher-less traffic out of \
             enforcement; ext {}",
            driven.ext,
        );
        assert_eq!(
            driven.ext.pointer("/warnings/fake_bb"),
            Some(&serde_json::json!([{
                "code": 10017_u32,
                "message": "bidder blocked by privacy settings: fake_bb",
            }])),
            "enforcement still applies, so the bidder is blocked as usual; ext {}",
            driven.ext,
        );
    }

    #[test]
    fn non_standard_publisher_still_emits_the_wire_signal_conflict_warning() {
        // The account is opted out of ENFORCEMENT, not out of
        // diagnostics: a request carrying both `regs.gdpr` and
        // `regs.gpp_sid` must still warn.
        let host = enforcing_host(vec![ACCOUNT_ID.to_owned()], true, false);
        let mut request = gdpr_request(&["fake_bb"]);
        request.regs = Some(Regs {
            gdpr: Some(1_i32),
            gpp_sid: Some(vec![2_i32]),
            ..Regs::default()
        });
        let fanout = RecordingFanout::default();
        let driven = drive(&host, request, &fanout, 50, Duration::ZERO);

        let warnings = driven
            .ext
            .pointer("/warnings/prebid")
            .expect("prebid warnings are reported");
        let codes: Vec<u64> = warnings
            .as_array()
            .expect("warnings is an array")
            .iter()
            .filter_map(|entry| entry.pointer("/code").and_then(serde_json::Value::as_u64))
            .collect();
        assert!(
            codes.contains(&10001_u64),
            "the unconditional regs/gpp conflict warning must survive the NSP short-circuit; \
             got {warnings}",
        );
    }

    #[test]
    fn pass_id_false_scrubs_the_outbound_request_but_leaves_geo() {
        // Purpose 2 unenforced -> the default row allows the bid
        // request; `pass_id` is `false` on that row regardless, and
        // special feature 1 is unenforced so geo still passes.
        let host = enforcing_host(vec![], false, false);
        let fanout = RecordingFanout::default();
        let driven = drive(
            &host,
            gdpr_request(&["fake_bb"]),
            &fanout,
            50,
            Duration::ZERO,
        );
        assert_eq!(
            fanout.call_count(),
            1,
            "the bidder is permitted, only its identifiers are denied; ext {}",
            driven.ext,
        );
        let body = fanout.sole_body();
        assert!(
            !body.contains("buyer-1"),
            "PassID = false must strip user.buyeruid from the outbound body; body {body}",
        );
        assert!(
            !body.contains("user-1"),
            "PassID = false must strip user.id from the outbound body; body {body}",
        );
        assert!(
            body.contains(CLIENT_V4) && body.contains(CLIENT_V6),
            "PassGeo is still true, so the client IPs must reach the bidder intact; body {body}",
        );
        assert!(
            body.contains("48.858844"),
            "PassGeo is still true, so geo must keep its precision; body {body}",
        );
    }

    #[test]
    fn denied_identifiers_cannot_reach_bidder_through_unknown_user_fields() {
        let mut request = gdpr_request(&["fake_bb"]);
        request.user = Some(
            serde_json::from_value(serde_json::json!({
                "sha256email": "email-marker",
                "uid2": "uid-marker",
                "unclassified": { "nested_id": "nested-marker" },
                "ext": { "uid2": "extension-marker" }
            }))
            .expect("user wire fixture deserializes"),
        );

        for allow_ids in [false, true] {
            let publishers = if allow_ids {
                vec![ACCOUNT_ID.to_owned()]
            } else {
                vec![]
            };
            let host = enforcing_host(publishers, false, false);
            let fanout = RecordingFanout::default();
            drive(&host, request.clone(), &fanout, 50, Duration::ZERO);
            let body: serde_json::Value =
                serde_json::from_str(&fanout.sole_body()).expect("bidder received a JSON request");
            for field in ["sha256email", "uid2", "unclassified"] {
                assert_eq!(
                    body["user"].get(field).is_some(),
                    allow_ids,
                    "unknown user field {field} must follow the identifier permission",
                );
            }
            assert_eq!(body["user"]["ext"].get("uid2").is_some(), allow_ids);
        }
    }

    fn gdpr_request_with_device_catch_alls() -> BidRequest {
        let mut request = gdpr_request(&["fake_bb"]);
        let sua = serde_json::json!({
            "browsers": [{ "brand": "Browser", "version": ["1"],
                "ext": { "uid2": "browser-extension-id" } }],
            "platform": { "brand": "Android", "version": ["15"], "uid2": "platform-id" },
            "mobile": 1_i32, "source": 1_i32,
            "ext": { "uid2": "sua-extension-id" }, "uid2": "sua-id"
        });
        let context = serde_json::json!({
            "make": "Example", "model": "Handset", "devicetype": 4_i32,
            "osv": "15", "hwv": "rev-2", "h": 800_i32, "w": 400_i32,
            "ppi": 300_i32, "pxratio": 2.0_f64, "js": 1_i32, "geofetch": 0_i32,
            "flashver": "0", "language": "en", "langb": "en-GB",
            "carrier": "Example carrier", "mccmnc": "234-10", "connectiontype": 2_i32,
            "sua": sua
        });
        let mut device_wire = serde_json::json!({
                "ua": "fixture-ua", "os": "Android", "dnt": 1_i32, "lmt": 1_i32,
                "ip": CLIENT_V4, "ipv6": CLIENT_V6, "ifa": "typed-ifa-marker",
                "dpidsha1": "legacy-id", "dpidmd5": "legacy-id",
                "didsha1": "legacy-id", "didmd5": "legacy-id",
                "macsha1": "legacy-id", "macmd5": "legacy-id",
                "uid2": "root-uid-marker", "sha256email": "root-email-marker",
                "eids": [{ "uids": [{ "id": "root-eid-marker" }] }],
                "unclassified": { "nested_id": "root-nested-marker" },
                "vendor_context": { "category": "news" },
                "geo": {
                    "country": "USA", "lat": 48.858_844_f64, "lon": 2.294_351_f64,
                    "zip": "precise-postcode", "ext": { "address": "precise-address" }
                },
                "ext": {
                    "atts": 3_i32, "ifa": "extension-ifa-marker",
                    "prebid": {
                        "interstitial": { "minwidthperc": 80_i64, "minheightperc": 70_i64,
                            "uid2": "interstitial-id-marker" },
                        "uid2": "prebid-device-id-marker"
                    },
                    "dpidsha1": "extension-legacy-id", "dpidmd5": "extension-legacy-id",
                    "didsha1": "extension-legacy-id", "didmd5": "extension-legacy-id",
                    "macsha1": "extension-legacy-id", "macmd5": "extension-legacy-id",
                    "uid2": "extension-uid-marker", "sha256email": "extension-email-marker",
                    "eids": [{ "uids": [{ "id": "extension-eid-marker" }] }],
                    "unclassified": { "nested_id": "extension-nested-marker" },
                    "vendor_context": { "category": "news" }
                }
        });
        device_wire
            .as_object_mut()
            .expect("device object")
            .extend(context.as_object().expect("context object").clone());
        request.device =
            Some(serde_json::from_value(device_wire).expect("device wire fixture deserializes"));
        let device = request.device.as_ref().expect("fixture device");
        let user = request.user.as_mut().expect("fixture user");
        user.geo = device.geo.clone();
        user.consent = Some("consent-signal".to_owned());
        user.data = Some(serde_json::json!([{ "context": "news" }]));
        request
    }

    fn device_permission_inputs(
        request: &BidRequest,
        allow_ids: bool,
        allow_geo: bool,
    ) -> (HostConfig, BidRequest) {
        let publishers = if allow_ids {
            vec![ACCOUNT_ID.to_owned()]
        } else {
            vec![]
        };
        let mut host = enforcing_host(publishers, false, !allow_geo);
        let mut scoped = request.clone();
        if allow_ids && !allow_geo {
            // Usable consent plus a purpose exception permits identifiers,
            // while absent SF1 opt-in still denies geo. An NSP bypass would
            // grant both and cannot exercise these independent permissions.
            host.gdpr.non_standard_publishers.clear();
            host.gdpr
                .tcf2
                .purposes
                .get_mut(&2)
                .unwrap()
                .vendor_exceptions = vec!["fake_bb".to_owned()];
            scoped.user.as_mut().unwrap().consent =
                Some("CP5TLYAP5TLYAEsACBENCuFgAPAAAEAAAAYgAQAAAAAEAIAAAAACAAA.IAAo".to_owned());
        }
        (host, scoped)
    }

    #[test]
    #[expect(
        clippy::cognitive_complexity,
        reason = "four independent permission rows keep all positive-control and denied-field wire assertions together"
    )]
    fn denied_identifiers_cannot_reach_bidder_through_unknown_device_fields() {
        let request = gdpr_request_with_device_catch_alls();
        let original = serde_json::to_value(&request).expect("fixture serializes");
        assert_eq!(
            original["user"]["data"],
            serde_json::json!([{ "context": "news" }])
        );
        for owner in ["device", "user"] {
            assert!(original[owner]["geo"].get("ext").is_some());
        }
        for path in [
            "/device/ext/uid2",
            "/device/ext/prebid/uid2",
            "/device/ext/prebid/interstitial/uid2",
            "/device/sua/ext",
            "/device/sua/browsers/0/ext",
            "/device/sua/platform/uid2",
        ] {
            assert!(original.pointer(path).is_some(), "fixture lacks {path}");
        }

        for (allow_ids, allow_geo) in [(true, true), (true, false), (false, true), (false, false)] {
            let (host, scoped) = device_permission_inputs(&request, allow_ids, allow_geo);
            let expected = serde_json::to_value(&scoped).expect("scoped request serializes");
            let fanout = RecordingFanout::default();
            drive(&host, scoped, &fanout, 50, Duration::ZERO);
            let body: serde_json::Value = serde_json::from_str(&fanout.sole_body())
                .expect("FakeBidder::default() emits JSON");
            if allow_ids && allow_geo {
                assert_eq!(
                    body["device"], original["device"],
                    "AllowAll preserves the device exactly"
                );
                assert_eq!(
                    body["user"], original["user"],
                    "AllowAll preserves the user exactly"
                );
            } else if !allow_ids {
                for field in [
                    "ifa", "dpidsha1", "dpidmd5", "didsha1", "didmd5", "macsha1", "macmd5",
                ] {
                    assert!(original["device"].get(field).is_some());
                    assert!(
                        body["device"].get(field).is_none(),
                        "device.{field} must be absent on the wire"
                    );
                }
                for field in ["id", "buyeruid"] {
                    assert!(original["user"].get(field).is_some());
                    assert!(body["user"].get(field).is_none());
                }
            } else {
                for field in [
                    "ifa", "dpidsha1", "dpidmd5", "didsha1", "didmd5", "macsha1", "macmd5",
                ] {
                    assert!(original["device"].get(field).is_some());
                    assert_eq!(body["device"][field], expected["device"][field]);
                }
                for field in ["id", "buyeruid"] {
                    assert_eq!(body["user"][field], expected["user"][field]);
                    assert!(body["user"].get(field).is_some());
                }
            }
            if !allow_ids || !allow_geo {
                for field in [
                    "uid2",
                    "sha256email",
                    "eids",
                    "unclassified",
                    "vendor_context",
                ] {
                    assert!(original["device"].get(field).is_some());
                    assert!(body["device"].get(field).is_none());
                }
                assert_eq!(
                    body["device"]["ext"],
                    serde_json::json!({"atts": 3_i32, "prebid": {"interstitial": {
                        "minwidthperc": 80_i64, "minheightperc": 70_i64}}})
                );
            }
            assert_standard_device_context(&body, &original, allow_ids && allow_geo);
            assert_eq!(body["user"]["consent"], expected["user"]["consent"]);
            assert_eq!(body["user"]["data"], original["user"]["data"]);
            assert_device_geo_permission(&body, &original, allow_ids, allow_geo);
        }
    }

    #[test]
    fn malformed_user_data_is_removed_under_either_domain_but_fpd_extension_survives() {
        for (allow_ids, allow_geo) in [(true, true), (true, false), (false, true), (false, false)] {
            let mut request = gdpr_request_with_device_catch_alls();
            let user = request.user.as_mut().unwrap();
            user.data = Some(serde_json::json!({"context": "malformed-array"}));
            user.ext = Some(
                serde_json::from_value(serde_json::json!({
                    "data": {"context": "opaque-fpd", "uid2": "fpd-owned-marker"}
                }))
                .unwrap(),
            );
            let (host, scoped) = device_permission_inputs(&request, allow_ids, allow_geo);
            let original = serde_json::to_value(&scoped).unwrap();
            assert_eq!(
                original["user"]["data"],
                serde_json::json!({
                    "context": "malformed-array"
                })
            );
            assert_eq!(
                original["user"]["ext"]["data"],
                serde_json::json!({
                    "context": "opaque-fpd", "uid2": "fpd-owned-marker"
                })
            );
            let fanout = RecordingFanout::default();
            drive(&host, scoped, &fanout, 50, Duration::ZERO);
            let body: serde_json::Value = serde_json::from_str(&fanout.sole_body()).unwrap();
            if allow_ids && allow_geo {
                assert_eq!(body["user"]["data"], original["user"]["data"]);
            } else {
                assert!(body["user"].get("data").is_none());
            }
            assert_eq!(body["user"]["ext"]["data"], original["user"]["ext"]["data"]);
        }
    }

    fn assert_device_geo_permission(
        body: &serde_json::Value,
        original: &serde_json::Value,
        allow_ids: bool,
        allow_geo: bool,
    ) {
        if allow_geo {
            for field in ["ip", "ipv6"] {
                assert_eq!(body["device"][field], original["device"][field]);
            }
            for owner in ["device", "user"] {
                let mut expected_geo = original[owner]["geo"].clone();
                if !allow_ids {
                    assert!(
                        expected_geo
                            .as_object_mut()
                            .unwrap()
                            .remove("ext")
                            .is_some()
                    );
                }
                assert_eq!(body[owner]["geo"], expected_geo);
            }
        } else {
            assert_eq!(body["device"]["ip"], MASKED_V4);
            assert_eq!(body["device"]["ipv6"], MASKED_V6);
            for owner in ["device", "user"] {
                assert_eq!(
                    body[owner]["geo"],
                    serde_json::json!({
                        "country": "USA", "lat": 48.86_f64, "lon": 2.29_f64
                    })
                );
            }
        }
    }

    fn assert_standard_device_context(
        body: &serde_json::Value,
        original: &serde_json::Value,
        unrestricted: bool,
    ) {
        for field in [
            "ua",
            "os",
            "dnt",
            "lmt",
            "make",
            "model",
            "devicetype",
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
            assert!(
                original["device"].get(field).is_some(),
                "loaded context {field}"
            );
            assert_eq!(body["device"][field], original["device"][field]);
        }
        let expected_sua = if unrestricted {
            original["device"]["sua"].clone()
        } else {
            serde_json::json!({
                "browsers": [{ "brand": "Browser", "version": ["1"] }],
                "platform": { "brand": "Android", "version": ["15"] },
                "mobile": 1_i32, "source": 1_i32
            })
        };
        assert_eq!(body["device"]["sua"], expected_sua);
    }

    #[test]
    fn pass_geo_false_masks_the_outbound_ip_and_coarsens_geo() {
        let host = enforcing_host(vec![], false, true);
        let fanout = RecordingFanout::default();
        let driven = drive(
            &host,
            gdpr_request(&["fake_bb"]),
            &fanout,
            50,
            Duration::ZERO,
        );
        assert_eq!(fanout.call_count(), 1, "ext {}", driven.ext);
        let body = fanout.sole_body();
        assert!(
            body.contains(MASKED_V4) && !body.contains(CLIENT_V4),
            "PassGeo = false must mask the client IPv4 to the account keep-bits; body {body}",
        );
        assert!(
            body.contains(MASKED_V6) && !body.contains(CLIENT_V6),
            "PassGeo = false must mask the client IPv6 to the account keep-bits; body {body}",
        );
        assert!(
            body.contains("48.86") && !body.contains("48.858844"),
            "PassGeo = false must coarsen device.geo.lat to 2 decimals; body {body}",
        );
    }

    #[test]
    fn denied_geo_cannot_reach_bidder_through_flattened_fields_or_extensions() {
        let mut request = gdpr_request(&["fake_bb"]);
        let geo = serde_json::from_value(serde_json::json!({
            "country": "USA", "lat": 48.858_844_f64, "lon": 2.294_351_f64,
            "zip": "precise-postcode", "city": "precise-city", "metro": "precise-metro",
            "coordinates": [48.858_844_f64, 2.294_351_f64],
            "ext": { "address": "precise-address" }
        }))
        .expect("geo fixture deserializes");
        request.device.as_mut().expect("fixture device").geo = Some(geo);
        request.user.as_mut().expect("fixture user").geo = request
            .device
            .as_ref()
            .and_then(|device| device.geo.clone());

        let original = serde_json::to_value(&request).expect("fixture serializes");
        for owner in ["device", "user"] {
            for field in ["country", "city", "metro", "zip", "coordinates", "ext"] {
                assert!(original[owner]["geo"].get(field).is_some());
            }
        }
        for (allow_ids, allow_geo) in [(true, true), (true, false), (false, true), (false, false)] {
            let (host, scoped) = device_permission_inputs(&request, allow_ids, allow_geo);
            let fanout = RecordingFanout::default();
            drive(&host, scoped, &fanout, 50, Duration::ZERO);
            let body: serde_json::Value =
                serde_json::from_str(&fanout.sole_body()).expect("bidder received JSON");
            for owner in ["device", "user"] {
                let outbound_geo = &body[owner]["geo"];
                assert_eq!(outbound_geo["country"], "USA");
                assert_eq!(outbound_geo["city"], "precise-city");
                assert_eq!(outbound_geo["metro"], "precise-metro");
                assert_eq!(outbound_geo.get("zip").is_some(), allow_geo);
                for field in ["coordinates", "ext"] {
                    assert_eq!(
                        outbound_geo.get(field).is_some(),
                        allow_ids && allow_geo,
                        "{owner}.geo.{field} is opaque under either restricted domain"
                    );
                }
            }
        }
    }

    #[test]
    fn a_privacy_denied_bidder_does_not_consume_a_max_bidders_slot() {
        // Cap of one over two participating bidders, both denied. The
        // cap runs AFTER the filter, so it sees an empty candidate list
        // and drops nobody: every bidder carries the privacy warning and
        // none carries the cap warning. Running the cap first would
        // stamp one of them with code 10999 instead.
        let host = enforcing_host(vec![], true, false);
        let fanout = RecordingFanout::default();
        let driven = drive(
            &host,
            gdpr_request(&["fake_bb", "fake_ccc"]),
            &fanout,
            1,
            Duration::ZERO,
        );

        for bidder in ["fake_bb", "fake_ccc"] {
            let warnings = driven
                .ext
                .pointer(&format!("/warnings/{bidder}"))
                .unwrap_or_else(|| panic!("{bidder} has a warnings block; ext {}", driven.ext));
            let codes: Vec<u64> = warnings
                .as_array()
                .expect("warnings is an array")
                .iter()
                .filter_map(|entry| entry.pointer("/code").and_then(serde_json::Value::as_u64))
                .collect();
            assert_eq!(
                codes,
                vec![10017_u64],
                "{bidder} must carry the privacy warning ALONE — a cap warning here would mean \
                 the max-bidders cap ran before the privacy filter; ext {}",
                driven.ext,
            );
        }
    }

    #[test]
    fn privacy_bookkeeping_and_warnings_survive_an_expired_tmax() {
        // The budget is gone before fan-out, so the auction takes the
        // short-circuit. It must AUGMENT the bookkeeping the privacy
        // filter already wrote rather than rebuild it, and it must carry
        // the collected privacy warnings rather than an empty list.
        let host = enforcing_host(vec![], true, false);
        let mut request = gdpr_request(&["fake_bb"]);
        request.tmax = Some(1_i64);
        request.regs = Some(Regs {
            gdpr: Some(1_i32),
            gpp_sid: Some(vec![2_i32]),
            ..Regs::default()
        });
        let fanout = RecordingFanout::default();
        let driven = drive(&host, request, &fanout, 50, Duration::from_millis(50));

        assert_eq!(
            driven.ext.pointer("/errors/prebid"),
            Some(&serde_json::json!([{ "code": 12_u32, "message": "tmax expired" }])),
            "the expired budget still reports the global code 12; ext {}",
            driven.ext,
        );
        let warnings = driven
            .ext
            .pointer("/warnings/prebid")
            .expect("privacy warnings reach the expired-budget response too");
        let codes: Vec<u64> = warnings
            .as_array()
            .expect("warnings is an array")
            .iter()
            .filter_map(|entry| entry.pointer("/code").and_then(serde_json::Value::as_u64))
            .collect();
        assert!(
            codes.contains(&10001_u64),
            "the short-circuit must forward the collected privacy warnings; got {warnings}",
        );

        let bidder_warnings = driven
            .ext
            .pointer("/warnings/fake_bb")
            .expect("the privacy-denied bidder keeps its own warning block");
        assert_eq!(
            bidder_warnings,
            &serde_json::json!([{
                "code": 10017_u32,
                "message": "bidder blocked by privacy settings: fake_bb",
            }]),
            "the short-circuit must preserve the privacy bookkeeping, not overwrite it",
        );
        assert_eq!(
            driven.ext.pointer("/prebid/seatnonbid"),
            Some(&serde_json::json!([{
                "seat": "fake_bb",
                "nonbid": [{ "impid": "imp-1", "statuscode": 300_u32 }],
            }])),
            "a denied bidder is not in the fan-out set, so it keeps its 300 rather than \
             collecting a timeout 101; ext {}",
            driven.ext,
        );
    }
}
