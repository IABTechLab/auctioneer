//! Compiled disclosure classification for bidder-owned requests.
//!
//! Decisions are supplied by the caller; this module neither resolves
//! privacy nor reads policy configuration. The auction composes GDPR,
//! activity, CCPA, LMT, and COPPA decisions before invoking these scrubs.
//!
//! Typed slots and flattened values share pure compiled-rule actions. Protected
//! boundaries close unknown subtrees while preserving reviewed context and
//! independently permitted FPD. Unrelated vendor payloads are not privacy-scanned.

#[cfg(test)]
use auctioneer_config_data::types::IpKeepBits;
#[cfg(test)]
use auctioneer_openrtb::types::bid_request::BidRequest;

pub mod audit;
pub mod classified;
pub mod decision;
pub mod fields;
pub mod limits;
pub mod value;
pub use audit::{ChangeKind, DisclosureAudit};
pub use classified::apply_classified;
pub(crate) use classified::apply_classified_with_limits;
pub use limits::construction_costs;

/// Already-resolved decisions for one bidder's owned request.
#[derive(Clone, Copy, Debug)]
pub struct ScrubDecisions {
    pub pass_id: bool,
    pub pass_geo: bool,
    pub transmit_ufpd: bool,
    pub transmit_precise_geo: bool,
    pub transmit_tid: bool,
    pub ccpa: bool,
    pub lmt: bool,
    pub coppa: bool,
}

// Legacy primitive tests resolve only their named decision and exercise the
// production mutator with an explicit scope-local audit.
#[cfg(test)]
const ALLOW: ScrubDecisions = ScrubDecisions {
    pass_id: true,
    pass_geo: true,
    transmit_ufpd: true,
    transmit_precise_geo: true,
    transmit_tid: true,
    ccpa: false,
    lmt: false,
    coppa: false,
};

#[cfg(test)]
fn apply_scrubs(req: &mut BidRequest, decisions: ScrubDecisions, keep_bits: IpKeepBits) {
    let mut audit = DisclosureAudit::default();
    apply_classified(req, decisions, keep_bits, &mut audit)
        .expect("classified test scope is admitted");
}

#[cfg(test)]
fn apply_gdpr_scrubs(req: &mut BidRequest, pass_id: bool, pass_geo: bool, keep_bits: IpKeepBits) {
    apply_scrubs(
        req,
        ScrubDecisions {
            pass_id,
            pass_geo,
            ..ALLOW
        },
        keep_bits,
    );
}

#[cfg(test)]
fn scrub_gdpr_id(req: &mut BidRequest) {
    apply_scrubs(
        req,
        ScrubDecisions {
            pass_id: false,
            ..ALLOW
        },
        IpKeepBits { v4: 24, v6: 56 },
    );
}

#[cfg(test)]
fn scrub_user_fpd(req: &mut BidRequest) {
    apply_scrubs(
        req,
        ScrubDecisions {
            transmit_ufpd: false,
            ..ALLOW
        },
        IpKeepBits { v4: 24, v6: 56 },
    );
}

#[cfg(test)]
fn scrub_ccpa_lmt(req: &mut BidRequest, keep_bits: IpKeepBits) {
    apply_scrubs(
        req,
        ScrubDecisions {
            ccpa: true,
            ..ALLOW
        },
        keep_bits,
    );
}

#[cfg(test)]
fn scrub_coppa(req: &mut BidRequest, keep_bits: IpKeepBits) {
    apply_scrubs(
        req,
        ScrubDecisions {
            coppa: true,
            ..ALLOW
        },
        keep_bits,
    );
}

#[cfg(test)]
fn scrub_tid(req: &mut BidRequest) {
    apply_scrubs(
        req,
        ScrubDecisions {
            transmit_tid: false,
            ..ALLOW
        },
        IpKeepBits { v4: 24, v6: 56 },
    );
}

#[cfg(test)]
fn scrub_geo_and_device_ip(req: &mut BidRequest, keep_bits: IpKeepBits) {
    apply_scrubs(
        req,
        ScrubDecisions {
            pass_geo: false,
            ..ALLOW
        },
        keep_bits,
    );
}

#[cfg(test)]
mod tests {
    use super::value::coordinate as round_half_up_2dp;
    use super::{apply_gdpr_scrubs, scrub_gdpr_id, scrub_geo_and_device_ip};
    use auctioneer_config_data::types::IpKeepBits;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_openrtb::types::device::Device;

    const KEEP: IpKeepBits = IpKeepBits { v4: 24, v6: 56 };

    const V4: &str = "192.168.1.1";
    const V6: &str = "2001:db8:85a3:1234:5678:8a2e:370:7335";

    const LEGACY_DEVICE_ID_KEYS: [&str; 6] = [
        "dpidsha1", "dpidmd5", "didsha1", "didmd5", "macsha1", "macmd5",
    ];

    /// Exact float comparison without tripping the float-comparison
    /// restriction lints: two coordinates are equal iff their bit
    /// patterns are, and every expectation below is a value `f64`
    /// represents unambiguously.
    fn bits(value: f64) -> u64 {
        value.to_bits()
    }

    fn mask_ipv4(raw: &str, keep_bits: u8) -> Option<String> {
        let mut req = BidRequest {
            device: Some(Device {
                ip: Some(raw.to_owned()),
                ..Device::default()
            }),
            ..BidRequest::default()
        };
        scrub_geo_and_device_ip(
            &mut req,
            IpKeepBits {
                v4: keep_bits,
                v6: 56,
            },
        );
        let Some(device) = req.device else {
            panic!("device survives");
        };
        device.ip
    }

    fn mask_ipv6(raw: &str, keep_bits: u8) -> Option<String> {
        let mut req = BidRequest {
            device: Some(Device {
                ipv6: Some(raw.to_owned()),
                ..Device::default()
            }),
            ..BidRequest::default()
        };
        scrub_geo_and_device_ip(
            &mut req,
            IpKeepBits {
                v4: 24,
                v6: keep_bits,
            },
        );
        let Some(device) = req.device else {
            panic!("device survives");
        };
        device.ipv6
    }

    /// A request carrying every field the two scrubs touch.
    fn loaded_request() -> BidRequest {
        let sua = serde_json::json!({
            "browsers": [{ "brand": "Example browser", "version": ["1", "2"],
                "uid2": "browser-id-marker", "ext": { "uid2": "browser-extension-id" } }],
            "platform": { "brand": "Android", "version": ["15"],
                "uid2": "platform-id-marker", "ext": { "uid2": "platform-extension-id" } },
            "mobile": 1_i32, "architecture": "arm", "bitness": "64",
            "model": "Handset", "source": 1_i32,
            "uid2": "sua-id-marker",
            "ext": { "nested_id": "sua-extension-id-marker" }
        });
        let context = serde_json::json!({
            "make": "Example", "model": "Handset", "devicetype": 4_i32,
            "osv": "15", "hwv": "rev-2", "h": 800_i32, "w": 400_i32,
            "ppi": 300_i32, "pxratio": 2.0_f64, "js": 1_i32, "geofetch": 0_i32,
            "flashver": "0", "language": "en", "langb": "en-GB",
            "carrier": "Example carrier", "mccmnc": "234-10", "connectiontype": 2_i32,
            "sua": sua
        });
        let mut wire = serde_json::json!({
            "id": "scrub-1",
            "user": {
                "id": "user-1", "buyeruid": "buyer-1", "yob": 1979_i32, "gender": "M",
                "consent": "consent-signal", "keywords": "news",
                "data": [{ "context": "news" }],
                "eids": [{ "source": "example.test" }],
                "geo": {
                    "country": "GBR", "lat": 51.507_351_f64, "lon": -0.127_758_f64,
                    "zip": "user-postcode", "ext": { "address": "user-address" }
                },
                "ext": {
                    "eids": [{ "source": "legacy.test" }], "data": { "context": "news" }
                }
            },
            "device": {
                "ua": "fixture-ua", "os": "Android", "dnt": 1_i32, "lmt": 1_i32,
                "ip": V4, "ipv6": V6, "ifa": "ifa-1",
                "dpidsha1": "legacy-id", "dpidmd5": "legacy-id",
                "didsha1": "legacy-id", "didmd5": "legacy-id",
                "macsha1": "legacy-id", "macmd5": "legacy-id",
                "uid2": "root-uid-marker", "sha256email": "root-email-marker",
                "eids": [{ "uids": [{ "id": "root-eid-marker" }] }],
                "unclassified": { "nested_id": "root-nested-marker" },
                "vendor_context": { "category": "news" },
                "geo": {
                    "country": "USA", "lat": 48.858_844_f64, "lon": 2.294_351_f64,
                    "zip": "device-postcode", "ext": { "address": "device-address" }
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
            }
        });
        wire["device"]
            .as_object_mut()
            .expect("device object")
            .extend(context.as_object().expect("context object").clone());
        serde_json::from_value(wire)
            .expect("the loaded request deserializes including both device catch-all maps")
    }

    fn json(req: &BidRequest) -> String {
        serde_json::to_string(req).expect("the fixture request serializes")
    }

    #[test]
    fn pass_id_false_clears_every_identifier() {
        let mut req = loaded_request();
        scrub_gdpr_id(&mut req);

        let user = req.user.as_ref().expect("user survives the scrub");
        assert_eq!(user.id, None, "user.id must be cleared");
        assert_eq!(user.buyeruid, None, "user.buyeruid must be cleared");
        assert_eq!(user.yob, None, "user.yob must be cleared");
        assert_eq!(user.gender, None, "user.gender must be cleared");
        assert!(
            user.eids.is_empty(),
            "the native 2.6 user.eids location must be emptied",
        );
        assert_eq!(
            user.ext.as_ref().and_then(|ext| ext.eids.as_ref()),
            None,
            "the legacy user.ext.eids location must be cleared too",
        );

        let device = req.device.as_ref().expect("device survives the scrub");
        assert_eq!(device.ifa, None, "device.ifa must be cleared");
        for key in LEGACY_DEVICE_ID_KEYS {
            assert!(
                !device.other.contains_key(key),
                "flattened legacy device id {key} must be removed",
            );
        }
    }

    #[test]
    fn denied_identifiers_clear_unclassified_user_fields_after_deserialization() {
        let mut req: BidRequest = serde_json::from_value(serde_json::json!({
            "id": "opaque-user-fields",
            "imp": [],
            "user": {
                "sha256email": "email-marker",
                "uid2": "uid-marker",
                "unclassified": { "nested_id": "nested-marker" },
                "ext": { "uid2": "extension-marker", "data": { "context": "news" } },
                "consent": "consent-signal"
            }
        }))
        .expect("unknown user fields deserialize into the flattened map");
        let allowed = req.clone();
        apply_gdpr_scrubs(&mut req, false, true, KEEP);

        let denied_wire = serde_json::to_value(&req).expect("scrubbed request serializes");
        for field in ["sha256email", "uid2", "unclassified"] {
            assert!(denied_wire["user"].get(field).is_none());
        }
        assert_eq!(denied_wire["user"]["consent"], "consent-signal");
        assert!(denied_wire["user"]["ext"].get("uid2").is_none());
        assert_eq!(denied_wire["user"]["ext"]["data"]["context"], "news");

        let mut allowed_req = allowed.clone();
        apply_gdpr_scrubs(&mut allowed_req, true, true, KEEP);
        assert_eq!(json(&allowed_req), json(&allowed));
    }

    fn assert_geo_permission(
        wire: &serde_json::Value,
        original: &serde_json::Value,
        pass_id: bool,
        pass_geo: bool,
    ) {
        if pass_geo {
            for field in ["ip", "ipv6"] {
                assert_eq!(wire["device"][field], original["device"][field]);
            }
            for owner in ["device", "user"] {
                let mut expected = original[owner]["geo"].clone();
                if !pass_id {
                    expected.as_object_mut().expect("geo object").remove("ext");
                }
                assert_eq!(wire[owner]["geo"], expected);
            }
        } else {
            assert_eq!(wire["device"]["ip"], "192.168.1.0");
            assert_eq!(wire["device"]["ipv6"], "2001:db8:85a3:1200::");
            assert_eq!(
                wire["device"]["geo"],
                serde_json::json!({
                    "country": "USA", "lat": 48.86_f64, "lon": 2.29_f64
                })
            );
            assert_eq!(
                wire["user"]["geo"],
                serde_json::json!({
                    "country": "GBR", "lat": 51.51_f64, "lon": -0.13_f64
                })
            );
        }
    }

    fn expected_structured_agent(
        original: &serde_json::Value,
        restricted: bool,
    ) -> serde_json::Value {
        let mut expected = original.clone();
        if restricted {
            let object = expected.as_object_mut().expect("structured user agent");
            object.remove("uid2");
            object.remove("ext");
            for path in ["/browsers/0", "/platform"] {
                let fields = expected
                    .pointer_mut(path)
                    .and_then(serde_json::Value::as_object_mut)
                    .expect("brand object");
                fields.remove("uid2");
                fields.remove("ext");
            }
        }
        expected
    }

    fn assert_reviewed_device_context(wire: &serde_json::Value, original_wire: &serde_json::Value) {
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
                original_wire["device"].get(field).is_some(),
                "loaded context {field}"
            );
            assert_eq!(wire["device"][field], original_wire["device"][field]);
        }
    }

    #[test]
    fn denied_identifiers_clear_unclassified_device_fields_after_deserialization() {
        let original = loaded_request();
        let original_wire = serde_json::to_value(&original).expect("fixture serializes");
        let original_device = original.device.as_ref().expect("fixture device");
        let original_ext = original_device
            .ext
            .as_ref()
            .expect("fixture device extension");

        for (pass_id, pass_geo) in [(true, true), (true, false), (false, true), (false, false)] {
            let mut req = original.clone();
            apply_gdpr_scrubs(&mut req, pass_id, pass_geo, KEEP);
            let wire = serde_json::to_value(&req).expect("scrubbed request serializes");
            let device = req.device.as_ref().expect("device survives");
            let ext = device
                .ext
                .as_ref()
                .expect("typed device extension survives");
            if pass_id {
                assert_eq!(device.ifa, original_device.ifa);
            } else {
                assert_eq!(device.ifa, None);
                assert!(
                    device.other.is_empty(),
                    "all unclassified device fields are denied"
                );
            }
            if pass_id && pass_geo {
                assert_eq!(device.other, original_device.other);
                assert_eq!(ext.other, original_ext.other);
            } else {
                if pass_id {
                    let mut expected_ids = serde_json::Map::new();
                    for key in LEGACY_DEVICE_ID_KEYS {
                        expected_ids.insert(
                            key.to_owned(),
                            original_device
                                .other
                                .get(key)
                                .expect("loaded ID hash")
                                .clone(),
                        );
                    }
                    assert_eq!(device.other, expected_ids, "geo-only retains reviewed IDs");
                }
                assert!(
                    ext.other.is_empty(),
                    "unknown extension fields close under either gate"
                );
                assert_eq!(
                    wire["device"]["ext"],
                    serde_json::json!({ "atts": 3_i32,
                        "prebid": { "interstitial": { "minwidthperc": 80_i64, "minheightperc": 70_i64 } }
                    })
                );
            }
            assert_eq!(ext.atts, Some(3_i32));
            assert_reviewed_device_context(&wire, &original_wire);
            assert_eq!(
                wire["device"]["sua"],
                expected_structured_agent(&original_wire["device"]["sua"], !pass_id || !pass_geo)
            );
            for field in ["consent", "keywords", "data"] {
                assert_eq!(wire["user"][field], original_wire["user"][field]);
            }
            assert_eq!(
                wire["user"]["ext"]["data"],
                original_wire["user"]["ext"]["data"]
            );
            assert_geo_permission(&wire, &original_wire, pass_id, pass_geo);
            if pass_id && pass_geo {
                assert_eq!(wire, original_wire, "AllowAll is an exact control");
            }
            let once = json(&req);
            apply_gdpr_scrubs(&mut req, pass_id, pass_geo, KEEP);
            assert_eq!(
                json(&req),
                once,
                "every permission combination is idempotent"
            );
        }
    }

    #[test]
    fn pass_id_false_preserves_permitted_coordinates_and_ip() {
        let mut req = loaded_request();
        scrub_gdpr_id(&mut req);

        let device = req.device.as_ref().expect("device survives");
        assert_eq!(
            device.ip.as_deref(),
            Some(V4),
            "the id scrub must not mask the client IP",
        );
        assert_eq!(device.ipv6.as_deref(), Some(V6));
        let geo = device.geo.as_ref().expect("device geo survives");
        assert_eq!(
            geo.lat.map(bits),
            Some(bits(48.858_844_f64)),
            "the id scrub must not coarsen geo",
        );
        let user_geo = req
            .user
            .as_ref()
            .and_then(|user| user.geo.as_ref())
            .expect("user geo survives");
        assert_eq!(user_geo.lon.map(bits), Some(bits(-0.127_758_f64)));
    }

    #[test]
    fn scrub_gdpr_id_is_idempotent() {
        let mut once = loaded_request();
        scrub_gdpr_id(&mut once);
        let mut twice = loaded_request();
        scrub_gdpr_id(&mut twice);
        scrub_gdpr_id(&mut twice);
        assert_eq!(
            json(&once),
            json(&twice),
            "a second application must change nothing",
        );
    }

    #[test]
    fn pass_geo_false_rounds_device_and_user_geo() {
        let mut req = loaded_request();
        scrub_geo_and_device_ip(&mut req, KEEP);

        let device_geo = req
            .device
            .as_ref()
            .and_then(|device| device.geo.as_ref())
            .expect("device geo survives");
        assert_eq!(
            device_geo.lat.map(bits),
            Some(bits(48.86_f64)),
            "device latitude must be coarsened to 2 decimals",
        );
        assert_eq!(device_geo.lon.map(bits), Some(bits(2.29_f64)));

        let user_geo = req
            .user
            .as_ref()
            .and_then(|user| user.geo.as_ref())
            .expect("user geo survives");
        assert_eq!(
            user_geo.lat.map(bits),
            Some(bits(51.51_f64)),
            "user geo carries the same precision and must be coarsened too",
        );
        assert_eq!(user_geo.lon.map(bits), Some(bits(-0.13_f64)));
    }

    #[test]
    fn pass_geo_false_masks_ipv4_and_ipv6() {
        let mut req = loaded_request();
        scrub_geo_and_device_ip(&mut req, KEEP);
        let device = req.device.as_ref().expect("device survives");
        assert_eq!(
            device.ip.as_deref(),
            Some("192.168.1.0"),
            "the v4 address must be masked to the configured keep-bits",
        );
        assert_eq!(
            device.ipv6.as_deref(),
            Some("2001:db8:85a3:1200::"),
            "the v6 address must be masked to the configured keep-bits",
        );
    }

    #[test]
    fn denied_geo_removes_unclassified_fields_and_extensions() {
        let mut req: BidRequest = serde_json::from_value(serde_json::json!({
            "id": "opaque-geo-fields",
            "imp": [],
            "device": { "geo": {
                "country": "USA", "lat": 48.858_844_f64, "lon": 2.294_351_f64,
                "zip": "precise-postcode", "city": "known-city", "metro": "known-market",
                "coordinates": [48.858_844_f64, 2.294_351_f64],
                "ext": { "address": "precise-address" }
            } },
            "user": { "geo": {
                "country": "USA", "lat": 48.858_844_f64, "lon": 2.294_351_f64,
                "zip": "precise-postcode", "city": "known-city", "metro": "known-market",
                "coordinates": [48.858_844_f64, 2.294_351_f64],
                "ext": { "address": "precise-address" }
            } }
        }))
        .expect("geo catch-all fixture deserializes");
        let before = json(&req);
        apply_gdpr_scrubs(&mut req, true, true, KEEP);
        assert_eq!(json(&req), before, "allowed geo is unchanged");
        apply_gdpr_scrubs(&mut req, true, false, KEEP);
        let wire = serde_json::to_value(&req).expect("scrubbed geo serializes");
        for owner in ["device", "user"] {
            let geo = &wire[owner]["geo"];
            assert_eq!(geo["country"], "USA");
            assert_eq!(geo["lat"], serde_json::json!(48.86_f64));
            assert_eq!(geo["lon"], serde_json::json!(2.29_f64));
            assert_eq!(geo["city"], "known-city");
            assert_eq!(geo["metro"], "known-market");
            for field in ["zip", "coordinates", "ext"] {
                assert!(
                    geo.get(field).is_none(),
                    "{owner}.geo.{field} must not bypass PassGeo"
                );
            }
        }
        let once = json(&req);
        scrub_geo_and_device_ip(&mut req, KEEP);
        assert_eq!(json(&req), once);
    }

    #[test]
    fn geo_rounding_is_half_up_including_negatives() {
        for (input, expected) in [
            (1.238_f64, 1.24_f64),
            (1.232_f64, 1.23_f64),
            (-1.238_f64, -1.24_f64),
            (-1.232_f64, -1.23_f64),
            (1.125_f64, 1.13_f64),
            (-1.125_f64, -1.12_f64),
            (-90.0_f64, -90.0_f64),
            (90.0_f64, 90.0_f64),
            (-180.0_f64, -180.0_f64),
            (180.0_f64, 180.0_f64),
        ] {
            assert_eq!(
                round_half_up_2dp(input).map(bits),
                Some(bits(expected)),
                "half-up rounding of {input} must yield {expected}",
            );
        }
    }

    #[test]
    fn geo_rounding_rejects_non_finite_input_and_finite_overflow() {
        for input in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::MAX,
            -f64::MAX,
        ] {
            assert_eq!(
                round_half_up_2dp(input),
                None,
                "invalid transform of {input}"
            );
        }
    }

    #[test]
    fn ip_mask_keep_bits_boundaries() {
        for (keep, expected) in [
            (0_u8, "0.0.0.0"),
            (1_u8, "128.0.0.0"),
            (24_u8, "192.168.1.0"),
            (31_u8, "192.168.1.0"),
            (32_u8, "192.168.1.1"),
        ] {
            assert_eq!(
                mask_ipv4(V4, keep).as_deref(),
                Some(expected),
                "v4 /{keep} must mask to {expected}",
            );
        }
        for (keep, expected) in [
            (0_u8, "::"),
            (1_u8, "::"),
            (56_u8, "2001:db8:85a3:1200::"),
            (127_u8, "2001:db8:85a3:1234:5678:8a2e:370:7334"),
            (128_u8, V6),
        ] {
            assert_eq!(
                mask_ipv6(V6, keep).as_deref(),
                Some(expected),
                "v6 /{keep} must mask to {expected}",
            );
        }
    }

    #[test]
    fn malformed_ip_is_dropped_not_passed_through() {
        assert_eq!(mask_ipv4("not-an-ip", 24_u8), None);
        assert_eq!(mask_ipv6("still-not-an-ip", 56_u8), None);

        let mut req = BidRequest {
            id: "scrub-bad-ip".to_owned(),
            device: Some(Device {
                ip: Some("999.1.2.3".to_owned()),
                ipv6: Some("zzzz::1".to_owned()),
                ..Device::default()
            }),
            ..BidRequest::default()
        };
        scrub_geo_and_device_ip(&mut req, KEEP);
        let device = req.device.as_ref().expect("device survives");
        assert_eq!(
            device.ip, None,
            "an unparseable v4 address must be dropped, never passed through unmasked",
        );
        assert_eq!(
            device.ipv6, None,
            "an unparseable v6 address must be dropped, never passed through unmasked",
        );
    }

    #[test]
    fn scrub_geo_and_device_ip_is_idempotent() {
        let mut once = loaded_request();
        scrub_geo_and_device_ip(&mut once, KEEP);
        let mut twice = loaded_request();
        scrub_geo_and_device_ip(&mut twice, KEEP);
        scrub_geo_and_device_ip(&mut twice, KEEP);
        assert_eq!(
            json(&once),
            json(&twice),
            "masking an already-masked address must not narrow it further",
        );
    }

    #[test]
    fn allow_all_scrubs_nothing() {
        let before = loaded_request();
        let mut after = loaded_request();
        apply_gdpr_scrubs(&mut after, true, true, KEEP);
        assert_eq!(
            json(&before),
            json(&after),
            "a bidder allowed everything must see a byte-identical request",
        );
    }

    #[test]
    fn both_false_applies_both_scrubs() {
        let mut both = loaded_request();
        apply_gdpr_scrubs(&mut both, false, false, KEEP);

        let mut sequential = loaded_request();
        scrub_gdpr_id(&mut sequential);
        scrub_geo_and_device_ip(&mut sequential, KEEP);
        assert_eq!(
            json(&both),
            json(&sequential),
            "both gates denied must apply both scrubs",
        );

        let device = both.device.as_ref().expect("device survives");
        assert_eq!(device.ifa, None, "the id scrub ran");
        assert_eq!(
            device.ip.as_deref(),
            Some("192.168.1.0"),
            "the geo/ip scrub ran",
        );
    }
}

/// Independent classified-policy expectations, separate from the old privacy enforcement oracle.
#[cfg(test)]
mod classified_tests {
    use super::value::{AddressBuffer, Frames, Mutation, TransformScratch};
    use super::{ChangeKind, DisclosureAudit, ScrubDecisions, apply_classified};
    use auctioneer_config_data::disclosure::SchemaId;
    use auctioneer_config_data::types::IpKeepBits;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_openrtb::types::device::{Device, Geo};
    use auctioneer_openrtb::types::user::User;
    use serde::Deserializer;
    use serde::de::{DeserializeSeed, Error as DeError, MapAccess, SeqAccess, Visitor};
    use serde_json::value::RawValue;
    use serde_json::{Value, json};
    use std::alloc::{GlobalAlloc, Layout};
    use std::cell::Cell;
    use std::fmt;
    use std::mem::size_of;
    use std::ptr::null_mut;

    use auctioneer_runtime::test_support::alloc_track;

    #[global_allocator]
    static ALLOCATOR: AllocationCalls = AllocationCalls;

    thread_local! {
        static ALLOCATION_CALLS: Cell<u64> = const { Cell::new(0) };
        static ALLOCATION_REQUEST_BYTES: Cell<u64> = const { Cell::new(0) };
        static FAIL_NEXT_ALLOCATION: Cell<bool> = const { Cell::new(false) };
    }

    struct AllocationCalls;

    fn allocation_call(bytes: usize) -> bool {
        let _result = ALLOCATION_CALLS.try_with(|calls| calls.set(calls.get().saturating_add(1)));
        let _bytes_result = ALLOCATION_REQUEST_BYTES.try_with(|requested| {
            requested.set(
                requested
                    .get()
                    .saturating_add(u64::try_from(bytes).unwrap_or(u64::MAX)),
            );
        });
        FAIL_NEXT_ALLOCATION
            .try_with(|fail| fail.replace(false))
            .unwrap_or(false)
    }

    #[expect(
        unsafe_code,
        reason = "test-only allocator delegates unchanged to the existing tracker"
    )]
    // SAFETY: pointer/layout operations delegate unchanged to the existing tracker;
    // injected allocation failures return null without modifying existing ownership.
    unsafe impl GlobalAlloc for AllocationCalls {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            if allocation_call(layout.size()) {
                return null_mut();
            }
            // SAFETY: the caller's layout is forwarded unchanged.
            unsafe { alloc_track::CountingAllocator.alloc(layout) }
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            if allocation_call(layout.size()) {
                return null_mut();
            }
            // SAFETY: the caller's layout is forwarded unchanged.
            unsafe { alloc_track::CountingAllocator.alloc_zeroed(layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            if allocation_call(new_size) {
                return null_mut();
            }
            // SAFETY: the valid allocation and replacement size are forwarded unchanged.
            unsafe { alloc_track::CountingAllocator.realloc(ptr, layout, new_size) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: the pointer and original layout are forwarded unchanged.
            unsafe { alloc_track::CountingAllocator.dealloc(ptr, layout) }
        }
    }

    const KEEP: IpKeepBits = IpKeepBits { v4: 24, v6: 56 };
    const ALLOW: ScrubDecisions = ScrubDecisions {
        pass_id: true,
        pass_geo: true,
        transmit_ufpd: true,
        transmit_precise_geo: true,
        transmit_tid: true,
        ccpa: false,
        lmt: false,
        coppa: false,
    };

    fn geo() -> Value {
        json!({"country": "GBR", "city": "London", "metro": "market",
            "region": "LND", "regionfips104": "UKH9", "utcoffset": 60_i32,
            "type": 2_i32, "ipservice": 3_i32, "lastfix": 12_i32,
            "lat": 51.5074_f64, "lon": -0.1278_f64, "zip": "postcode", "accuracy": 1_i32,
            "ext": {"uid2": "secret"}, "unknown": [{"uid2": "secret"}]})
    }

    fn eid() -> Value {
        json!({"source": "source", "inserter": "inserter", "matcher": "matcher", "mm": 1_i32,
            "uids": [{"id": "uid", "atype": 1_i32, "ext": {"uid2": "secret"}, "unknown": "secret"}],
            "ext": {"uid2": "secret"}, "unknown": "secret"})
    }

    fn input() -> Value {
        let source = json!({"fd": 1_i32, "tid": "transaction", "pchain": "chain",
            "ext": {"tid": "not-owned", "unknown": {"uid2": "outside"}}});
        let user = json!({"id": "user", "buyeruid": "buyer", "customdata": "custom",
                "yob": 1990_i32, "gender": "F", "consent": "consent", "eids": [eid()],
                "keywords": "news", "kwarray": ["news"], "data": [{"uid2": "owned"}],
                "geo": geo(), "ext": {"eids": [eid()], "data": {"uid2": "owned"},
                    "unknown": "secret"}, "unknown": [{"uid2": "secret"}]});
        let sua = json!({"mobile": 1_i32, "source": 2_i32, "architecture": "arm", "bitness": "64",
            "model": "model", "browsers": [{}, {"brand": "browser", "version": ["1"],
                "ext": {"uid2": "secret"}, "unknown": "secret"}],
            "platform": {"brand": "platform", "version": ["1"], "unknown": "secret"},
            "ext": {"uid2": "secret"}, "unknown": "secret"});
        let device_ext = json!({"atts": 2_i32, "prebid": {"interstitial": {"minwidthperc": 70_i32,
            "minheightperc": 60_i32, "unknown": "secret"}, "unknown": "secret"}, "unknown": "secret"});
        let mut device = json!({"ifa": "ifa", "didmd5": "id", "didsha1": "id", "dpidmd5": "id",
                "dpidsha1": "id", "macmd5": "id", "macsha1": "id",
                "ip": "192.168.1.33", "ipv6": "2001:db8:85a3:1234:5678:8a2e:370:7335",
                "geo": geo(), "sua": sua, "ext": device_ext, "unknown": [{"uid2": "secret"}]});
        let context = json!({"ua": "agent", "os": "OS", "osv": "1", "make": "maker",
                "model": "model", "hwv": "hw", "language": "en", "langb": "en-GB",
                "flashver": "1", "carrier": "carrier", "mccmnc": "123456", "devicetype": 1_i32,
                "h": 800_i32, "w": 600_i32, "ppi": 120_i32, "js": 1_i32, "geofetch": 1_i32, "connectiontype": 2_i32,
                "pxratio": 2_i32, "dnt": 1_i32, "lmt": 0_i32});
        device
            .as_object_mut()
            .unwrap()
            .extend(context.as_object().unwrap().clone());
        json!({"id": "request", "source": source, "user": user, "device": device,
            "imp": [{"id": "imp", "ext": {"tid": "transaction", "bidder": {"tid": "param"}}}],
            "ext": {"tid": "not-owned"}, "site": {"ext": {"data": {"uid2": "outside"}}}})
    }

    fn loaded() -> BidRequest {
        serde_json::from_value(input()).unwrap()
    }

    fn wire(req: &BidRequest) -> Value {
        serde_json::from_slice(&serde_json::to_vec(req).unwrap()).unwrap()
    }

    fn apply(req: &mut BidRequest, decisions: ScrubDecisions) -> DisclosureAudit {
        let mut audit = DisclosureAudit::default();
        apply_classified(req, decisions, KEEP, &mut audit).unwrap();
        audit
    }

    fn decisions(mask: u8) -> ScrubDecisions {
        ScrubDecisions {
            pass_id: mask & 1 == 0,
            pass_geo: mask & 2 == 0,
            transmit_ufpd: mask & 4 == 0,
            transmit_precise_geo: mask & 8 == 0,
            transmit_tid: mask & 16 == 0,
            ccpa: mask & 32 != 0,
            lmt: mask & 64 != 0,
            coppa: mask & 128 != 0,
        }
    }

    fn remove(value: &mut Value, path: &str, fields: &[&str]) {
        let object = value.pointer_mut(path).unwrap().as_object_mut().unwrap();
        for field in fields {
            assert!(
                object.remove(*field).is_some(),
                "before-state presence: {path}/{field}"
            );
        }
    }

    // Literal field lists and coarse values are independent of generated rules/actions.
    fn expected(mask: u8) -> Value {
        let mut result = input();
        let deny_id = mask & 0b1110_0101 != 0;
        let deny_geo = mask & 0b1110_1010 != 0;
        if deny_id || deny_geo {
            for parent in [
                "/user",
                "/device",
                "/user/ext",
                "/device/ext",
                "/device/ext/prebid",
                "/device/ext/prebid/interstitial",
                "/device/sua",
                "/device/sua/browsers/1",
                "/device/sua/platform",
            ] {
                remove(&mut result, parent, &["unknown"]);
            }
            for parent in [
                "/device/sua",
                "/device/sua/browsers/1",
                "/user/geo",
                "/device/geo",
            ] {
                remove(&mut result, parent, &["ext"]);
            }
            for parent in ["/user/geo", "/device/geo"] {
                remove(&mut result, parent, &["unknown"]);
            }
            if !deny_id {
                for parent in [
                    "/user/eids/0",
                    "/user/ext/eids/0",
                    "/user/eids/0/uids/0",
                    "/user/ext/eids/0/uids/0",
                ] {
                    remove(&mut result, parent, &["ext", "unknown"]);
                }
            }
        }
        if deny_id {
            remove(
                &mut result,
                "/user",
                &["id", "buyeruid", "customdata", "yob", "gender", "eids"],
            );
            remove(&mut result, "/user/ext", &["eids"]);
            remove(
                &mut result,
                "/device",
                &[
                    "ifa", "didmd5", "didsha1", "dpidmd5", "dpidsha1", "macmd5", "macsha1",
                ],
            );
        }
        if mask & (4 | 128) != 0 {
            remove(&mut result, "/user", &["keywords", "kwarray", "data"]);
            remove(&mut result, "/user/ext", &["data"]);
        }
        if deny_geo {
            for parent in ["user", "device"] {
                remove(&mut result, &format!("/{parent}/geo"), &["zip", "accuracy"]);
                result[parent]["geo"]["lat"] = json!(51.51_f64);
                result[parent]["geo"]["lon"] = json!(-0.13_f64);
                if mask & 128 != 0 {
                    result[parent]["geo"] = json!({});
                }
            }
            result["device"]["ip"] = json!("192.168.1.0");
            result["device"]["ipv6"] = json!("2001:db8:85a3:1200::");
        }
        if mask & 16 != 0 {
            remove(&mut result, "/source", &["tid"]);
            remove(&mut result, "/imp/0/ext", &["tid"]);
        }
        result
    }

    #[test]
    fn classified_all_256_composed_decisions_match_independent_wire_and_zero_repeat_changes() {
        let original = loaded();
        let sibling = original.clone();
        assert_eq!(wire(&original), input());
        for mask in 0_u8..=u8::MAX {
            let mut req = original.clone();
            let audit = apply(&mut req, decisions(mask));
            assert_eq!(wire(&req), expected(mask), "mask {mask}");
            assert_eq!(audit.attempted_scopes, 1);
            assert_eq!(audit.failed_scopes, 0);
            assert_eq!(
                apply(&mut req, decisions(mask)).counts(),
                [0; 8],
                "repeat mask {mask}"
            );
        }
        assert_eq!(wire(&original), input());
        assert_eq!(wire(&sibling), input());
    }

    #[test]
    fn classified_all_scoped_fields_in_flattened_maps_use_the_same_256_wire_oracle() {
        for mask in 0_u8..=u8::MAX {
            let fixture = input();
            let mut req = loaded();
            req.user = Some(User {
                other: fixture["user"].as_object().unwrap().clone(),
                ..User::default()
            });
            req.device = Some(Device {
                other: fixture["device"].as_object().unwrap().clone(),
                ..Device::default()
            });
            assert_eq!(wire(&req), fixture, "flattened before-state mask {mask}");
            apply(&mut req, decisions(mask));
            assert_eq!(wire(&req), expected(mask), "flattened mask {mask}");
            assert_eq!(apply(&mut req, decisions(mask)).counts(), [0; 8]);
        }
    }

    #[test]
    fn classified_shape_checks_close_context_and_descendants_under_either_domain() {
        for mask in [1, 2, 3] {
            let mut req = loaded();
            req.user
                .as_mut()
                .unwrap()
                .geo
                .as_mut()
                .unwrap()
                .other
                .insert("city".into(), json!({"uid2": "secret"}));
            req.device
                .as_mut()
                .unwrap()
                .other
                .insert("ua".into(), json!({"uid2": "secret"}));
            req.device.as_mut().unwrap().other.insert(
                "sua".into(),
                json!({
                "browsers": [{"version": ["1", {"uid2": "secret"}], "unknown": "secret"}],
                "platform": {"brand": {"uid2": "secret"}}, "architecture": ["secret"]}),
            );
            req.device.as_mut().unwrap().other.insert(
                "ext".into(),
                json!({"prebid": {
                "interstitial": {"minwidthperc": {"uid2": "secret"}, "minheightperc": 60_i32,
                    "unknown": [{"uid2": "secret"}]}, "unknown": "secret"}}),
            );
            assert!(
                req.user
                    .as_ref()
                    .unwrap()
                    .geo
                    .as_ref()
                    .unwrap()
                    .other
                    .contains_key("city")
            );
            let audit = apply(&mut req, decisions(mask));
            assert!(
                !req.user
                    .as_ref()
                    .unwrap()
                    .geo
                    .as_ref()
                    .unwrap()
                    .other
                    .contains_key("city")
            );
            let shadows = &req.device.as_ref().unwrap().other;
            assert!(!shadows.contains_key("ua"));
            assert_eq!(shadows["sua"], json!({"browsers": [{}], "platform": {}}));
            assert_eq!(
                shadows["ext"],
                json!({"prebid": {"interstitial": {"minheightperc": 60_i32}}})
            );
            // city, ua, browser version, platform brand, architecture, and minwidthperc.
            assert_eq!(audit.counts()[6], 6);
        }
    }

    #[test]
    fn classified_coppa_removes_allowed_user_fpd_with_owned_audit_counts() {
        let mut req = loaded();
        let policy = decisions(128);
        assert!(policy.transmit_ufpd);
        let audit = apply(&mut req, policy);
        assert_eq!(wire(&req), expected(128));
        assert_eq!(audit.counts()[3], 4);
        assert_eq!(audit.schema_counts(SchemaId::User)[3], 3);
        assert_eq!(audit.schema_counts(SchemaId::UserExt)[3], 1);
        assert_eq!(audit.counts()[7], 2);
        assert_eq!(apply(&mut req, policy).counts(), [0; 8]);
    }

    #[test]
    fn classified_fpd_exception_is_only_owned_branches_not_customdata_or_unknown() {
        let mut req = loaded();
        req.user
            .as_mut()
            .unwrap()
            .other
            .insert("kwarray".into(), json!(["news"]));
        let audit = apply(&mut req, decisions(1));
        let result = wire(&req);
        assert_eq!(result["user"]["kwarray"], json!(["news"]));
        assert_eq!(result["user"]["data"], json!([{"uid2": "owned"}]));
        assert_eq!(result["user"]["ext"]["data"], json!({"uid2": "owned"}));
        assert!(result["user"].get("customdata").is_none());
        assert!(result["user"].get("unknown").is_none());
        assert_eq!(audit.counts()[3], 0);
        let deny_audit = apply(&mut req, decisions(4));
        assert_eq!(deny_audit.counts()[3], 4);
    }

    #[test]
    fn classified_delegation_preserves_owned_payloads_but_checks_declared_outer_shapes() {
        for mask in [1, 2, 3] {
            let mut req = loaded();
            let user = req.user.as_mut().unwrap();
            user.other
                .insert("kwarray".into(), json!(["news", {"uid2": "secret"}]));
            user.other
                .insert("keywords".into(), json!({"uid2": "secret"}));
            user.data = Some(json!([{"nested": ["anything", {"uid2": "owned"}]}]));
            user.ext.as_mut().unwrap().data = Some(json!({"arbitrary": [{"uid2": "owned"}]}));
            assert!(user.other.contains_key("kwarray"));
            let audit = apply(&mut req, decisions(mask));
            let disclosed_user = req.user.as_ref().unwrap();
            assert!(!disclosed_user.other.contains_key("kwarray"));
            assert!(!disclosed_user.other.contains_key("keywords"));
            assert_eq!(disclosed_user.keywords.as_deref(), Some("news"));
            assert_eq!(
                disclosed_user.data,
                Some(json!([{"nested": ["anything", {"uid2": "owned"}]}]))
            );
            assert_eq!(
                disclosed_user.ext.as_ref().unwrap().data,
                Some(json!({"arbitrary": [{"uid2": "owned"}]}))
            );
            assert_eq!(audit.counts()[3], 0);
            assert_eq!(audit.counts()[6], 2);
        }
    }

    #[test]
    fn classified_geo_precedence_only_applies_to_owning_geo_restriction() {
        for mask in [1, 2, 128] {
            let mut req = loaded();
            req.user.as_mut().unwrap().other.insert(
                "geo".into(),
                json!({"city": "shadow", "lat": 1.238_f64, "unknown": "secret"}),
            );
            apply(&mut req, decisions(mask));
            if mask == 1 {
                assert_eq!(
                    req.user.as_ref().unwrap().other["geo"],
                    json!({"city": "shadow", "lat": 1.238_f64})
                );
            } else {
                assert!(!req.user.as_ref().unwrap().other.contains_key("geo"));
            }
            let mut shadow_only = loaded();
            shadow_only.user.as_mut().unwrap().geo = None;
            shadow_only.user.as_mut().unwrap().other.insert(
                "geo".into(),
                json!({"city": "shadow", "lat": 1.238_f64, "unknown": "secret"}),
            );
            apply(&mut shadow_only, decisions(mask));
            let expect = match mask {
                1 => json!({"city": "shadow", "lat": 1.238_f64}),
                2 => json!({"city": "shadow", "lat": 1.24_f64}),
                _ => json!({}),
            };
            assert_eq!(wire(&shadow_only)["user"]["geo"], expect);
        }
    }

    #[test]
    fn classified_tid_only_visits_exact_five_leaves_and_no_other_source_context() {
        let mut req = loaded();
        req.source
            .as_mut()
            .unwrap()
            .other
            .insert("tid".into(), json!({"uid2": "shadow"}));
        req.imp[0]
            .ext
            .as_mut()
            .unwrap()
            .other
            .insert("tid".into(), json!("shadow"));
        req.imp[0].other.insert(
            "ext".into(),
            json!({"tid": "shadow", "bidder": {"tid": "outside"}, "unknown": "outside"}),
        );
        let user_before = serde_json::to_vec(&req.user).unwrap();
        let device_before = serde_json::to_vec(&req.device).unwrap();
        let source_ext_before = serde_json::to_vec(&req.source.as_ref().unwrap().ext).unwrap();
        let audit = apply(&mut req, decisions(16));
        assert_eq!(audit.counts(), [0, 0, 0, 0, 5, 0, 0, 0]);
        assert_eq!(serde_json::to_vec(&req.user).unwrap(), user_before);
        assert_eq!(serde_json::to_vec(&req.device).unwrap(), device_before);
        assert_eq!(
            serde_json::to_vec(&req.source.as_ref().unwrap().ext).unwrap(),
            source_ext_before
        );
        assert_eq!(
            req.imp[0].other["ext"],
            json!({"bidder": {"tid": "outside"}, "unknown": "outside"})
        );
        assert_eq!(apply(&mut req, decisions(16)).counts(), [0; 8]);
    }

    #[test]
    fn classified_sparse_null_empty_and_numeric_forms_are_preserved_without_backing_allocation() {
        for ratio in [json!(2_i32), json!(2.0_f64)] {
            for mask in [0, 1, 2, 3, 128] {
                let mut req: BidRequest =
                    serde_json::from_value(json!({"id": "request", "device": {
                    "h": null, "pxratio": ratio, "ext": {"prebid": null},
                    "sua": {"browsers": [{}, {"brand": null, "version": []}], "platform": null}}}))
                    .unwrap();
                let before = serde_json::to_vec(&req).unwrap();
                assert_eq!(apply(&mut req, decisions(mask)).counts(), [0; 8]);
                assert_eq!(serde_json::to_vec(&req).unwrap(), before);
                assert!(
                    req.device
                        .as_ref()
                        .unwrap()
                        .sua
                        .as_ref()
                        .unwrap()
                        .browsers
                        .as_ref()
                        .unwrap()[0]
                        .fields()
                        .is_none()
                );
            }
        }
        let mut req: BidRequest =
            serde_json::from_value(json!({"id": "request", "user": {"geo": {}}, "device": {}}))
                .unwrap();
        assert_eq!(apply(&mut req, decisions(128)).counts(), [0; 8]);
        assert_eq!(
            wire(&req),
            json!({"id": "request", "user": {"geo": {}}, "device": {}})
        );
    }

    #[test]
    fn classified_optional_coordinate_transform_removes_nonfinite_and_overflow_not_null() {
        for coordinate in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::MAX,
            -f64::MAX,
        ] {
            let mut req = loaded();
            req.user.as_mut().unwrap().geo = Some(Geo {
                lat: Some(coordinate),
                lon: Some(coordinate),
                ..Geo::default()
            });
            apply(&mut req, decisions(2));
            assert_eq!(wire(&req)["user"]["geo"], json!({}));
        }
        for (coordinate, rounded) in [
            (1.238_f64, 1.24_f64),
            (1.232_f64, 1.23_f64),
            (-1.238_f64, -1.24_f64),
            (-1.232_f64, -1.23_f64),
            (-1.125_f64, -1.12_f64),
            (1.125_f64, 1.13_f64),
        ] {
            let mut req = loaded();
            req.user.as_mut().unwrap().geo = Some(Geo {
                lat: Some(coordinate),
                ..Geo::default()
            });
            req.device.as_mut().unwrap().geo = None;
            req.device
                .as_mut()
                .unwrap()
                .other
                .insert("geo".into(), json!({"lat": coordinate}));
            apply(&mut req, decisions(2));
            assert_eq!(wire(&req)["user"]["geo"]["lat"], json!(rounded));
            assert_eq!(wire(&req)["device"]["geo"]["lat"], json!(rounded));
        }
        let mut req = loaded();
        req.device.as_mut().unwrap().ip = Some("not-ip".into());
        req.device.as_mut().unwrap().ipv6 = Some("192.168.1.33".into());
        req.device
            .as_mut()
            .unwrap()
            .other
            .insert("geo".into(), json!({"lat": f64::MAX}));
        req.device.as_mut().unwrap().geo = None;
        apply(&mut req, decisions(2));
        assert!(wire(&req)["device"].get("ip").is_none());
        assert!(wire(&req)["device"].get("ipv6").is_none());
        assert_eq!(wire(&req)["device"]["geo"], json!({}));
    }

    #[test]
    fn classified_unknown_subtrees_and_eid_collections_are_counted_once() {
        let mut req: BidRequest = serde_json::from_value(json!({"id": "request", "user": {
            "eids": [eid(), eid()], "unknown": [{"uid2": "secret"}, {"deeper": [1_i32, 2_i32, 3_i32]}]}}))
        .unwrap();
        assert_eq!(
            apply(&mut req, decisions(1)).counts(),
            [1, 0, 0, 0, 0, 1, 0, 0]
        );
        let mut geo_only: BidRequest =
            serde_json::from_value(json!({"id": "request", "user": {"geo": geo()}})).unwrap();
        assert_eq!(
            apply(&mut geo_only, decisions(128)).counts(),
            [0, 0, 0, 0, 0, 0, 0, 1]
        );
        assert_eq!(apply(&mut geo_only, decisions(128)).counts(), [0; 8]);
    }

    #[test]
    fn classified_decision_union_matches_sequential_restrictions_and_is_monotone() {
        for left in 0_u8..=u8::MAX {
            for right in [1, 2, 4, 8, 16, 32, 64, 128] {
                let mut sequential = loaded();
                apply(&mut sequential, decisions(left));
                apply(&mut sequential, decisions(right));
                let mut united = loaded();
                apply(&mut united, decisions(left | right));
                assert_eq!(wire(&sequential), wire(&united), "union {left} {right}");
            }
        }
    }

    // A streaming visitor records every key occurrence, including duplicate flattened keys.
    struct KeyPaths<'a> {
        path: String,
        paths: &'a mut Vec<String>,
    }

    impl<'de> DeserializeSeed<'de> for KeyPaths<'_> {
        type Value = ();
        fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
            deserializer.deserialize_any(self)
        }
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "inherited Serde visitor methods forward JSON scalar forms and reject non-JSON types"
    )]
    impl<'de> Visitor<'de> for KeyPaths<'_> {
        type Value = ();
        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a JSON value")
        }
        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<(), M::Error> {
            while let Some(key) = map.next_key::<String>()? {
                let path = format!("{}/{}", self.path, key);
                self.paths.push(path.clone());
                map.next_value_seed(KeyPaths {
                    path,
                    paths: self.paths,
                })?;
            }
            Ok(())
        }
        fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<(), S::Error> {
            while seq
                .next_element_seed(KeyPaths {
                    path: self.path.clone(),
                    paths: self.paths,
                })?
                .is_some()
            {}
            Ok(())
        }
        fn visit_bool<E: DeError>(self, _: bool) -> Result<(), E> {
            Ok(())
        }
        fn visit_i64<E: DeError>(self, _: i64) -> Result<(), E> {
            Ok(())
        }
        fn visit_u64<E: DeError>(self, _: u64) -> Result<(), E> {
            Ok(())
        }
        fn visit_f64<E: DeError>(self, _: f64) -> Result<(), E> {
            Ok(())
        }
        fn visit_str<E: DeError>(self, _: &str) -> Result<(), E> {
            Ok(())
        }
        fn visit_unit<E: DeError>(self) -> Result<(), E> {
            Ok(())
        }
    }

    fn byte_paths(req: &BidRequest) -> Vec<String> {
        let bytes = serde_json::to_vec(req).unwrap();
        let mut de = serde_json::Deserializer::from_slice(&bytes);
        let mut paths = Vec::new();
        KeyPaths {
            path: String::new(),
            paths: &mut paths,
        }
        .deserialize(&mut de)
        .unwrap();
        de.end().unwrap();
        paths
    }

    #[test]
    fn classified_serialized_bytes_cannot_hide_denied_typed_and_flattened_occurrences() {
        let mut req = loaded();
        for key in ["id", "buyeruid", "yob", "gender", "customdata", "eids"] {
            req.user
                .as_mut()
                .unwrap()
                .other
                .insert(key.into(), json!("shadow"));
        }
        req.user.as_mut().unwrap().other.insert(
            "ext".into(),
            json!({"eids": [eid()], "data": {"uid2": "owned"}}),
        );
        req.device
            .as_mut()
            .unwrap()
            .other
            .insert("ifa".into(), json!("shadow"));
        req.device
            .as_mut()
            .unwrap()
            .other
            .insert("ip".into(), json!("10.1.2.3"));
        let before = byte_paths(&req);
        assert_eq!(
            before
                .iter()
                .filter(|path| path.as_str() == "/user/id")
                .count(),
            2
        );
        apply(&mut req, decisions(3));
        let after = byte_paths(&req);
        for path in [
            "/user/id",
            "/user/buyeruid",
            "/user/yob",
            "/user/gender",
            "/user/customdata",
            "/user/eids",
            "/user/ext/eids",
            "/device/ifa",
        ] {
            assert!(
                !after.iter().any(|actual| actual == path),
                "denied occurrence {path}"
            );
        }
        assert_eq!(
            after
                .iter()
                .filter(|path| path.as_str() == "/device/ip")
                .count(),
            1
        );
        assert!(after.iter().any(|path| path == "/user/ext/data"));
    }

    #[test]
    fn classified_limits_reject_borrowed_depth_129_before_mutation_and_admit_127_128() {
        for (depth, admitted) in [(127_i32, true), (128_i32, true), (129_i32, false)] {
            for mask in [0, 1, 2, 128] {
                let mut req: BidRequest =
                    serde_json::from_value(json!({"id": "request", "user": {"id": "private"}}))
                        .unwrap();
                let mut nested = Value::Null;
                for _ in 2_i32..depth {
                    nested = Value::Array(vec![nested]);
                }
                req.user
                    .as_mut()
                    .unwrap()
                    .other
                    .insert("unknown".into(), nested);
                let mut audit = DisclosureAudit::default();
                let result = super::apply_classified_with_limits(
                    &mut req,
                    decisions(mask),
                    KEEP,
                    &mut audit,
                    10_000,
                    128,
                );
                assert_eq!(result.is_ok(), admitted, "depth {depth}, mask {mask}");
                assert_eq!(audit.attempted_scopes, 1);
                assert_eq!(audit.failed_scopes, u64::from(!admitted));
                if admitted {
                    let repeated = apply(&mut req, decisions(mask));
                    assert_eq!(repeated.counts(), [0; 8]);
                } else {
                    assert_eq!(audit.counts(), [0; 8]);
                    assert_eq!(req.user.as_ref().unwrap().id.as_deref(), Some("private"));
                    assert!(matches!(result, Err(crate::AuctionError::Internal(_))));
                }
                // Rejected/unrestricted caller-owned input is torn down iteratively.
                if let Some(mut remaining) = req.user.as_mut().unwrap().other.remove("unknown") {
                    while let Value::Array(mut items) = remaining {
                        remaining = items.pop().unwrap();
                    }
                }
            }
        }
    }

    #[test]
    fn classified_limit_failure_retains_prior_auction_counts_and_payload_ownership() {
        let mut audit = DisclosureAudit::default();
        audit.start_scope();
        audit.record_schema(SchemaId::User, ChangeKind::Id);
        let mut req = loaded();
        let before = serde_json::to_vec(&req).unwrap();
        let result =
            super::apply_classified_with_limits(&mut req, decisions(255), KEEP, &mut audit, 1, 128);
        assert!(matches!(result, Err(crate::AuctionError::Internal(_))));
        assert_eq!(audit.counts(), [1, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(audit.attempted_scopes, 2);
        assert_eq!(audit.failed_scopes, 1);
        assert_eq!(serde_json::to_vec(&req).unwrap(), before);
        // Even a caller-supplied depth cap cannot widen the construction contract.
        let mut nested = Value::Null;
        for _ in 1_i32..129_i32 {
            nested = Value::Array(vec![nested]);
        }
        req.other.insert("unknown".into(), nested);
        assert!(
            super::apply_classified_with_limits(
                &mut req,
                ALLOW,
                KEEP,
                &mut audit,
                10_000,
                u32::MAX
            )
            .is_err()
        );
        let mut remaining = req.other.remove("unknown").unwrap();
        while let Value::Array(mut items) = remaining {
            remaining = items.pop().unwrap();
        }
    }

    #[test]
    fn classified_resource_preflight_bounds_raw_json_even_before_opaque_deletion() {
        for (depth, admitted) in [(127, true), (128, true), (129, false)] {
            let mut req: BidRequest = serde_json::from_value(
                json!({"id": "request", "user": {"id": "private", "geo": {}}}),
            )
            .unwrap();
            let raw = format!("{}null{}", "[".repeat(depth - 3), "]".repeat(depth - 3));
            req.user.as_mut().unwrap().geo.as_mut().unwrap().ext =
                Some(RawValue::from_string(raw.clone()).unwrap());
            let mut audit = DisclosureAudit::default();
            let result = super::apply_classified_with_limits(
                &mut req,
                decisions(1),
                KEEP,
                &mut audit,
                10_000,
                128,
            );
            assert_eq!(result.is_ok(), admitted, "embedded raw depth {depth}");
            if admitted {
                assert!(
                    req.user
                        .as_ref()
                        .unwrap()
                        .geo
                        .as_ref()
                        .unwrap()
                        .ext
                        .is_none()
                );
            } else {
                assert_eq!(req.user.as_ref().unwrap().id.as_deref(), Some("private"));
                assert_eq!(
                    req.user
                        .as_ref()
                        .unwrap()
                        .geo
                        .as_ref()
                        .unwrap()
                        .ext
                        .as_ref()
                        .unwrap()
                        .get(),
                    raw
                );
                assert_eq!(audit.counts(), [0; 8]);
                assert_eq!(audit.failed_scopes, 1);
            }
        }
    }

    #[test]
    fn classified_layout_charges_use_the_actual_audit_frames_and_transform_owner() {
        let costs = super::construction_costs();
        assert_eq!(
            costs.fixed_per_auction_bytes,
            u64::try_from(size_of::<DisclosureAudit>()).unwrap()
        );
        assert_eq!(
            costs.reusable_scratch_bytes,
            u64::try_from(size_of::<Frames<'static>>() + size_of::<Mutation<'static>>()).unwrap()
        );
        assert!(
            costs.reusable_scratch_bytes > u64::try_from(size_of::<TransformScratch>()).unwrap()
        );
    }

    #[test]
    fn classified_ordinary_fixtures_use_no_heap_allocation_for_composed_and_sparse_context_mutation()
     {
        for mask in 0_u8..=u8::MAX {
            let mut req = loaded();
            let mut audit = DisclosureAudit::default();
            let baseline = alloc_track::reset_peak();
            let calls_before = ALLOCATION_CALLS.with(Cell::get);
            let result = apply_classified(&mut req, decisions(mask), KEEP, &mut audit);
            let calls_after = ALLOCATION_CALLS.with(Cell::get);
            let peak = alloc_track::peak_since(baseline);
            let retained = alloc_track::live();
            result.unwrap();
            assert_eq!(calls_after, calls_before, "allocation activity mask {mask}");
            assert_eq!(peak, 0, "mutation peak mask {mask}");
            assert!(retained <= baseline);
        }
        let mut req: BidRequest = serde_json::from_value(
            json!({"id": "request", "device": {"sua": {"browsers": vec![json!({}); 512]}}}),
        )
        .unwrap();
        let mut audit = DisclosureAudit::default();
        let baseline = alloc_track::reset_peak();
        let calls_before = ALLOCATION_CALLS.with(Cell::get);
        apply_classified(&mut req, decisions(3), KEEP, &mut audit).unwrap();
        assert_eq!(ALLOCATION_CALLS.with(Cell::get), calls_before);
        assert_eq!(alloc_track::peak_since(baseline), 0);
        assert_eq!(alloc_track::live(), baseline);
        assert!(
            req.device
                .as_ref()
                .unwrap()
                .sua
                .as_ref()
                .unwrap()
                .browsers
                .as_ref()
                .unwrap()
                .iter()
                .all(|brand| brand.fields().is_none())
        );
    }

    fn ipv6_request(address: String, shadow: bool) -> BidRequest {
        let device = if shadow {
            Device {
                other: serde_json::Map::from_iter([("ipv6".to_owned(), Value::String(address))]),
                ..Device::default()
            }
        } else {
            Device {
                ipv6: Some(address),
                ..Device::default()
            }
        };
        BidRequest {
            id: "ipv6-growth".to_owned(),
            device: Some(device),
            ..BidRequest::default()
        }
    }

    fn assert_coarsened_ipv6(
        request: &BidRequest,
        shadow: bool,
        network: &str,
        audit: &DisclosureAudit,
    ) {
        let rewritten = request.device.as_ref().unwrap();
        let actual = if shadow {
            assert!(rewritten.ipv6.is_none());
            rewritten.other.get("ipv6").and_then(Value::as_str)
        } else {
            assert!(!rewritten.other.contains_key("ipv6"));
            rewritten.ipv6.as_deref()
        };
        // Capacity changes funding, never the canonical network or permission.
        assert_eq!(actual, Some(network));
        assert_eq!(audit.counts(), [0, 0, 1, 0, 0, 0, 0, 0]);
        assert_eq!(
            audit.schema_counts(SchemaId::Device),
            [0, 0, 1, 0, 0, 0, 0, 0]
        );
        assert_eq!(audit.attempted_scopes, 1);
        assert_eq!(audit.failed_scopes, 0);
    }

    fn assert_ipv6_bounded_growth(raw: &str, network: &str, keep: u8, shadow: bool) {
        let address_bytes = size_of::<AddressBuffer>();
        let growth_limit = u64::try_from(address_bytes).unwrap();
        let mut tight_wire = None;
        for spare in [false, true] {
            let mut address = Box::<str>::from(raw).into_string();
            assert_eq!(address.capacity(), address.len());
            if spare {
                address
                    .try_reserve_exact(address_bytes.checked_sub(address.len()).unwrap())
                    .unwrap();
                assert!(address.capacity() >= address_bytes);
            }
            let mut request = ipv6_request(address, shadow);
            let mut audit = DisclosureAudit::default();
            let baseline = alloc_track::reset_peak();
            let calls_before = ALLOCATION_CALLS.with(Cell::get);
            let bytes_before = ALLOCATION_REQUEST_BYTES.with(Cell::get);
            let result = apply_classified(
                &mut request,
                decisions(2),
                IpKeepBits { v4: 24, v6: keep },
                &mut audit,
            );
            let calls = ALLOCATION_CALLS
                .with(Cell::get)
                .checked_sub(calls_before)
                .unwrap();
            let requested = ALLOCATION_REQUEST_BYTES
                .with(Cell::get)
                .checked_sub(bytes_before)
                .unwrap();
            let peak = alloc_track::peak_since(baseline);
            let retained = alloc_track::live();

            result.unwrap();
            assert_coarsened_ipv6(&request, shadow, network, &audit);
            let output = serde_json::to_vec(&request).unwrap();
            if let Some(tight) = tight_wire.as_ref() {
                assert_eq!(&output, tight);
            } else {
                tight_wire = Some(output);
            }
            assert!(calls <= 1);
            assert!(requested <= growth_limit);
            assert!(peak <= growth_limit);
            if spare {
                assert_eq!(calls, 0);
                assert_eq!(requested, 0);
                assert_eq!(peak, 0);
                assert_eq!(retained, baseline);
            } else {
                assert_eq!(calls, 1);
                assert_eq!(requested, u64::try_from(network.len()).unwrap());
                // The tracker adds the new destination before subtracting the old.
                assert_eq!(peak, requested);
                let growth = network.len().checked_sub(raw.len()).unwrap();
                assert_eq!(
                    retained,
                    baseline
                        .checked_add(i64::try_from(growth).unwrap())
                        .unwrap()
                );
            }
            let repeated_calls = ALLOCATION_CALLS.with(Cell::get);
            let mut repeated = DisclosureAudit::default();
            apply_classified(
                &mut request,
                decisions(2),
                IpKeepBits { v4: 24, v6: keep },
                &mut repeated,
            )
            .unwrap();
            assert_eq!(ALLOCATION_CALLS.with(Cell::get), repeated_calls);
            assert_eq!(repeated.counts(), [0; 8]);
        }
    }

    #[test]
    fn classified_ipv6_bounded_growth_typed_mapped_keeps_canonical_wire() {
        assert_ipv6_bounded_growth("::ffff:1:1", "::ffff:0.0.0.0", 96, false);
    }

    #[test]
    fn classified_ipv6_bounded_growth_shadow_mapped_keeps_canonical_wire() {
        assert_ipv6_bounded_growth("::ffff:1:1", "::ffff:0.0.0.0", 96, true);
    }

    #[test]
    fn classified_ipv6_bounded_growth_typed_singleton_keeps_canonical_wire() {
        assert_ipv6_bounded_growth("1:2::3:4:5:6:7", "1:2:0:3:4:5:6:0", 112, false);
    }

    #[test]
    fn classified_ipv6_bounded_growth_shadow_singleton_keeps_canonical_wire() {
        assert_ipv6_bounded_growth("1:2::3:4:5:6:7", "1:2:0:3:4:5:6:0", 112, true);
    }

    fn assert_ipv6_growth_failure_is_static_and_keeps_the_field(shadow: bool) {
        let address = Box::<str>::from("::ffff:1:1").into_string();
        let mut request = ipv6_request(address, shadow);
        request
            .device
            .as_mut()
            .unwrap()
            .other
            .insert("zzz_unknown".to_owned(), json!("untouched-after-failure"));
        let before = serde_json::to_vec(&request).unwrap();
        let mut audit = DisclosureAudit::default();
        FAIL_NEXT_ALLOCATION.with(|fail| fail.set(true));
        let result = apply_classified(
            &mut request,
            decisions(2),
            IpKeepBits { v4: 24, v6: 96 },
            &mut audit,
        );
        assert!(!FAIL_NEXT_ALLOCATION.with(Cell::get));
        match result {
            Err(crate::AuctionError::Internal(message)) => {
                assert_eq!(message, "request disclosure construction limit exceeded");
            }
            _ => panic!("address reservation failure must refuse the scope"),
        }
        assert_eq!(serde_json::to_vec(&request).unwrap(), before);
        assert_eq!(audit.counts(), [0; 8]);
        assert_eq!(audit.attempted_scopes, 1);
        assert_eq!(audit.failed_scopes, 1);
    }

    #[test]
    fn classified_ipv6_bounded_growth_typed_failure_refuses_without_invalid_drop() {
        assert_ipv6_growth_failure_is_static_and_keeps_the_field(false);
    }

    #[test]
    fn classified_ipv6_bounded_growth_shadow_failure_refuses_without_invalid_drop() {
        assert_ipv6_growth_failure_is_static_and_keeps_the_field(true);
    }

    #[test]
    fn classified_tid_only_closes_malformed_direct_imp_ext_without_source_normalization() {
        for malformed in [Value::Null, json!("secret"), json!([{"tid": "secret"}])] {
            let mut req = loaded();
            req.imp[0].other.insert("ext".into(), malformed);
            let source_context = serde_json::to_vec(&req.source.as_ref().unwrap().ext).unwrap();
            let audit = apply(&mut req, decisions(16));
            assert!(!req.imp[0].other.contains_key("ext"));
            assert_eq!(audit.counts()[6], 1);
            assert_eq!(
                serde_json::to_vec(&req.source.as_ref().unwrap().ext).unwrap(),
                source_context
            );
        }
    }

    #[test]
    fn classified_coppa_audit_distinguishes_typed_shadow_and_conflicting_geo_clears() {
        for representation in 0_u8..3 {
            let mut request: BidRequest = serde_json::from_value(json!({
                "id": "coppa-audit",
                "user": {"geo": {"country": "USA", "lat": 1.125_f64, "zip": "94107"}}
            }))
            .unwrap();
            if representation != 0 {
                let user = request.user.as_mut().unwrap();
                user.other.insert(
                    "geo".to_owned(),
                    json!({"country": "CAN", "lat": -1.125_f64, "city": "Toronto"}),
                );
                if representation == 1 {
                    user.geo = None;
                }
            }
            let audit = apply(&mut request, decisions(128));
            assert_eq!(
                wire(&request),
                json!({"id": "coppa-audit", "user": {"geo": {}}})
            );
            let changes = if representation == 2 { 2_u64 } else { 1_u64 };
            // Coppa is appended after the seven existing reasons, not merged into Geo.
            assert_eq!(
                (audit.counts()[1], audit.counts().get(7).copied()),
                (0, Some(changes))
            );
            assert_eq!(audit.schema_counts(SchemaId::Geo).get(7).copied(), Some(1),);
            assert_eq!(
                audit.schema_counts(SchemaId::User).get(7).copied(),
                Some(u64::from(representation == 2)),
            );
            let repeated = apply(&mut request, decisions(128));
            assert_eq!(repeated.counts().as_slice(), &[0_u64; 8]);
        }
    }

    #[test]
    fn classified_coppa_audit_keeps_partial_geo_and_noop_scopes_separate() {
        let mut request: BidRequest = serde_json::from_value(json!({
            "id": "partial-audit",
            "user": {"geo": {"country": "USA", "lat": 1.125_f64, "zip": "94107"}}
        }))
        .unwrap();
        let audit = apply(&mut request, decisions(2));
        assert_eq!(
            wire(&request),
            json!({
                "id": "partial-audit", "user": {"geo": {"country": "USA", "lat": 1.13_f64}}
            })
        );
        assert_eq!(audit.counts().as_slice(), &[0_u64, 1, 1, 0, 0, 0, 0, 0]);
        assert_eq!(
            apply(&mut request, decisions(2)).counts().as_slice(),
            &[0_u64; 8]
        );
        for user in [json!({}), json!({"geo": {}})] {
            let mut noop_request: BidRequest =
                serde_json::from_value(json!({"id": "coppa-noop", "user": user})).unwrap();
            let before = wire(&noop_request);
            let noop_audit = apply(&mut noop_request, decisions(128));
            assert_eq!(wire(&noop_request), before);
            assert_eq!(noop_audit.counts().as_slice(), &[0_u64; 8]);
            assert_eq!(noop_audit.attempted_scopes, 1);
            assert_eq!(noop_audit.failed_scopes, 0);
        }
    }

    // Inline target-executable proofs: no thread builder or enlarged host stack.
    mod target_depth_proof {
        use super::{ALLOCATION_CALLS, KEEP, decisions};
        use crate::AliasMap;
        use crate::fake_bidder::{self, FakeBidder};
        use crate::scrub::{DisclosureAudit, apply_classified, apply_classified_with_limits};
        use crate::split::{group_by_bidder, materialize_per_bidder};
        use auctioneer_bidder::BidderRegistry;
        use auctioneer_config_data::disclosure::SchemaId;
        use auctioneer_openrtb::ext::imp::{ExtImpPrebid, ImpExt};
        use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, Imp};
        use auctioneer_openrtb::types::field::JsonField;
        use auctioneer_openrtb::types::inventory::Site;
        use auctioneer_openrtb::types::user::User;
        use auctioneer_runtime::json_preflight::{PreflightError, preflight};
        use serde_json::{Map, Value, json};
        use std::cell::Cell;
        use std::collections::{BTreeMap, BTreeSet};
        use std::ptr::from_ref;

        fn protected_unknown(depth: u32, object_chain: bool) -> BidRequest {
            let mut nested = Value::String("unknown-secret".to_owned());
            // The request and User contribute the first two container levels.
            for _ in 2..depth {
                nested = if object_chain {
                    Value::Object(Map::from_iter([("next".to_owned(), nested)]))
                } else {
                    Value::Array(vec![nested])
                };
            }
            BidRequest {
                id: "depth-proof".to_owned(),
                user: Some(User {
                    id: Some("private".to_owned()),
                    other: Map::from_iter([("unknown".to_owned(), nested)]),
                    ..User::default()
                }),
                ..BidRequest::default()
            }
        }

        fn remove_admitted_inline(depth: u32) {
            for object_chain in [false, true] {
                for mask in [1, 2, 128] {
                    let mut request = protected_unknown(depth, object_chain);
                    let stats = preflight(&request, u64::MAX, 128).unwrap();
                    assert_eq!(stats.max_depth, depth);
                    let mut audit = DisclosureAudit::default();
                    let calls_before = ALLOCATION_CALLS.with(Cell::get);
                    apply_classified_with_limits(
                        &mut request,
                        decisions(mask),
                        KEEP,
                        &mut audit,
                        stats.nodes,
                        128,
                    )
                    .unwrap();
                    // Map::retain destroys the admitted whole unknown subtree here.
                    assert_eq!(ALLOCATION_CALLS.with(Cell::get), calls_before);
                    let user = request.user.as_ref().unwrap();
                    assert!(!user.other.contains_key("unknown"));
                    assert_eq!(user.id.as_deref(), (mask == 2).then_some("private"));
                    let counts = [u64::from(mask != 2), 0, 0, 0, 0, 1, 0, 0];
                    assert_eq!(audit.counts(), counts);
                    assert_eq!(audit.schema_counts(SchemaId::User), counts);
                    assert_eq!(audit.attempted_scopes, 1);
                    assert_eq!(audit.failed_scopes, 0);
                    let mut repeated = DisclosureAudit::default();
                    apply_classified(&mut request, decisions(mask), KEEP, &mut repeated).unwrap();
                    assert_eq!(repeated.counts(), [0; 8]);
                }
            }
        }

        #[test]
        fn protected_unknown_depth_127_is_removed_inline() {
            remove_admitted_inline(127);
        }

        #[test]
        fn protected_unknown_depth_128_is_removed_inline() {
            remove_admitted_inline(128);
        }

        fn fake_b_registry() -> BidderRegistry {
            let mut registry = BidderRegistry::new();
            let info = fake_bidder::info();
            registry
                .register(info, Box::new(FakeBidder::default()))
                .expect("unique test bidder");
            registry
        }

        fn construction_source(depth: u32, object_chain: bool, with_fpd: bool) -> BidRequest {
            let mut source = protected_unknown(depth, object_chain);
            source.site = Some(Site {
                id: Some("publisher".to_owned()),
                ..Site::default()
            });
            let prebid = with_fpd.then(|| {
                let mut allowed = Value::String("permitted-fpd".to_owned());
                // The real source route is root/imp[]/imp/ext/prebid/imp/fake_bb/ext/data.
                for _ in 9..depth {
                    allowed = if object_chain {
                        Value::Object(Map::from_iter([("next".to_owned(), allowed)]))
                    } else {
                        Value::Array(vec![allowed])
                    };
                }
                let data = Value::Object(Map::from_iter([("allowed".to_owned(), allowed)]));
                let extension = Value::Object(Map::from_iter([("data".to_owned(), data)]));
                let patch = Value::Object(Map::from_iter([
                    ("banner".to_owned(), json!({"w": 320_i32})),
                    ("ext".to_owned(), extension),
                ]));
                ExtImpPrebid {
                    imp: BTreeMap::from([("fake_bb".to_owned(), patch)]),
                    ..ExtImpPrebid::default()
                }
            });
            source.imp.push(Imp {
                id: "imp".to_owned(),
                banner: Some(Banner {
                    w: Some(300_i32),
                    h: Some(250_i32),
                    ..Banner::default()
                }),
                ext: Some(ImpExt {
                    bidder: JsonField::Value(json!({"fake_bb": {}})),
                    prebid: prebid.map_or(JsonField::Missing, JsonField::Value),
                    ..ImpExt::default()
                }),
                ..Imp::default()
            });
            source.validate().unwrap();
            source
        }

        fn assert_allowed_fpd(request: &BidRequest, depth: u32, object_chain: bool) {
            assert_eq!(request.imp[0].banner.as_ref().unwrap().w, Some(320_i32));
            assert_eq!(request.imp[0].banner.as_ref().unwrap().h, Some(250_i32));
            let data = &request.imp[0].ext.as_ref().unwrap().other["data"];
            let mut allowed = &data["allowed"];
            for _ in 9..depth {
                allowed = if object_chain {
                    &allowed["next"]
                } else {
                    &allowed[0]
                };
            }
            assert_eq!(allowed.as_str(), Some("permitted-fpd"));
            // Actual merge moves ext/data four levels outward, never deeper.
            assert_eq!(
                preflight(data, u64::MAX, 128).unwrap().max_depth,
                depth.checked_sub(8).unwrap()
            );
        }

        #[test]
        fn construction_source_127_128_groups_materializes_and_scrubs_inline() {
            let registry = fake_b_registry();
            let aliases = AliasMap::default();
            let disabled = BTreeSet::new();
            for depth in [127_u32, 128] {
                for object_chain in [false, true] {
                    for with_fpd in [false, true] {
                        let source = construction_source(depth, object_chain, with_fpd);
                        let source_stats = preflight(&source, u64::MAX, 128).unwrap();
                        assert_eq!(source_stats.max_depth, depth);
                        let grouped =
                            group_by_bidder(&source, &aliases, &registry, &disabled, &disabled)
                                .unwrap();
                        let survivors: Vec<_> = grouped.bidder_imps().keys().cloned().collect();
                        let mut scopes = materialize_per_bidder(
                            &grouped, &survivors, &aliases, &registry, &disabled,
                        )
                        .unwrap();
                        assert_eq!(scopes.len(), 1);
                        drop(grouped);
                        let scope = &mut scopes[0];
                        assert_eq!(scope.bidder.as_str(), "fake_bb");
                        let scoped_stats = preflight(&scope.request, u64::MAX, 128).unwrap();
                        assert_eq!(scoped_stats.max_depth, depth);
                        if with_fpd {
                            assert_allowed_fpd(&scope.request, depth, object_chain);
                        }
                        let mut audit = DisclosureAudit::default();
                        apply_classified_with_limits(
                            &mut scope.request,
                            decisions(1),
                            KEEP,
                            &mut audit,
                            scoped_stats.nodes,
                            128,
                        )
                        .unwrap();
                        let user = scope.request.user.as_ref().unwrap();
                        assert!(!user.other.contains_key("unknown"));
                        assert!(user.id.is_none());
                        assert_eq!(audit.counts(), [1, 0, 0, 0, 0, 1, 0, 0]);
                        let retained_depth =
                            preflight(&scope.request, u64::MAX, 128).unwrap().max_depth;
                        assert_eq!(
                            retained_depth,
                            if with_fpd {
                                depth.checked_sub(4).unwrap()
                            } else {
                                5
                            }
                        );
                        if with_fpd {
                            assert_allowed_fpd(&scope.request, depth, object_chain);
                        }
                        assert_eq!(preflight(&source, u64::MAX, 128).unwrap(), source_stats);
                        let original_user = source.user.as_ref().unwrap();
                        assert_eq!(original_user.id.as_deref(), Some("private"));
                        assert!(original_user.other.contains_key("unknown"));
                        assert_eq!(source.imp[0].banner.as_ref().unwrap().w, Some(300_i32));
                        drop(scopes);
                    }
                }
            }
        }

        #[test]
        fn construction_source_127_128_cancellation_drops_unscrubbed_scopes_inline() {
            let registry = fake_b_registry();
            let aliases = AliasMap::default();
            let disabled = BTreeSet::new();
            for depth in [127_u32, 128] {
                for object_chain in [false, true] {
                    for with_fpd in [false, true] {
                        let source = construction_source(depth, object_chain, with_fpd);
                        let source_stats = preflight(&source, u64::MAX, 128).unwrap();
                        assert_eq!(source_stats.max_depth, depth);
                        let grouped =
                            group_by_bidder(&source, &aliases, &registry, &disabled, &disabled)
                                .unwrap();
                        let survivors: Vec<_> = grouped.bidder_imps().keys().cloned().collect();
                        let scopes = materialize_per_bidder(
                            &grouped, &survivors, &aliases, &registry, &disabled,
                        )
                        .unwrap();
                        assert_eq!(scopes.len(), 1);
                        let request = &scopes[0].request;
                        assert_eq!(preflight(request, u64::MAX, 128).unwrap().max_depth, depth);
                        let scoped_user = request.user.as_ref().unwrap();
                        assert_eq!(scoped_user.id.as_deref(), Some("private"));
                        assert!(scoped_user.other.contains_key("unknown"));
                        if with_fpd {
                            assert_allowed_fpd(request, depth, object_chain);
                        }
                        drop(grouped);
                        // Cancellation happens before classification: destruction must
                        // handle admitted deep trees without relying on scrub deletion.
                        drop(scopes);
                        assert_eq!(preflight(&source, u64::MAX, 128).unwrap(), source_stats);
                        let original_user = source.user.as_ref().unwrap();
                        assert_eq!(original_user.id.as_deref(), Some("private"));
                        assert!(original_user.other.contains_key("unknown"));
                        assert_eq!(source.imp[0].banner.as_ref().unwrap().w, Some(300_i32));
                    }
                }
            }
        }

        fn caller_cleanup(mut nested: Value) -> u32 {
            let mut depth = 2_u32;
            loop {
                nested = match nested {
                    Value::Array(mut children) => {
                        assert_eq!(children.len(), 1);
                        depth = depth.checked_add(1).unwrap();
                        children.pop().unwrap()
                    }
                    Value::Object(mut children) => {
                        assert_eq!(children.len(), 1);
                        depth = depth.checked_add(1).unwrap();
                        children.remove("next").unwrap()
                    }
                    Value::String(marker) => {
                        assert_eq!(marker, "unknown-secret");
                        return depth;
                    }
                    Value::Null | Value::Bool(_) | Value::Number(_) => {
                        panic!("unexpected caller-owned chain")
                    }
                };
            }
        }

        #[test]
        fn borrowed_depth_129_refuses_before_mutation_and_caller_cleans_iteratively() {
            for object_chain in [false, true] {
                for mask in [1, 2, 128] {
                    let mut request = protected_unknown(129, object_chain);
                    assert_eq!(
                        preflight(&request, u64::MAX, 128),
                        Err(PreflightError::Depth)
                    );
                    let before =
                        from_ref(request.user.as_ref().unwrap().other.get("unknown").unwrap());
                    let mut audit = DisclosureAudit::default();
                    let result = apply_classified_with_limits(
                        &mut request,
                        decisions(mask),
                        KEEP,
                        &mut audit,
                        u64::MAX,
                        128,
                    );
                    assert!(matches!(result, Err(crate::AuctionError::Internal(_))));
                    let user = request.user.as_mut().unwrap();
                    assert_eq!(user.id.as_deref(), Some("private"));
                    assert_eq!(user.other.len(), 1);
                    assert_eq!(from_ref(user.other.get("unknown").unwrap()), before);
                    assert_eq!(audit.counts(), [0; 8]);
                    assert_eq!(audit.attempted_scopes, 1);
                    assert_eq!(audit.failed_scopes, 1);
                    // Take ownership only after refusal, then dismantle without recursive drop.
                    assert_eq!(caller_cleanup(user.other.remove("unknown").unwrap()), 129);
                }
            }
        }

        #[test]
        fn actual_bidrequest_decoder_admits_127_and_refuses_128_129_inline() {
            for (depth, admitted) in [(127, true), (128, false), (129, false)] {
                for object_chain in [false, true] {
                    let opening = if object_chain { "{\"next\":" } else { "[" };
                    let closing = if object_chain { "}" } else { "]" };
                    let bytes = format!(
                        "{{\"id\":\"depth-proof\",\"user\":{{\"id\":\"private\",\"unknown\":{}\"unknown-secret\"{}}}}}",
                        opening.repeat(depth - 2),
                        closing.repeat(depth - 2),
                    );
                    // Same pinned serde_json entry point as the auction handler.
                    let parsed = serde_json::from_slice::<BidRequest>(bytes.as_bytes());
                    assert_eq!(parsed.is_ok(), admitted, "decoder container depth {depth}");
                    match parsed {
                        Ok(mut request) => {
                            let stats = preflight(&request, u64::MAX, 128).unwrap();
                            assert_eq!(stats.max_depth, 127);
                            let mut audit = DisclosureAudit::default();
                            apply_classified_with_limits(
                                &mut request,
                                decisions(1),
                                KEEP,
                                &mut audit,
                                stats.nodes,
                                128,
                            )
                            .unwrap();
                            assert!(!request.user.as_ref().unwrap().other.contains_key("unknown"));
                            assert_eq!(audit.counts(), [1, 0, 0, 0, 0, 1, 0, 0]);
                        }
                        Err(error) => {
                            assert!(error.is_syntax());
                            assert!(error.to_string().contains("recursion limit exceeded"));
                        }
                    }
                }
            }
        }
    }
}

/// Independent legacy decision and wire oracles against the classified mutator.
#[cfg(test)]
mod composition_tests {
    use super::{
        ScrubDecisions, apply_gdpr_scrubs, apply_scrubs, scrub_ccpa_lmt, scrub_coppa,
        scrub_gdpr_id, scrub_geo_and_device_ip, scrub_tid, scrub_user_fpd,
    };
    use auctioneer_config_data::types::IpKeepBits;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};

    const KEEP: IpKeepBits = IpKeepBits { v4: 24, v6: 56 };
    const ALLOW: ScrubDecisions = ScrubDecisions {
        pass_id: true,
        pass_geo: true,
        transmit_ufpd: true,
        transmit_precise_geo: true,
        transmit_tid: true,
        ccpa: false,
        lmt: false,
        coppa: false,
    };

    use crate::test_support::privacy_fixture;
    use privacy_fixture::full_geo;

    fn loaded_request() -> BidRequest {
        privacy_fixture::request().expect("loaded fixture deserializes")
    }

    fn wire(req: &BidRequest) -> Value {
        let bytes = serde_json::to_vec(req).expect("request serializes");
        serde_json::from_slice(&bytes).expect("serialized request is JSON")
    }

    fn remove_fields(value: &mut Value, parent: &str, fields: &[&str]) {
        let object = value
            .pointer_mut(parent)
            .and_then(Value::as_object_mut)
            .expect("fixture parent exists");
        for field in fields {
            assert!(
                object.remove(*field).is_some(),
                "before-state presence: {parent}/{field}"
            );
        }
    }

    #[test]
    #[should_panic(expected = "before-state presence")]
    fn wire_oracle_refuses_a_missing_before_state_field() {
        let mut value = json!({"user": {}});
        remove_fields(&mut value, "/user", &["id"]);
    }

    // Independent wire oracle: literal field inventory and expected coarse values,
    // never production scrub, rounding, masking, or decision helpers.
    fn reviewed_geo(country: &str, lat: Option<f64>, lon: Option<f64>) -> Value {
        let mut expected = json!({
            "country": country, "type": 1_i32, "lastfix": 2_i32, "ipservice": 3_i32,
            "region": "CA", "regionfips104": "06", "metro": "807",
            "city": "San Francisco", "utcoffset": -480_i32
        });
        let object = expected.as_object_mut().expect("reviewed context");
        if let Some(latitude) = lat {
            object.insert("lat".to_owned(), json!(latitude));
        }
        if let Some(longitude) = lon {
            object.insert("lon".to_owned(), json!(longitude));
        }
        expected
    }

    fn expected_wire(ids: bool, fpd: bool, geo: bool, full_geo: bool, tid: bool) -> Value {
        let mut expected = wire(&loaded_request());
        if ids {
            remove_fields(
                &mut expected,
                "/user",
                &["id", "buyeruid", "yob", "gender", "eids"],
            );
            remove_fields(&mut expected, "/user/ext", &["eids"]);
            remove_fields(
                &mut expected,
                "/device",
                &[
                    "ifa", "dpidsha1", "dpidmd5", "didsha1", "didmd5", "macsha1", "macmd5",
                ],
            );
        }
        if ids || geo {
            remove_fields(&mut expected, "/user", &["custom"]);
            // Only native user.consent is a control; legacy ext.consent is unknown.
            remove_fields(&mut expected, "/user/ext", &["consent", "vendor"]);
            remove_fields(&mut expected, "/device", &["uid2", "metadata"]);
            remove_fields(&mut expected, "/device/ext", &["vendor"]);
            for path in ["/user/geo", "/device/geo"] {
                remove_fields(&mut expected, path, &["ext", "vendor"]);
            }
        }
        if fpd {
            remove_fields(&mut expected, "/user", &["data", "keywords", "kwarray"]);
            remove_fields(&mut expected, "/user/ext", &["data"]);
        }
        if geo {
            expected["user"]["geo"] = reviewed_geo("USA", Some(51.51_f64), Some(-0.13_f64));
            expected["device"]["geo"] = reviewed_geo("USA", Some(48.86_f64), Some(2.29_f64));
            expected["device"]["ip"] = json!("192.168.1.0");
            expected["device"]["ipv6"] = json!("2001:db8:85a3:1200::");
        }
        if full_geo {
            expected["user"]["geo"] = json!({});
            expected["device"]["geo"] = json!({});
        }
        if tid {
            remove_fields(&mut expected, "/source", &["tid"]);
            remove_fields(&mut expected, "/imp/0/ext", &["tid"]);
            remove_fields(&mut expected, "/imp/1/ext", &["tid"]);
        }
        expected
    }

    #[test]
    fn user_fpd_removes_ids_and_all_user_fpd_but_preserves_context() {
        let mut req = loaded_request();
        scrub_user_fpd(&mut req);
        assert_eq!(wire(&req), expected_wire(true, true, false, false, false));
    }

    #[test]
    fn ccpa_lmt_removes_ids_and_coarsens_geo_but_preserves_typed_fpd() {
        let mut req = loaded_request();
        scrub_ccpa_lmt(&mut req, KEEP);
        assert_eq!(wire(&req), expected_wire(true, false, true, false, false));
    }

    #[test]
    fn coppa_removes_user_fpd_and_empties_geo_without_removing_context_or_tids() {
        let mut req = loaded_request();
        scrub_coppa(&mut req, KEEP);
        assert_eq!(wire(&req), expected_wire(true, true, true, true, false));
    }

    #[test]
    fn tid_removes_source_and_every_impression_tid_not_ordinary_ids_or_params() {
        let mut req = loaded_request();
        scrub_tid(&mut req);
        assert_eq!(wire(&req), expected_wire(false, false, false, false, true));
    }

    fn decisions(mask: u8) -> ScrubDecisions {
        ScrubDecisions {
            pass_id: mask & 1 == 0,
            pass_geo: mask & 2 == 0,
            transmit_ufpd: mask & 4 == 0,
            transmit_precise_geo: mask & 8 == 0,
            transmit_tid: mask & 16 == 0,
            ccpa: mask & 32 != 0,
            lmt: mask & 64 != 0,
            coppa: mask & 128 != 0,
        }
    }

    #[test]
    fn all_256_decisions_match_the_independent_union_and_are_idempotent() {
        let original = loaded_request();
        let sibling = original.clone();
        let before = wire(&original);
        for mask in 0_u8..=u8::MAX {
            let mut req = original.clone();
            apply_scrubs(&mut req, decisions(mask), KEEP);
            // ID: GDPR/FPD/CCPA/LMT/COPPA. Geo: GDPR/activity/CCPA/LMT/COPPA.
            let expected = expected_wire(
                mask & 0b1110_0101 != 0,
                mask & (4 | 128) != 0,
                mask & 0b1110_1010 != 0,
                mask & 128 != 0,
                mask & 16 != 0,
            );
            assert_eq!(wire(&req), expected, "decision mask {mask}");
            apply_scrubs(&mut req, decisions(mask), KEEP);
            assert_eq!(wire(&req), expected, "idempotence for mask {mask}");
        }
        assert_eq!(wire(&original), before);
        assert_eq!(wire(&sibling), before);
    }

    #[test]
    fn bidder_owned_scrubs_do_not_modify_the_original_or_sibling() {
        let original = loaded_request();
        let mut restricted = original.clone();
        let mut sibling = original.clone();
        apply_scrubs(&mut restricted, decisions(u8::MAX), KEEP);
        apply_scrubs(
            &mut sibling,
            ScrubDecisions {
                transmit_tid: false,
                ..ALLOW
            },
            KEEP,
        );
        assert_eq!(
            wire(&restricted),
            expected_wire(true, true, true, true, true)
        );
        assert_eq!(
            wire(&sibling),
            expected_wire(false, false, false, false, true)
        );
        assert_eq!(
            wire(&original),
            expected_wire(false, false, false, false, false)
        );
    }

    #[test]
    fn missing_objects_stay_absent_under_every_decision() {
        for input in [
            json!({"id": "empty"}),
            json!({"id": "empty", "user": {}, "device": {}, "source": {},
                   "imp": [{"id": "no-ext"}, {"id": "empty-ext", "ext": {}}]}),
            json!({"id": "empty", "user": {"ext": {}}, "device": {"ext": {}}}),
            json!({"id": "empty", "user": {"geo": {}}, "device": {"geo": {}}}),
        ] {
            let original: BidRequest =
                serde_json::from_value(input.clone()).expect("sparse fixture");
            for mask in 0_u8..=u8::MAX {
                let mut req = original.clone();
                apply_scrubs(&mut req, decisions(mask), KEEP);
                assert_eq!(wire(&req), input, "sparse fixture, mask {mask}");
            }
        }
    }

    #[test]
    fn coppa_full_geo_dominates_coarsening_in_either_order() {
        for coppa_first in [false, true] {
            let mut req = loaded_request();
            if coppa_first {
                scrub_coppa(&mut req, KEEP);
            }
            scrub_geo_and_device_ip(&mut req, KEEP);
            scrub_ccpa_lmt(&mut req, KEEP);
            if !coppa_first {
                scrub_coppa(&mut req, KEEP);
            }
            assert_eq!(wire(&req), expected_wire(true, true, true, true, false));
        }
    }

    fn add_geo_shadows(req: &mut BidRequest, shadow_only: bool) {
        let user = req.user.as_mut().expect("user");
        let device = req.device.as_mut().expect("device");
        for (geo, other) in [
            (&mut user.geo, &mut user.other),
            (&mut device.geo, &mut device.other),
        ] {
            let mut shadow = full_geo(-1.125, 1.125);
            shadow["country"] = json!("CAN");
            shadow["shadow_context"] = json!("must not survive geo denial");
            other.insert("geo".to_owned(), shadow);
            if shadow_only {
                *geo = None;
            }
        }
    }

    #[test]
    fn coppa_clears_typed_and_shadow_geo_objects() {
        let mut req = loaded_request();
        add_geo_shadows(&mut req, false);
        scrub_coppa(&mut req, KEEP);
        assert_eq!(wire(&req), expected_wire(true, true, true, true, false));
        assert!(!req.user.as_ref().expect("user").other.contains_key("geo"));
        assert!(
            !req.device
                .as_ref()
                .expect("device")
                .other
                .contains_key("geo")
        );
    }

    #[test]
    fn coppa_empties_existing_shadow_only_geo_without_fabricating_typed_geo() {
        let mut req = loaded_request();
        add_geo_shadows(&mut req, true);
        let expected = expected_wire(true, true, true, true, false);
        for _ in 0_u8..2 {
            scrub_coppa(&mut req, KEEP);
            scrub_geo_and_device_ip(&mut req, KEEP);
            assert_eq!(wire(&req), expected);
            assert!(req.user.as_ref().expect("user").geo.is_none());
            assert!(req.device.as_ref().expect("device").geo.is_none());
        }
    }

    #[test]
    fn partial_geo_prefers_typed_geo_and_retains_reviewed_context() {
        for mask in [2_u8, 8, 32, 64] {
            let mut req = loaded_request();
            add_geo_shadows(&mut req, false);
            apply_scrubs(&mut req, decisions(mask), KEEP);
            assert_eq!(
                wire(&req),
                expected_wire(mask & 0b0110_0000 != 0, false, true, false, false),
                "typed geo with shadow, mask {mask}",
            );
        }
    }

    #[test]
    fn partial_geo_retains_reviewed_context_and_coarse_coordinates_from_shadows() {
        for mask in [2_u8, 8, 32, 64] {
            let mut req = loaded_request();
            add_geo_shadows(&mut req, true);
            let ids = mask & 0b0110_0000 != 0;
            let mut expected = expected_wire(ids, false, true, false, false);
            for parent in ["user", "device"] {
                expected[parent]["geo"] = reviewed_geo("CAN", Some(-1.12_f64), Some(1.13_f64));
            }
            for _ in 0_u8..2 {
                apply_scrubs(&mut req, decisions(mask), KEEP);
                assert_eq!(wire(&req), expected, "shadow-only geo, mask {mask}");
            }
        }
    }

    #[test]
    fn null_geo_stays_null_but_invalid_containers_and_coordinates_are_removed() {
        for shadow in [
            json!(null),
            json!("invalid"),
            json!([{"lat": 1.125_f64}]),
            json!(true),
        ] {
            for mask in [2_u8, 128] {
                let mut req = loaded_request();
                let user = req.user.as_mut().expect("user");
                user.geo = None;
                user.other.insert("geo".to_owned(), shadow.clone());
                let device = req.device.as_mut().expect("device");
                device.geo = None;
                device.other.insert("geo".to_owned(), shadow.clone());
                let mut expected = expected_wire(mask == 128, mask == 128, true, false, false);
                for owner in ["/user", "/device"] {
                    if shadow.is_null() {
                        expected.pointer_mut(owner).expect("owner")["geo"] = Value::Null;
                    } else {
                        remove_fields(&mut expected, owner, &["geo"]);
                    }
                }
                apply_scrubs(&mut req, decisions(mask), KEEP);
                assert_eq!(wire(&req), expected, "invalid shadow {shadow}, mask {mask}");
            }
        }
        for (lat, lon, expected_geo) in [
            (
                json!("-1.125"),
                json!({"unexpected": 1.125_f64}),
                reviewed_geo("CAN", None, None),
            ),
            (
                json!(null),
                json!(1.125_f64),
                reviewed_geo("CAN", None, Some(1.13_f64)),
            ),
            (
                json!(-1.125_f64),
                json!(false),
                reviewed_geo("CAN", Some(-1.12_f64), None),
            ),
            (
                json!([1.125_f64]),
                json!(true),
                reviewed_geo("CAN", None, None),
            ),
        ] {
            let mut req = loaded_request();
            add_geo_shadows(&mut req, true);
            for other in [
                &mut req.user.as_mut().expect("user").other,
                &mut req.device.as_mut().expect("device").other,
            ] {
                let shadow = other.get_mut("geo").expect("shadow geo");
                shadow["lat"] = lat.clone();
                shadow["lon"] = lon.clone();
            }
            let mut expected = expected_wire(false, false, true, false, false);
            for parent in ["user", "device"] {
                expected[parent]["geo"] = expected_geo.clone();
            }
            for _ in 0_u8..2 {
                scrub_geo_and_device_ip(&mut req, KEEP);
                assert_eq!(wire(&req), expected, "malformed coordinates {lat}/{lon}");
            }
        }
    }

    fn add_shadows(req: &mut BidRequest) {
        let before = wire(req);
        let user = req.user.as_mut().expect("user");
        for key in [
            "id", "buyeruid", "yob", "gender", "eids", "data", "keywords",
        ] {
            user.other
                .insert(key.to_owned(), before["user"][key].clone());
        }
        let user_ext = user.ext.as_mut().expect("user ext");
        for key in ["eids", "data"] {
            user_ext
                .other
                .insert(key.to_owned(), before["user"]["ext"][key].clone());
        }
        let device = req.device.as_mut().expect("device");
        for key in ["ifa", "ip", "ipv6"] {
            device
                .other
                .insert(key.to_owned(), before["device"][key].clone());
        }
        for (geo, parent) in [
            (user.geo.as_mut().expect("user geo"), "user"),
            (device.geo.as_mut().expect("device geo"), "device"),
        ] {
            for key in ["lat", "lon"] {
                geo.other
                    .insert(key.to_owned(), before[parent]["geo"][key].clone());
            }
        }
        req.source
            .as_mut()
            .expect("source")
            .other
            .insert("tid".to_owned(), json!("source-tid"));
        for imp in &mut req.imp {
            if let Some(ext) = imp.ext.as_mut() {
                ext.other.insert("tid".to_owned(), json!(ext.tid));
            }
        }
    }

    #[test]
    fn same_field_shadows_cannot_bypass_any_of_the_256_decisions() {
        for mask in 0_u8..=u8::MAX {
            let mut req = loaded_request();
            add_shadows(&mut req);
            let expected = expected_wire(
                mask & 0b1110_0101 != 0,
                mask & (4 | 128) != 0,
                mask & 0b1110_1010 != 0,
                mask & 128 != 0,
                mask & 16 != 0,
            );
            for _ in 0_u8..2 {
                apply_scrubs(&mut req, decisions(mask), KEEP);
                assert_eq!(wire(&req), expected, "shadow fields, decision mask {mask}");
            }
        }
    }

    #[test]
    fn shadow_only_ids_fpd_and_tids_are_removed() {
        let mut req = loaded_request();
        add_shadows(&mut req);
        let user = req.user.as_mut().expect("user");
        user.id = None;
        user.buyeruid = None;
        user.yob = None;
        user.gender = None;
        user.eids.clear();
        user.data = None;
        user.keywords = None;
        let user_ext = user.ext.as_mut().expect("user ext");
        user_ext.eids = None;
        user_ext.data = None;
        req.device.as_mut().expect("device").ifa = None;
        req.source.as_mut().expect("source").tid = None;
        for imp in &mut req.imp {
            if let Some(ext) = imp.ext.as_mut() {
                ext.tid = None;
            }
        }
        scrub_user_fpd(&mut req);
        scrub_tid(&mut req);
        assert_eq!(wire(&req), expected_wire(true, true, false, false, true));
    }

    #[test]
    fn shadow_only_ip_and_coordinates_use_the_same_masking_and_rounding() {
        let mut req = loaded_request();
        add_shadows(&mut req);
        let user = req.user.as_mut().expect("user");
        let device = req.device.as_mut().expect("device");
        device.ip = None;
        device.ipv6 = None;
        for geo in [
            user.geo.as_mut().expect("user geo"),
            device.geo.as_mut().expect("device geo"),
        ] {
            geo.lat = None;
            geo.lon = None;
        }
        let expected = expected_wire(false, false, true, false, false);
        scrub_geo_and_device_ip(&mut req, KEEP);
        assert_eq!(wire(&req), expected);
    }

    #[test]
    fn identifier_primitive_removes_denied_shadows_but_preserves_owned_fpd() {
        let mut req = loaded_request();
        add_shadows(&mut req);
        let user = req.user.as_mut().expect("user");
        for (key, value) in [
            ("uid2", json!("unclassified-id")),
            ("sha256email", json!("unclassified-hash")),
            (
                "metadata",
                json!({"id": "nested-id", "data": "unclassified-fpd"}),
            ),
        ] {
            user.other.insert(key.to_owned(), value.clone());
            user.ext
                .as_mut()
                .expect("user ext")
                .other
                .insert(key.to_owned(), value);
        }
        for _ in 0_u8..2 {
            scrub_gdpr_id(&mut req);
            assert_eq!(wire(&req), expected_wire(true, false, false, false, false));
            let scrubbed_user = req.user.as_ref().expect("user");
            assert_eq!(scrubbed_user.other.len(), 3);
            for key in ["data", "keywords", "kwarray"] {
                assert!(
                    scrubbed_user.other.contains_key(key),
                    "permitted FPD shadow {key}"
                );
            }
            let ext_other = &scrubbed_user.ext.as_ref().expect("user ext").other;
            assert_eq!(ext_other.len(), 1);
            assert!(ext_other.contains_key("data"));
        }
    }

    fn add_ext_shadows(req: &mut BidRequest, shadow_only: bool) {
        let user = req.user.as_mut().expect("user");
        user.other.insert(
            "ext".to_owned(),
            json!({
                "eids": [{"source": "shadow.test", "uids": [{"id": "shadow-user-eid"}]}],
                "data": {"private": "shadow-user-fpd"}, "consent": "shadow-consent",
                "metadata": {"eids": ["opaque-eid"], "data": "opaque-data", "tid": "opaque-tid"}
            }),
        );
        if shadow_only {
            user.ext = None;
        }
        for imp in req.imp.iter_mut().take(2) {
            imp.other.insert(
                "ext".to_owned(),
                json!({
                    "tid": "shadow-imp-tid", "data": {"context": "shadow-imp-data"},
                    "bidder": {"placementId": 42_i32, "tid": "opaque-bidder-tid"},
                    "metadata": {"eids": ["opaque-eid"], "data": "opaque-data", "tid": "opaque-tid"}
                }),
            );
            if shadow_only {
                imp.ext = None;
            }
        }
    }

    fn expected_ext_shadow_wire(ids: bool, fpd: bool, tid: bool) -> Value {
        let mut fixture = loaded_request();
        add_ext_shadows(&mut fixture, false);
        let shadows = wire(&fixture);
        let mut expected = expected_wire(ids, fpd, false, false, tid);
        if ids {
            expected["user"]["ext"] = if fpd {
                json!({})
            } else {
                json!({"data": {"private": "shadow-user-fpd"}})
            };
        } else {
            expected["user"]["ext"] = shadows["user"]["ext"].clone();
        }
        for path in ["/imp/0/ext", "/imp/1/ext"] {
            *expected.pointer_mut(path).expect("extension") =
                shadows.pointer(path).expect("shadow").clone();
        }
        if tid {
            remove_fields(&mut expected, "/imp/0/ext", &["tid"]);
            remove_fields(&mut expected, "/imp/1/ext", &["tid"]);
        }
        expected
    }

    // Also serialize a copy without container shadows: last-key-wins JSON parsing
    // must not conceal a protected leaf or changes to permitted typed context.
    fn assert_ext_wire_views(
        req: &BidRequest,
        mut expected_typed: Value,
        expected_shadow: &Value,
        shadow_only: bool,
    ) {
        assert_eq!(&wire(req), expected_shadow);
        let mut typed = req.clone();
        typed.user.as_mut().expect("user").other.remove("ext");
        for imp in &mut typed.imp {
            imp.other.remove("ext");
        }
        if shadow_only {
            remove_fields(&mut expected_typed, "/user", &["ext"]);
            remove_fields(&mut expected_typed, "/imp/0", &["ext"]);
            remove_fields(&mut expected_typed, "/imp/1", &["ext"]);
        }
        assert_eq!(wire(&typed), expected_typed);
    }

    #[test]
    fn id_scrub_closes_shadow_ids_but_keeps_permitted_typed_and_shadow_data() {
        for shadow_only in [false, true] {
            let mut req = loaded_request();
            add_ext_shadows(&mut req, shadow_only);
            let expected = expected_ext_shadow_wire(true, false, false);
            for _ in 0_u8..2 {
                scrub_gdpr_id(&mut req);
                assert_ext_wire_views(
                    &req,
                    expected_wire(true, false, false, false, false),
                    &expected,
                    shadow_only,
                );
            }
        }
    }

    #[test]
    fn fpd_scrub_removes_owned_typed_and_shadow_data_without_normalizing_containers() {
        for shadow_only in [false, true] {
            let mut req = loaded_request();
            add_ext_shadows(&mut req, shadow_only);
            let expected = expected_ext_shadow_wire(true, true, false);
            for _ in 0_u8..2 {
                scrub_user_fpd(&mut req);
                assert_ext_wire_views(
                    &req,
                    expected_wire(true, true, false, false, false),
                    &expected,
                    shadow_only,
                );
            }
        }
    }

    #[test]
    fn tid_scrub_removes_every_imp_ext_shadow_tid_but_keeps_params_and_metadata() {
        for shadow_only in [false, true] {
            let mut req = loaded_request();
            add_ext_shadows(&mut req, shadow_only);
            let expected = expected_ext_shadow_wire(false, false, true);
            for _ in 0_u8..2 {
                scrub_tid(&mut req);
                assert_ext_wire_views(
                    &req,
                    expected_wire(false, false, false, false, true),
                    &expected,
                    shadow_only,
                );
            }
        }
    }

    #[test]
    fn id_denial_closes_unknown_shadow_descendants_without_rounding_permitted_geo() {
        let mut req = loaded_request();
        add_geo_shadows(&mut req, true);
        add_ext_shadows(&mut req, true);
        let mut expected = expected_ext_shadow_wire(true, false, false);
        for parent in ["user", "device"] {
            let mut geo = reviewed_geo("CAN", Some(-1.125_f64), Some(1.125_f64));
            geo["accuracy"] = json!(5_i32);
            geo["zip"] = json!("94107");
            expected[parent]["geo"] = geo;
        }
        for _ in 0_u8..2 {
            scrub_gdpr_id(&mut req);
            assert_eq!(wire(&req), expected);
            let user = req.user.as_ref().expect("user");
            assert_eq!(user.other.len(), 3);
            for key in ["geo", "ext", "kwarray"] {
                assert!(user.other.contains_key(key), "permitted user shadow {key}");
            }
            assert!(user.geo.is_none());
            assert!(user.ext.is_none());
            let device = req.device.as_ref().expect("device");
            assert_eq!(device.other.len(), 1);
            assert!(device.other.contains_key("geo"));
            assert!(device.geo.is_none());
        }
    }

    #[test]
    fn invalid_ext_shadows_close_under_the_owning_scrub_with_scoped_null_controls() {
        for malformed in [
            json!(null),
            json!(false),
            json!(42_i32),
            json!("invalid"),
            json!([{"eids": "id", "data": "fpd", "tid": "tid"}]),
        ] {
            for shadow_only in [false, true] {
                for (ids, fpd, tid) in [
                    (true, false, false),
                    (true, true, false),
                    (false, false, true),
                ] {
                    let mut req = loaded_request();
                    add_ext_shadows(&mut req, shadow_only);
                    req.user
                        .as_mut()
                        .expect("user")
                        .other
                        .insert("ext".to_owned(), malformed.clone());
                    for imp in req.imp.iter_mut().take(2) {
                        imp.other.insert("ext".to_owned(), malformed.clone());
                    }
                    let expected_typed = expected_wire(ids, fpd, false, false, tid);
                    let mut expected = expected_typed.clone();
                    for (parent, path, scrubbed) in [
                        ("/user", "/user/ext", ids),
                        ("/imp/0", "/imp/0/ext", tid),
                        ("/imp/1", "/imp/1/ext", tid),
                    ] {
                        let scoped_null = parent == "/user" && malformed.is_null();
                        if !scrubbed || scoped_null {
                            *expected.pointer_mut(path).expect("extension") = malformed.clone();
                        }
                        if scrubbed && shadow_only && !scoped_null {
                            remove_fields(&mut expected, parent, &["ext"]);
                        }
                    }
                    apply_scrubs(
                        &mut req,
                        ScrubDecisions {
                            pass_id: !ids,
                            transmit_ufpd: !fpd,
                            transmit_tid: !tid,
                            ..ALLOW
                        },
                        KEEP,
                    );
                    assert_ext_wire_views(&req, expected_typed, &expected, shadow_only);
                }
            }
        }
    }

    #[test]
    fn missing_ext_stays_absent_and_owned_denials_empty_existing_shadow_objects() {
        let mut req: BidRequest = serde_json::from_value(json!({
            "id": "missing-ext", "user": {}, "imp": [{"id": "imp-1"}, {"id": "imp-2"}]
        }))
        .expect("missing extension fixture");
        let before = wire(&req);
        scrub_user_fpd(&mut req);
        scrub_tid(&mut req);
        assert_eq!(wire(&req), before);
        req.user
            .as_mut()
            .expect("user")
            .other
            .insert("ext".to_owned(), json!({"eids": [], "data": {}}));
        req.imp[0]
            .other
            .insert("ext".to_owned(), json!({"tid": "shadow-tid"}));
        scrub_user_fpd(&mut req);
        scrub_tid(&mut req);
        assert_eq!(
            wire(&req),
            json!({
                "id": "missing-ext", "user": {"ext": {}},
                "imp": [{"id": "imp-1", "ext": {}}, {"id": "imp-2"}]
            })
        );
    }

    #[test]
    fn malformed_ip_is_dropped_in_serialized_output_for_every_geo_policy() {
        for (v4, v6) in [
            ("999.1.2.3", "zzzz::1"),
            ("", ""),
            (" 192.168.1.1", "2001:db8::1 "),
            ("2001:db8::1", "192.168.1.1"),
        ] {
            for mask in [2_u8, 8, 32, 64, 128] {
                let mut req = loaded_request();
                let device = req.device.as_mut().expect("device");
                device.ip = Some(v4.to_owned());
                device.ipv6 = Some(v6.to_owned());
                device.other.insert("ip".to_owned(), json!(v4));
                device.other.insert("ipv6".to_owned(), json!(v6));
                let mut expected = expected_wire(
                    mask & 0b1110_0000 != 0,
                    mask == 128,
                    true,
                    mask == 128,
                    false,
                );
                remove_fields(&mut expected, "/device", &["ip", "ipv6"]);
                apply_scrubs(&mut req, decisions(mask), KEEP);
                assert_eq!(wire(&req), expected, "malformed IP, decision mask {mask}");
            }
        }
    }

    #[test]
    fn serialized_ip_masks_preserve_keep_bit_bounds_and_clamping() {
        for (v4, v6, expected_v4, expected_v6) in [
            (0, 0, "0.0.0.0", "::"),
            (1, 1, "128.0.0.0", "::"),
            (24, 56, "192.168.1.0", "2001:db8:85a3:1200::"),
            (
                31,
                127,
                "192.168.1.0",
                "2001:db8:85a3:1234:5678:8a2e:370:7334",
            ),
            (
                32,
                128,
                "192.168.1.1",
                "2001:db8:85a3:1234:5678:8a2e:370:7335",
            ),
            (
                33,
                129,
                "192.168.1.1",
                "2001:db8:85a3:1234:5678:8a2e:370:7335",
            ),
            (
                255,
                255,
                "192.168.1.1",
                "2001:db8:85a3:1234:5678:8a2e:370:7335",
            ),
        ] {
            let mut req = loaded_request();
            let mut expected = expected_wire(false, false, true, false, false);
            expected["device"]["ip"] = json!(expected_v4);
            expected["device"]["ipv6"] = json!(expected_v6);
            apply_scrubs(
                &mut req,
                ScrubDecisions {
                    pass_geo: false,
                    ..ALLOW
                },
                IpKeepBits { v4, v6 },
            );
            assert_eq!(wire(&req), expected, "keep bits {v4}/{v6}");
        }
    }

    #[test]
    fn serialized_rounding_preserves_negative_half_up_and_repeated_results() {
        for (input, rounded) in [
            (-1.125_f64, -1.12_f64),
            (1.125_f64, 1.13_f64),
            (-1.238_f64, -1.24_f64),
            (-1.232_f64, -1.23_f64),
            (1.238_f64, 1.24_f64),
            (1.232_f64, 1.23_f64),
            (1.005_f64, 1.0_f64),
        ] {
            let mut req: BidRequest = serde_json::from_value(json!({
                "id": "rounding", "user": {"geo": {"lat": input, "lon": input}},
                "device": {"geo": {"lat": input, "lon": input}}
            }))
            .expect("rounding fixture");
            let expected = json!({
                "id": "rounding", "user": {"geo": {"lat": rounded, "lon": rounded}},
                "device": {"geo": {"lat": rounded, "lon": rounded}}
            });
            for _ in 0_u8..3 {
                apply_scrubs(
                    &mut req,
                    ScrubDecisions {
                        pass_geo: false,
                        ..ALLOW
                    },
                    KEEP,
                );
                assert_eq!(wire(&req), expected, "rounding {input}");
            }
        }
    }

    #[test]
    fn gdpr_wrapper_does_not_activate_new_policies_from_request_signals() {
        for pass_id in [false, true] {
            for pass_geo in [false, true] {
                let mut req = loaded_request();
                apply_gdpr_scrubs(&mut req, pass_id, pass_geo, KEEP);
                assert_eq!(
                    wire(&req),
                    expected_wire(!pass_id, false, !pass_geo, false, false)
                );
            }
        }
    }

    #[test]
    fn allow_all_is_byte_identical_even_with_shadow_fields() {
        for shadow_only in [false, true] {
            let mut req = loaded_request();
            add_shadows(&mut req);
            add_ext_shadows(&mut req, shadow_only);
            add_geo_shadows(&mut req, shadow_only);
            let before = serde_json::to_vec(&req).expect("request serializes");
            apply_scrubs(&mut req, ALLOW, KEEP);
            assert_eq!(
                serde_json::to_vec(&req).expect("request serializes"),
                before
            );
        }
    }

    #[test]
    fn scrubs_match_pinned_prebid_except_named_privacy_policy_differences() {
        let reference: Value = serde_json::from_str(include_str!(
            "../../../auctioneer-privacy/tests/data/privacy/reference.json"
        ))
        .expect("pinned reference fixtures");
        let rows = reference["scrubs"].as_array().unwrap();
        assert_eq!(rows.len(), 9);
        for row in rows {
            let name = row["name"].as_str().unwrap();
            let mut request: BidRequest = serde_json::from_value(row["input"].clone()).unwrap();
            match name {
                "gdpr_id" | "native_eids" => scrub_gdpr_id(&mut request),
                "fpd" => scrub_user_fpd(&mut request),
                "geo" | "negative_geo_once" => scrub_geo_and_device_ip(&mut request, KEEP),
                "negative_geo_twice" => {
                    scrub_geo_and_device_ip(&mut request, KEEP);
                    scrub_geo_and_device_ip(&mut request, KEEP);
                }
                "ccpa_lmt" => scrub_ccpa_lmt(&mut request, KEEP),
                "coppa" => scrub_coppa(&mut request, KEEP),
                "tid" => scrub_tid(&mut request),
                _ => panic!("unhandled reference scrub: {name}"),
            }
            let mut expected = row["reference_output"].clone();
            match name {
                "fpd" => remove_fields(&mut expected, "/user/ext", &["eids"]),
                "native_eids" => remove_fields(&mut expected, "/user", &["eids"]),
                "coppa" => {
                    // Approved Auctioneer COPPA FPD differential; keep the reference untouched.
                    assert_eq!(row["kind"], "intentional-coppa-user-fpd-difference");
                    for path in [
                        "/user/data",
                        "/user/keywords",
                        "/user/kwarray",
                        "/user/ext/data",
                    ] {
                        assert!(row["reference_output"].pointer(path).is_some());
                    }
                    remove_fields(&mut expected, "/user", &["data", "keywords", "kwarray"]);
                    remove_fields(&mut expected, "/user/ext", &["data"]);
                }
                "negative_geo_once" | "negative_geo_twice" => {
                    expected["user"]["geo"]["lat"] = json!(-1.24_f64);
                }
                _ => assert_eq!(row["kind"], "parity"),
            }
            // Protected boundaries close unknown extension context under either
            // gate, but retain reviewed Geo city and independently allowed kwarray.
            match name {
                "gdpr_id" | "native_eids" | "fpd" | "ccpa_lmt" | "coppa" | "geo"
                | "negative_geo_once" | "negative_geo_twice" => {
                    assert_eq!(row["input"]["user"]["ext"]["context"], "keep");
                    remove_fields(&mut expected, "/user/ext", &["context"]);
                }
                _ => {}
            }
            match name {
                "fpd" | "coppa" => {
                    assert!(row["input"]["user"]["kwarray"].is_array());
                    assert!(expected["user"].get("kwarray").is_none());
                }
                "tid" => {}
                _ => assert_eq!(expected["user"]["kwarray"], json!(["interest"])),
            }
            if name != "coppa" {
                assert_eq!(expected["user"]["geo"]["city"], "city");
            }
            assert_eq!(wire(&request), expected, "reference scrub: {name}");
        }
    }
}

/// Production classification at the consuming projection boundary.
#[cfg(test)]
mod boundary_proof {
    use crate::fake_bidder::{self, FakeBidder};

    use crate::resolve_aliases;
    use crate::split::{group_by_bidder, materialize_per_bidder};
    use auctioneer_bidder::BidderRegistry;
    use auctioneer_config_data::types::IpKeepBits;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_openrtb::types::device::Geo;
    use serde_json::{Value, json};
    use std::collections::{BTreeMap, BTreeSet};

    fn input() -> Value {
        json!({
            "id": "request", "site": {"id": "publisher"},
            "imp": [{"id": "imp", "banner": {}, "ext": {"bidder": {"fake_bb": {}}}}],
            "user": {
                "id": "private-user", "kwarray": ["news", "sport"],
                "geo": {"lat": 51.5074_f64, "lon": -0.1278_f64,
                    "country": "GBR", "city": "London", "metro": "known-market",
                    "region": "LND", "regionfips104": "UKH9", "type": 2_i32,
                    "utcoffset": 60_i32, "ipservice": 3_i32, "lastfix": 12_i32,
                    "zip": "precise-postcode", "accuracy": 1_i32,
                    "unknown": {"uid2": "secret"}}
            },
            "device": {
                "make": "vendor", "h": null, "pxratio": 2_i32, "dnt": 1_i32, "lmt": 0_i32,
                "ext": {"atts": 2_i32, "prebid": {"interstitial": {
                    "minwidthperc": 70_i32, "minheightperc": 60_i32,
                    "unknown": {"uid2": "secret"}}, "unknown": "secret"}},
                "sua": {"browsers": [{}, {"brand": "browser", "version": ["1"],
                    "ext": {"uid2": "secret"}, "unknown": "secret"}],
                    "platform": {}, "ext": {"uid2": "secret"}, "unknown": "secret"}
            }
        })
    }

    fn scoped(value: &Value) -> BidRequest {
        let original: BidRequest = serde_json::from_value(value.clone()).unwrap();
        original.validate().unwrap();
        let mut registry = BidderRegistry::new();
        registry
            .register(fake_bidder::info(), Box::new(FakeBidder::default()))
            .expect("unique test bidder");
        let disabled = BTreeSet::new();
        let aliases = resolve_aliases(&original, &registry, &disabled, &BTreeMap::new()).unwrap();
        let grouped = group_by_bidder(
            &original,
            &aliases.map,
            &registry,
            &disabled,
            &aliases.dropped_alias_names,
        )
        .unwrap();
        let survivors = grouped.bidder_imps().keys().cloned().collect::<Vec<_>>();
        let mut scoped = materialize_per_bidder(
            &grouped,
            &survivors,
            &aliases.map,
            &registry,
            &aliases.dropped_alias_names,
        )
        .unwrap();
        assert_eq!(scoped.len(), 1);
        assert_eq!(serde_json::to_value(&original).unwrap(), *value);
        scoped.pop().unwrap().request
    }

    fn wire(request: &BidRequest) -> Value {
        serde_json::from_slice(&serde_json::to_vec(request).unwrap()).unwrap()
    }

    fn scrub_fixture(request: &mut BidRequest, pass_id: bool, pass_geo: bool) {
        let mut audit = super::DisclosureAudit::default();
        super::apply_classified(
            request,
            super::ScrubDecisions {
                pass_id,
                pass_geo,
                transmit_ufpd: true,
                transmit_precise_geo: true,
                transmit_tid: true,
                ccpa: false,
                lmt: false,
                coppa: false,
            },
            IpKeepBits { v4: 24, v6: 56 },
            &mut audit,
        )
        .unwrap();
        assert_eq!(audit.attempted_scopes, 1);
        assert_eq!(audit.failed_scopes, 0);
    }

    #[test]
    fn scoped_geo_context_survives_identifier_and_location_denial() {
        for (pass_id, pass_geo) in [(false, true), (true, false), (false, false)] {
            let value = input();
            let mut request = scoped(&value);
            let before = wire(&request);
            scrub_fixture(&mut request, pass_id, pass_geo);
            let after = wire(&request);
            for key in [
                "country",
                "city",
                "metro",
                "region",
                "regionfips104",
                "utcoffset",
                "type",
                "ipservice",
                "lastfix",
            ] {
                assert_eq!(after["user"]["geo"][key], before["user"]["geo"][key]);
            }
            assert!(after["user"]["geo"].get("unknown").is_none());
            assert_eq!(after["user"]["kwarray"], json!(["news", "sport"]));
            if pass_geo {
                assert_eq!(after["user"]["geo"]["lat"], json!(51.5074_f64));
            } else {
                assert_eq!(after["user"]["geo"]["lat"], json!(51.51_f64));
                assert_eq!(after["user"]["geo"]["lon"], json!(-0.13_f64));
                assert!(after["user"]["geo"].get("zip").is_none());
                assert!(after["user"]["geo"].get("accuracy").is_none());
            }
        }
    }

    #[test]
    fn context_shapes_are_checked_before_unknown_subtree_retention() {
        for (pass_id, pass_geo) in [(false, true), (true, false), (false, false)] {
            let mut value = input();
            value["user"]["geo"]["city"] = json!({"uid2": "secret"});
            value["user"]["geo"]["metro"] = json!(["known", {"uid2": "secret"}]);
            value["user"]["geo"]["utcoffset"] = json!("secret");
            value["user"]["kwarray"] = json!(["known", {"uid2": "secret"}]);
            let mut request = scoped(&value);
            assert!(wire(&request)["user"]["geo"].get("city").is_some());
            scrub_fixture(&mut request, pass_id, pass_geo);
            let after = wire(&request);
            for key in ["city", "metro", "utcoffset"] {
                assert!(after["user"]["geo"].get(key).is_none());
            }
            assert!(after["user"].get("kwarray").is_none());
        }
    }

    #[test]
    fn restricted_descendants_preserve_interstitial_and_sparse_brand_context() {
        for (pass_id, pass_geo) in [(false, true), (true, false), (false, false)] {
            let mut request = scoped(&input());
            scrub_fixture(&mut request, pass_id, pass_geo);
            let device = request.device.as_ref().unwrap();
            let sua = device.sua.as_ref().unwrap();
            assert!(sua.browsers.as_ref().unwrap()[0].fields().is_none());
            assert!(sua.platform.as_ref().unwrap().fields().is_none());
            let after = wire(&request);
            assert_eq!(
                after["device"]["sua"]["browsers"],
                json!([{}, {"brand": "browser", "version": ["1"]}])
            );
            assert_eq!(
                after["device"]["ext"],
                json!({"atts": 2_i32, "prebid": {"interstitial": {
                "minwidthperc": 70_i32, "minheightperc": 60_i32}}})
            );
        }
    }

    #[test]
    fn unrestricted_model_bytes_preserve_nulls_numeric_forms_and_raw_extensions() {
        for pxratio in [json!(2_i32), json!(2.0_f64)] {
            let mut value = input();
            value["device"]["pxratio"] = pxratio;
            let mut request = scoped(&value);
            let before = serde_json::to_vec(&request).unwrap();
            scrub_fixture(&mut request, true, true);
            assert_eq!(serde_json::to_vec(&request).unwrap(), before);
            assert_eq!(wire(&request)["device"]["h"], Value::Null);
        }
    }

    #[test]
    fn restricted_model_keeps_explicit_nulls_and_distinct_empty_brand_states() {
        for (pass_id, pass_geo) in [(false, true), (true, false), (false, false)] {
            let mut value = input();
            value["device"]["sua"]["platform"] = Value::Null;
            value["device"]["sua"]["browsers"] = json!([{}, {"brand": null, "version": []}]);
            let mut request = scoped(&value);
            let before = wire(&request);
            assert_eq!(before["device"].get("h"), Some(&Value::Null));
            assert_eq!(before["device"]["sua"].get("platform"), Some(&Value::Null));
            scrub_fixture(&mut request, pass_id, pass_geo);
            let after = wire(&request);
            assert_eq!(after["device"].get("h"), Some(&Value::Null));
            assert_eq!(after["device"]["sua"].get("platform"), Some(&Value::Null));
            assert_eq!(
                after["device"]["sua"].get("browsers"),
                before["device"]["sua"].get("browsers")
            );
            let browsers = request
                .device
                .as_ref()
                .unwrap()
                .sua
                .as_ref()
                .unwrap()
                .browsers
                .as_ref()
                .unwrap();
            assert!(browsers[0].fields().is_none());
            assert!(browsers[1].fields().is_some());
        }
    }

    #[test]
    fn shadow_geo_uses_context_shapes_and_geo_only_typed_precedence() {
        for pass_id in [false, true] {
            let mut request = scoped(&input());
            let user = request.user.as_mut().unwrap();
            user.geo = None;
            user.other.insert(
                "geo".into(),
                json!({"country": "GBR", "city": "London",
                "lat": 51.5074_f64, "unknown": "secret"}),
            );
            scrub_fixture(&mut request, pass_id, false);
            assert_eq!(
                wire(&request)["user"]["geo"],
                json!({"country": "GBR", "city": "London", "lat": 51.51_f64})
            );
        }
        let mut request = scoped(&input());
        request
            .user
            .as_mut()
            .unwrap()
            .other
            .insert("geo".into(), json!({"city": "shadow"}));
        scrub_fixture(&mut request, false, true);
        assert_eq!(
            request.user.as_ref().unwrap().other["geo"]["city"],
            json!("shadow")
        );
        scrub_fixture(&mut request, false, false);
        assert!(!request.user.as_ref().unwrap().other.contains_key("geo"));
    }

    #[test]
    fn in_place_mutation_removes_nonfinite_and_overflow_coordinates() {
        for coordinate in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MAX] {
            let mut request = scoped(&input());
            request.user.as_mut().unwrap().geo = Some(Geo {
                country: Some("GBR".into()),
                lat: Some(coordinate),
                lon: Some(coordinate),
                ..Geo::default()
            });
            scrub_fixture(&mut request, true, false);
            let geo = request.user.as_ref().unwrap().geo.as_ref().unwrap();
            assert!(geo.lat.is_none());
            assert!(geo.lon.is_none());
            assert_eq!(geo.country.as_deref(), Some("GBR"));
        }
    }
}
