//! Offline acceptance of reviewed differences, never a live dual-engine path.
//! Goldens below are synthetic `json!` inputs and independently authored outputs.
//! The frozen comparator is committed privacy enforcement source, not the changing working tree.
//!
//! Cutover must revise old country-only Geo expectations in
//! `scrub::tests::denied_geo_removes_unclassified_fields_and_extensions`,
//! `scrub::composition_tests::partial_geo_retains_only_country_and_coarse_coordinates_from_surviving_shadows`,
//! the kwarray/country-only oracle in `scrub::composition_tests`, and
//! `scrub::tests::geo_rounding_leaves_non_finite_untouched`.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

#[cfg(test)]
mod frozen_scrub;

#[cfg(test)]
mod tests {
    use super::frozen_scrub;
    use crate::fake_bidder::{self, FakeBidder};
    use auctioneer_bidder::{
        Bidder as _, BidderName, BidderRegistry, BidderRequestInfo, CallBudget, EntryPoint,
        OutboundRequestSink, SinkLimits,
    };
    use auctioneer_config_data::types::IpKeepBits;
    use auctioneer_exchange::{
        resolve_aliases,
        scrub::{DisclosureAudit, ScrubDecisions, apply_classified},
        split::{group_by_bidder, materialize_per_bidder},
        validate_endpoint::ExchangeEndpointValidator,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_runtime::count_json_nodes;
    use serde_json::{Value, json};
    use std::collections::{BTreeMap, BTreeSet};
    use std::panic::catch_unwind;

    const KEEP: IpKeepBits = IpKeepBits { v4: 24, v6: 56 };
    const BYTE_CAP: usize = 0x4000;
    const NODE_CAP: u32 = 1_024;
    const MODES: [(&str, bool, bool); 4] = [
        ("allow-id-allow-geo", true, true),
        ("allow-id-deny-geo", true, false),
        ("deny-id-allow-geo", false, true),
        ("deny-id-deny-geo", false, false),
    ];

    fn decisions(pass_id: bool, pass_geo: bool) -> ScrubDecisions {
        ScrubDecisions {
            pass_id,
            pass_geo,
            transmit_ufpd: true,
            transmit_precise_geo: true,
            transmit_tid: true,
            ccpa: false,
            lmt: false,
            coppa: false,
        }
    }

    // Never use assert_eq! on payloads, serde errors, or adapter errors. Labels
    // are fixed call-site literals; neither field names nor values enter reports.
    fn equal(label: &'static str, actual: &Value, expected: &Value) {
        assert!(actual == expected, "{label}");
    }

    fn bytes(request: &BidRequest, label: &'static str) -> Vec<u8> {
        let bytes = serde_json::to_vec(request).unwrap_or_else(|_| panic!("{label}"));
        assert!(bytes.len() <= BYTE_CAP, "{label}");
        assert!(count_json_nodes(&bytes, NODE_CAP).is_ok(), "{label}");
        bytes
    }

    fn wire(request: &BidRequest, label: &'static str) -> Value {
        serde_json::from_slice(&bytes(request, label)).unwrap_or_else(|_| panic!("{label}"))
    }

    fn decode(value: &Value, label: &'static str) -> BidRequest {
        let input = serde_json::to_vec(value).unwrap_or_else(|_| panic!("{label}"));
        assert!(input.len() <= BYTE_CAP, "{label}");
        assert!(count_json_nodes(&input, NODE_CAP).is_ok(), "{label}");
        let request: BidRequest =
            serde_json::from_slice(&input).unwrap_or_else(|_| panic!("{label}"));
        assert!(request.validate().is_ok(), "{label}");
        request
    }

    fn owners(user: Value, device: Value) -> Value {
        let mut request = json!({
            "id": "offline-request",
            "imp": [{"id": "offline-imp", "banner": {"w": 300_i32, "h": 250_i32}}],
            "site": {"id": "site-context"}
        });
        request["user"] = user;
        request["device"] = device;
        request
    }

    fn pair(
        source: &BidRequest,
        decision: ScrubDecisions,
        label: &'static str,
    ) -> (BidRequest, BidRequest) {
        let before = bytes(source, label);
        let mut old = source.clone();
        let mut new = source.clone();
        frozen_scrub::apply_scrubs(
            &mut old,
            frozen_scrub::ScrubDecisions {
                pass_id: decision.pass_id,
                pass_geo: decision.pass_geo,
                transmit_ufpd: decision.transmit_ufpd,
                transmit_precise_geo: decision.transmit_precise_geo,
                transmit_tid: decision.transmit_tid,
                ccpa: decision.ccpa,
                lmt: decision.lmt,
                coppa: decision.coppa,
            },
            KEEP,
        );
        let mut audit = DisclosureAudit::default();
        assert!(
            apply_classified(&mut new, decision, KEEP, &mut audit).is_ok(),
            "{label}"
        );
        assert!(bytes(source, label) == before, "{label}");
        (old, new)
    }

    fn remove(value: &mut Value, path: &str, keys: &[&str]) {
        let object = value
            .pointer_mut(path)
            .and_then(Value::as_object_mut)
            .expect("fixed-golden-object");
        for key in keys {
            object.remove(*key);
        }
    }

    fn control_fixture() -> Value {
        json!({
            "id": "offline-request", "at": 2_i32,
            "imp": [{"id": "offline-imp", "banner": {"w": 300_i32, "h": 250_i32},
                "ext": {"tid": "imp-transaction", "bidder": {"placementId": 42_i32},
                    "data": {"context": "impression"}}}],
            "site": {"id": "site-context", "keywords": "news",
                "ext": {"data": {"context": "site"}}},
            "source": {"tid": "source-transaction", "pchain": "payment-context"},
            "user": {
                "id": "synthetic-user", "buyeruid": "synthetic-buyer",
                "yob": 1984_i32, "gender": "F", "consent": "synthetic-consent",
                "keywords": "news", "data": [{"id": "synthetic-audience"}],
                "eids": [{"source": "synthetic.test", "uids": [{"id": "synthetic-eid"}]}],
                "ext": {"data": {"context": "news"},
                    "eids": [{"source": "legacy.test", "uids": [{"id": "synthetic-legacy"}]}]},
                "geo": {"country": "GBR", "lat": 51.507_351_f64, "lon": -0.127_758_f64}
            },
            "device": {
                "ifa": "synthetic-device", "didmd5": "synthetic-hash",
                "ip": "192.0.2.129", "ipv6": "2001:db8:1234:5678::1",
                "ua": "synthetic-agent", "dnt": 1_i32, "lmt": 1_i32, "os": "Android",
                "make": "Example", "model": "Handset", "devicetype": 4_i32,
                "osv": "15", "hwv": "rev-2", "h": null, "w": 400_i64,
                "ppi": 300_i64, "pxratio": 2_i32, "js": 1_i32, "geofetch": 0_i32,
                "flashver": "0", "language": "en", "langb": "en-GB",
                "carrier": "Example", "mccmnc": "234-10", "connectiontype": 2_i32,
                "sua": {"browsers": [{}, {}, {"brand": "Example", "version": ["1", "2"]}],
                    "platform": {}, "mobile": 1_i32, "architecture": "arm",
                    "bitness": "64", "model": "Handset", "source": 1_i32},
                "geo": {"country": "USA", "lat": 48.858_844_f64, "lon": 2.294_351_f64},
                "ext": {"atts": 2_i32, "prebid": {"interstitial": {
                    "minwidthperc": 70_i64, "minheightperc": 60_i64}}}
            }
        })
    }

    // Keep the previous policy golden separate from the approved COPPA amendment.
    // Neither oracle reads classified output, generated rules, or the frozen scrub.
    fn previous_control_golden(decision: ScrubDecisions) -> Value {
        let mut expected = control_fixture();
        let id_denied = !decision.pass_id
            || !decision.transmit_ufpd
            || decision.ccpa
            || decision.lmt
            || decision.coppa;
        let geo_denied = !decision.pass_geo
            || !decision.transmit_precise_geo
            || decision.ccpa
            || decision.lmt
            || decision.coppa;
        if id_denied {
            remove(
                &mut expected,
                "/user",
                &["id", "buyeruid", "yob", "gender", "eids"],
            );
            remove(&mut expected, "/user/ext", &["eids"]);
            remove(&mut expected, "/device", &["ifa", "didmd5"]);
        }
        if !decision.transmit_ufpd {
            remove(&mut expected, "/user", &["data", "keywords"]);
            remove(&mut expected, "/user/ext", &["data"]);
        }
        if geo_denied {
            expected["user"]["geo"] = json!({"country": "GBR", "lat": 51.51_f64, "lon": -0.13_f64});
            expected["device"]["geo"] =
                json!({"country": "USA", "lat": 48.86_f64, "lon": 2.29_f64});
            expected["device"]["ip"] = json!("192.0.2.0");
            expected["device"]["ipv6"] = json!("2001:db8:1234:5600::");
        }
        if decision.coppa {
            expected["user"]["geo"] = json!({});
            expected["device"]["geo"] = json!({});
        }
        if !decision.transmit_tid {
            remove(&mut expected, "/source", &["tid"]);
            remove(&mut expected, "/imp/0/ext", &["tid"]);
        }
        expected
    }

    fn current_control_golden(decision: ScrubDecisions) -> Value {
        let mut expected = previous_control_golden(decision);
        if decision.coppa {
            remove(&mut expected, "/user", &["data", "keywords"]);
            remove(&mut expected, "/user/ext", &["data"]);
        }
        expected
    }

    #[test]
    fn unchanged_controls_match_frozen_scrub_in_all_four_id_geo_modes() {
        let source = decode(&control_fixture(), "control-input");
        for (label, pass_id, pass_geo) in MODES {
            let decision = decisions(pass_id, pass_geo);
            let (old, new) = pair(&source, decision, label);
            let expected = current_control_golden(decision);
            equal(label, &wire(&old, label), &expected);
            equal(label, &wire(&new, label), &expected);
            assert!(bytes(&old, label) == bytes(&new, label), "{label}");
            if pass_id && pass_geo {
                assert!(bytes(&new, label) == bytes(&source, label), "{label}");
            }
            let (_, twice) = pair(&new, decision, label);
            assert!(bytes(&new, label) == bytes(&twice, label), "{label}");
        }
    }

    #[test]
    fn all_256_composed_decisions_match_separate_previous_and_current_goldens() {
        const LABEL: &str = "composed-control-matrix";
        let source = decode(&control_fixture(), LABEL);
        for mask in 0_u16..256_u16 {
            let decision = ScrubDecisions {
                pass_id: mask & 1 != 0,
                pass_geo: mask & 2 != 0,
                transmit_ufpd: mask & 4 != 0,
                transmit_precise_geo: mask & 8 != 0,
                transmit_tid: mask & 16 != 0,
                ccpa: mask & 32 != 0,
                lmt: mask & 64 != 0,
                coppa: mask & 128 != 0,
            };
            let (old, new) = pair(&source, decision, LABEL);
            let old_expected = previous_control_golden(decision);
            let new_expected = current_control_golden(decision);
            equal(LABEL, &wire(&old, LABEL), &old_expected);
            equal(LABEL, &wire(&new, LABEL), &new_expected);
            if decision.coppa && decision.transmit_ufpd {
                assert!(
                    bytes(&old, LABEL) != bytes(&new, LABEL),
                    "coppa-user-fpd-policy-differential"
                );
            } else {
                assert!(bytes(&old, LABEL) == bytes(&new, LABEL), "{LABEL}");
            }
        }
    }

    #[test]
    fn coppa_removes_all_user_fpd_while_frozen_scrub_preserved_three_branches() {
        const LABEL: &str = "coppa-user-fpd-policy-differential";
        let mut input = control_fixture();
        input["user"]["kwarray"] = json!(["news", "sports"]);
        let source = decode(&input, LABEL);
        let decision = ScrubDecisions {
            coppa: true,
            ..decisions(true, true)
        };
        let old_expected = previous_control_golden(decision);
        let new_expected = current_control_golden(decision);
        let (old, new) = pair(&source, decision, LABEL);
        // The frozen ID scrub already removed kwarray, but retained the other FPD.
        equal(LABEL, &wire(&old, LABEL), &old_expected);
        equal(LABEL, &wire(&new, LABEL), &new_expected);
        assert!(bytes(&old, LABEL) != bytes(&new, LABEL), "{LABEL}");
        let (_, repeated) = pair(&new, decision, LABEL);
        assert!(bytes(&new, LABEL) == bytes(&repeated, LABEL), "{LABEL}");
    }

    #[test]
    fn fpd_only_denial_removes_kwarray_without_coarsening_geo_or_removing_tid() {
        const LABEL: &str = "fpd-only-control";
        let mut input = control_fixture();
        input["user"]["kwarray"] = json!(["news", "sports"]);
        let source = decode(&input, LABEL);
        let decision = ScrubDecisions {
            transmit_ufpd: false,
            ..decisions(true, true)
        };
        let (old, new) = pair(&source, decision, LABEL);
        let expected = current_control_golden(decision);
        equal(LABEL, &wire(&old, LABEL), &expected);
        equal(LABEL, &wire(&new, LABEL), &expected);
        assert!(bytes(&old, LABEL) == bytes(&new, LABEL), "{LABEL}");
    }

    #[test]
    fn tid_only_denial_leaves_every_protected_payload_and_unknown_untouched() {
        const LABEL: &str = "tid-only-control";
        let mut input = control_fixture();
        input["user"]["kwarray"] = json!(["news"]);
        input["user"]["unknown"] = json!({"uid2": "synthetic-private"});
        input["device"]["geo"]["city"] = json!("Example City");
        input["device"]["sua"]["ext"] = json!({"uid2": "synthetic-raw"});
        let mut expected = input.clone();
        remove(&mut expected, "/source", &["tid"]);
        remove(&mut expected, "/imp/0/ext", &["tid"]);
        let source = decode(&input, LABEL);
        let decision = ScrubDecisions {
            transmit_tid: false,
            ..decisions(true, true)
        };
        let (old, new) = pair(&source, decision, LABEL);
        equal(LABEL, &wire(&old, LABEL), &expected);
        equal(LABEL, &wire(&new, LABEL), &expected);
        assert!(bytes(&old, LABEL) == bytes(&new, LABEL), "{LABEL}");
    }

    fn geo_context() -> Value {
        json!({
            "country": "USA", "region": "CA", "regionfips104": "06",
            "city": "San Francisco", "metro": "807", "type": 1_i32,
            "utcoffset": -480_i32, "ipservice": 3_i32, "lastfix": 2_i32
        })
    }

    #[test]
    fn all_nine_geo_context_fields_survive_restriction_in_both_locations() {
        const LABEL: &str = "nine-geo-context-fields";
        let mut geo = geo_context();
        geo["lat"] = json!(1.125_f64);
        geo["lon"] = json!(-1.125_f64);
        let input = owners(json!({"geo": geo.clone()}), json!({"geo": geo}));
        let source = decode(&input, LABEL);
        for (label, pass_id, pass_geo) in MODES {
            let (old, new) = pair(&source, decisions(pass_id, pass_geo), label);
            let mut expected = input.clone();
            if !pass_geo {
                for owner in ["user", "device"] {
                    expected[owner]["geo"]["lat"] = json!(1.13_f64);
                    expected[owner]["geo"]["lon"] = json!(-1.12_f64);
                }
            }
            equal(LABEL, &wire(&new, LABEL), &expected);
            let mut old_expected = expected.clone();
            if !pass_geo {
                for owner in ["user", "device"] {
                    old_expected[owner]["geo"] =
                        json!({"country": "USA", "lat": 1.13_f64, "lon": -1.12_f64});
                }
            }
            equal(LABEL, &wire(&old, LABEL), &old_expected);
        }
    }

    #[test]
    fn id_only_denial_retains_kwarray_but_frozen_scrub_dropped_it() {
        const LABEL: &str = "kwarray-retained-id-only";
        let input = owners(
            json!({"id": "synthetic-user", "kwarray": ["news", "sports"],
            "keywords": "news", "consent": "synthetic-consent"}),
            json!({}),
        );
        let source = decode(&input, LABEL);
        for (label, pass_id, pass_geo) in MODES {
            let (old, new) = pair(&source, decisions(pass_id, pass_geo), label);
            let mut expected = input.clone();
            if !pass_id {
                remove(&mut expected, "/user", &["id"]);
            }
            equal(LABEL, &wire(&new, LABEL), &expected);
            if !pass_id {
                remove(&mut expected, "/user", &["kwarray"]);
            }
            equal(LABEL, &wire(&old, LABEL), &expected);
        }
    }

    #[test]
    fn geo_denial_removes_zip_and_original_accuracy_in_both_locations() {
        const LABEL: &str = "zip-accuracy-removal";
        let geo = json!({"country": "USA", "lat": 1.238_f64, "lon": -1.238_f64,
            "zip": "synthetic-postcode", "accuracy": 5_i32});
        let input = owners(json!({"geo": geo.clone()}), json!({"geo": geo}));
        let source = decode(&input, LABEL);
        for (label, pass_id, pass_geo) in MODES {
            let (old, new) = pair(&source, decisions(pass_id, pass_geo), label);
            let mut expected = input.clone();
            if !pass_geo {
                for owner in ["user", "device"] {
                    expected[owner]["geo"] =
                        json!({"country": "USA", "lat": 1.24_f64, "lon": -1.24_f64});
                }
            }
            equal(LABEL, &wire(&old, LABEL), &expected);
            equal(LABEL, &wire(&new, LABEL), &expected);
        }
    }

    fn unknown_fixture() -> Value {
        let geo = json!({"country": "USA", "lat": 1.238_f64, "lon": -1.238_f64,
            "unknown": {"uid2": "synthetic-geo"}, "ext": {"uid2": "synthetic-raw-geo"}});
        owners(
            json!({"id": "synthetic-user", "unknown": {"uid2": "synthetic-user-unknown"},
                "geo": geo.clone(), "ext": {"unknown": {"uid2": "synthetic-user-ext"},
                    "eids": [{"source": "legacy.test", "unknown": "synthetic-legacy-unknown",
                        "uids": [{"id": "synthetic-legacy", "unknown": "synthetic-legacy-uid",
                            "ext": {"uid2": "synthetic-legacy-ext"}}]}]},
                "eids": [{"source": "synthetic.test", "unknown": "synthetic-eid-unknown",
                    "uids": [{"id": "synthetic-eid", "atype": 1_i32,
                        "unknown": "synthetic-uid-unknown", "ext": {"uid2": "synthetic-uid-ext"}}]}]}),
            json!({"ifa": "synthetic-device", "unknown": {"uid2": "synthetic-device-unknown"},
                "geo": geo, "ext": {"atts": 2_i32, "unknown": {"uid2": "synthetic-device-ext"},
                    "prebid": {"unknown": "synthetic-prebid", "interstitial": {
                        "minwidthperc": 70_i64, "minheightperc": 60_i64,
                        "unknown": {"uid2": "synthetic-interstitial"}}}},
                "sua": {"unknown": "synthetic-sua", "ext": {"uid2": "synthetic-sua-ext"},
                    "browsers": [{}, {"brand": "Example", "version": ["1"],
                        "unknown": "synthetic-brand", "ext": {"uid2": "synthetic-brand-ext"}}],
                    "platform": {"brand": "Android", "unknown": "synthetic-platform",
                        "ext": {"uid2": "synthetic-platform-ext"}}}}),
        )
    }

    #[test]
    fn unknowns_and_raw_descendants_close_under_either_id_or_geo_denial() {
        const LABEL: &str = "protected-unknown-descendants";
        let input = unknown_fixture();
        let source = decode(&input, LABEL);
        for (label, pass_id, pass_geo) in MODES {
            let (old, new) = pair(&source, decisions(pass_id, pass_geo), label);
            let mut expected = owners(
                json!({"geo": {"country": "USA", "lat": 1.238_f64, "lon": -1.238_f64}, "ext": {}}),
                json!({"geo": {"country": "USA", "lat": 1.238_f64, "lon": -1.238_f64},
                    "ext": {"atts": 2_i32, "prebid": {"interstitial": {
                        "minwidthperc": 70_i64, "minheightperc": 60_i64}}},
                    "sua": {"browsers": [{}, {"brand": "Example", "version": ["1"]}],
                        "platform": {"brand": "Android"}}}),
            );
            if pass_id {
                expected["user"]["id"] = json!("synthetic-user");
                expected["device"]["ifa"] = json!("synthetic-device");
                expected["user"]["eids"] = json!([{"source": "synthetic.test",
                    "uids": [{"id": "synthetic-eid", "atype": 1_i32}]}]);
                expected["user"]["ext"]["eids"] = json!([{"source": "legacy.test",
                    "uids": [{"id": "synthetic-legacy"}]}]);
            }
            if !pass_geo {
                for owner in ["user", "device"] {
                    expected[owner]["geo"] =
                        json!({"country": "USA", "lat": 1.24_f64, "lon": -1.24_f64});
                }
            }
            if pass_id && pass_geo {
                expected = input.clone();
            }
            equal(LABEL, &wire(&new, LABEL), &expected);

            let mut old_expected = input.clone();
            if !pass_id {
                remove(&mut old_expected, "/user", &["id", "unknown", "eids"]);
                remove(&mut old_expected, "/user/ext", &["unknown", "eids"]);
                remove(&mut old_expected, "/device", &["ifa", "unknown"]);
                remove(&mut old_expected, "/device/ext", &["unknown"]);
                remove(&mut old_expected, "/device/ext/prebid", &["unknown"]);
                remove(
                    &mut old_expected,
                    "/device/ext/prebid/interstitial",
                    &["unknown"],
                );
                for path in [
                    "/device/sua",
                    "/device/sua/browsers/1",
                    "/device/sua/platform",
                ] {
                    remove(&mut old_expected, path, &["unknown", "ext"]);
                }
            }
            if !pass_geo {
                for owner in ["user", "device"] {
                    old_expected[owner]["geo"] =
                        json!({"country": "USA", "lat": 1.24_f64, "lon": -1.24_f64});
                }
            }
            equal(LABEL, &wire(&old, LABEL), &old_expected);
        }
    }

    #[test]
    fn nonfinite_and_finite_overflow_typed_coordinates_are_removed_not_serialized_as_null() {
        const LABEL: &str = "invalid-typed-coordinate-removal";
        for coordinate in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::MAX,
            f64::MIN,
        ] {
            let mut source = decode(
                &owners(
                    json!({"geo": {"country": "USA"}}),
                    json!({"geo": {"country": "USA"}}),
                ),
                LABEL,
            );
            for geo in [
                source.user.as_mut().unwrap().geo.as_mut().unwrap(),
                source.device.as_mut().unwrap().geo.as_mut().unwrap(),
            ] {
                geo.lat = Some(coordinate);
                geo.lon = Some(coordinate);
            }
            for pass_id in [true, false] {
                let (old, new) = pair(&source, decisions(pass_id, false), LABEL);
                for geo in [
                    old.user.as_ref().unwrap().geo.as_ref().unwrap(),
                    old.device.as_ref().unwrap().geo.as_ref().unwrap(),
                ] {
                    for retained in [geo.lat.unwrap(), geo.lon.unwrap()] {
                        if coordinate.is_finite() {
                            assert!(retained.is_infinite(), "{LABEL}");
                        } else {
                            assert!(retained.to_bits() == coordinate.to_bits(), "{LABEL}");
                        }
                    }
                }
                for geo in [
                    new.user.as_ref().unwrap().geo.as_ref().unwrap(),
                    new.device.as_ref().unwrap().geo.as_ref().unwrap(),
                ] {
                    assert!(geo.lat.is_none() && geo.lon.is_none(), "{LABEL}");
                }
                equal(
                    LABEL,
                    &wire(&new, LABEL),
                    &owners(
                        json!({"geo": {"country": "USA"}}),
                        json!({"geo": {"country": "USA"}}),
                    ),
                );
                assert!(
                    source
                        .user
                        .as_ref()
                        .unwrap()
                        .geo
                        .as_ref()
                        .unwrap()
                        .lat
                        .unwrap()
                        .to_bits()
                        == coordinate.to_bits(),
                    "{LABEL}"
                );
                assert!(
                    source
                        .device
                        .as_ref()
                        .unwrap()
                        .geo
                        .as_ref()
                        .unwrap()
                        .lon
                        .unwrap()
                        .to_bits()
                        == coordinate.to_bits(),
                    "{LABEL}"
                );
            }
        }
    }

    #[test]
    fn shadow_only_geo_retains_reviewed_context_instead_of_country_only() {
        const LABEL: &str = "shadow-only-geo-context";
        let mut source = decode(&owners(json!({}), json!({})), LABEL);
        let mut shadow = geo_context();
        shadow["lat"] = json!(1.125_f64);
        shadow["lon"] = json!(-1.125_f64);
        shadow["zip"] = json!("synthetic-postcode");
        shadow["accuracy"] = json!(5_i32);
        source
            .user
            .as_mut()
            .unwrap()
            .other
            .insert("geo".to_owned(), shadow.clone());
        source
            .device
            .as_mut()
            .unwrap()
            .other
            .insert("geo".to_owned(), shadow);
        let (old, new) = pair(&source, decisions(true, false), LABEL);
        let mut expected_geo = geo_context();
        expected_geo["lat"] = json!(1.13_f64);
        expected_geo["lon"] = json!(-1.12_f64);
        equal(
            LABEL,
            &wire(&new, LABEL),
            &owners(
                json!({"geo": expected_geo.clone()}),
                json!({"geo": expected_geo}),
            ),
        );
        let old_geo = json!({"country": "USA", "lat": 1.13_f64, "lon": -1.12_f64});
        equal(
            LABEL,
            &wire(&old, LABEL),
            &owners(json!({"geo": old_geo.clone()}), json!({"geo": old_geo})),
        );
    }

    #[test]
    fn malformed_context_and_keyword_array_impostors_are_not_retained() {
        const LABEL: &str = "malformed-context-removal";
        let geo = json!({"country": "USA", "city": {"uid2": "synthetic-hidden"},
            "metro": ["synthetic-hidden"], "lastfix": 2.5_f64, "type": true});
        let input = owners(
            json!({"geo": geo.clone(), "kwarray": [{"uid2": "synthetic-hidden"}]}),
            json!({"geo": geo}),
        );
        let source = decode(&input, LABEL);
        for decision in [
            decisions(false, true),
            decisions(true, false),
            decisions(false, false),
        ] {
            let (_, new) = pair(&source, decision, LABEL);
            equal(
                LABEL,
                &wire(&new, LABEL),
                &owners(
                    json!({"geo": {"country": "USA"}}),
                    json!({"geo": {"country": "USA"}}),
                ),
            );
        }
    }

    #[test]
    fn null_kwarray_is_retained_for_id_only_but_removed_for_fpd_only() {
        const LABEL: &str = "null-keyword-array-control";
        let input = owners(json!({"kwarray": null}), json!({}));
        let source = decode(&input, LABEL);
        let (old, new) = pair(&source, decisions(false, true), LABEL);
        equal(LABEL, &wire(&new, LABEL), &input);
        equal(LABEL, &wire(&old, LABEL), &owners(json!({}), json!({})));
        let fpd = ScrubDecisions {
            transmit_ufpd: false,
            ..decisions(true, true)
        };
        let (old_fpd, new_fpd) = pair(&source, fpd, LABEL);
        equal(LABEL, &wire(&old_fpd, LABEL), &owners(json!({}), json!({})));
        equal(LABEL, &wire(&new_fpd, LABEL), &owners(json!({}), json!({})));
    }

    #[test]
    fn dnt_lmt_interstitial_and_sparse_empty_brands_remain_controls() {
        const LABEL: &str = "privacy-and-sparse-context-controls";
        for population in [0_usize, 64_usize, 512_usize] {
            let input = owners(
                json!({}),
                json!({"dnt": 1_i32, "lmt": 1_i32,
                "sua": {"browsers": vec![json!({}); population], "platform": {}},
                "ext": {"atts": 2_i32, "prebid": {"interstitial": {
                    "minwidthperc": null, "minheightperc": 60_i64}}}}),
            );
            let source = decode(&input, LABEL);
            for (label, pass_id, pass_geo) in MODES {
                let (old, new) = pair(&source, decisions(pass_id, pass_geo), label);
                equal(LABEL, &wire(&old, LABEL), &input);
                equal(LABEL, &wire(&new, LABEL), &input);
                for request in [&source, &old, &new] {
                    let device = request.device.as_ref().unwrap();
                    assert!(device.geo.is_none(), "{LABEL}");
                    let sua = device.sua.as_ref().unwrap();
                    let browsers = sua.browsers.as_ref().unwrap();
                    assert!(browsers.len() == population, "{LABEL}");
                    assert!(
                        browsers.iter().all(|brand| brand.fields().is_none()),
                        "{LABEL}"
                    );
                    assert!(sua.platform.as_ref().unwrap().fields().is_none(), "{LABEL}");
                }
            }
        }
    }

    #[test]
    fn committed_model_corpus_matches_independent_control_goldens() {
        let fixtures = [
            (
                "corpus-device-context",
                include_str!(
                    "../../auctioneer-openrtb/tests/fixtures/device-context-number-and-null.json"
                ),
            ),
            (
                "corpus-native-eid-missing-uids",
                include_str!(
                    "../../auctioneer-bidder/tests/conformance/fixtures/openrtb/user-eids-uids-missing.json"
                ),
            ),
            (
                "corpus-legacy-eids-empty",
                include_str!(
                    "../../auctioneer-bidder/tests/conformance/fixtures/openrtb/user-ext-eids-empty.json"
                ),
            ),
        ];
        for (label, text) in fixtures {
            let fixture: Value = serde_json::from_str(text).unwrap_or_else(|_| panic!("{label}"));
            let input = fixture.get("mockBidRequest").unwrap_or(&fixture).clone();
            let source = decode(&input, label);
            for (_, pass_id, pass_geo) in MODES {
                let (old, new) = pair(&source, decisions(pass_id, pass_geo), label);
                let mut expected = input.clone();
                if !pass_id && expected.get("user").is_some() {
                    remove(&mut expected, "/user", &["eids"]);
                    if expected.pointer("/user/ext").is_some() {
                        remove(&mut expected, "/user/ext", &["eids"]);
                    }
                }
                equal(label, &wire(&old, label), &expected);
                equal(label, &wire(&new, label), &expected);
                assert!(bytes(&old, label) == bytes(&new, label), "{label}");
            }
        }
    }

    fn fake_b_body(request: &BidRequest, cap: usize) -> Option<Vec<u8>> {
        const LABEL: &str = "fake_bb-capped-serialization";
        let config = fake_bidder::info();
        assert_eq!(config.name, "fake_bb", "synthetic fixture identity");
        let Some(endpoint) = config.endpoint.as_deref() else {
            panic!("synthetic fixture requires a compiled endpoint");
        };
        let info = BidderRequestInfo::new(
            BidderName::new("fake_bb"),
            BidderName::new("fake_bb"),
            "synthetic-account",
            None,
            endpoint,
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        let budget = CallBudget::new(1);
        let validator = ExchangeEndpointValidator::new(config);
        let mut sink = OutboundRequestSink::new(SinkLimits::new(cap, 1), &budget, &validator);
        assert!(
            FakeBidder::default()
                .make_requests(request, &info, &mut sink)
                .is_ok(),
            "{LABEL}"
        );
        let mut outcome = sink.into_outcome();
        assert!(
            outcome.suppressed_error_count == 0 && outcome.rejected_call_count == 0,
            "{LABEL}"
        );
        if let Some(accepted) = outcome.accepted.pop() {
            assert!(
                outcome.errors.is_empty() && outcome.accepted.is_empty(),
                "{LABEL}"
            );
            assert!(accepted.body().len() <= cap, "{LABEL}");
            assert!(
                count_json_nodes(accepted.body(), NODE_CAP).is_ok(),
                "{LABEL}"
            );
            Some(accepted.into_parts().1)
        } else {
            assert!(
                outcome.errors.len() == 1 && budget.remaining() == 1,
                "{LABEL}"
            );
            None
        }
    }

    #[test]
    fn scoped_fake_emits_reviewed_diff_through_capped_sink() {
        const LABEL: &str = "scoped-fake_bb-difference";
        let mut input = owners(
            json!({"id": "synthetic-user", "kwarray": ["news"]}),
            json!({}),
        );
        input["imp"][0]["ext"] = json!({"bidder": {"fake_bb": {"placementId": 42_i32}}});
        let source = decode(&input, LABEL);
        let config = fake_bidder::info();
        let mut registry = BidderRegistry::new();
        registry
            .register(config, Box::new(FakeBidder::default()))
            .expect("unique test bidder");
        let disabled = BTreeSet::new();
        let aliases = resolve_aliases(&source, &registry, &disabled, &BTreeMap::new())
            .unwrap_or_else(|_| panic!("{LABEL}"));
        let grouped = group_by_bidder(
            &source,
            &aliases.map,
            &registry,
            &disabled,
            &aliases.dropped_alias_names,
        )
        .unwrap_or_else(|_| panic!("{LABEL}"));
        let survivors = vec![BidderName::new("fake_bb")];
        let scoped = materialize_per_bidder(
            &grouped,
            &survivors,
            &aliases.map,
            &registry,
            &aliases.dropped_alias_names,
        )
        .unwrap_or_else(|_| panic!("{LABEL}"));
        assert!(scoped.len() == 1, "{LABEL}");
        let scope = &scoped[0].request;
        let (old, new) = pair(scope, decisions(false, true), LABEL);
        let mut expected = owners(json!({"kwarray": ["news"]}), json!({}));
        expected["imp"][0]["ext"] = json!({"bidder": {"placementId": 42_i32}});
        let new_body = fake_b_body(&new, BYTE_CAP).expect("fake_bb-new-body-present");
        equal(
            LABEL,
            &serde_json::from_slice(&new_body).unwrap_or_else(|_| panic!("{LABEL}")),
            &expected,
        );
        remove(&mut expected, "/user", &["kwarray"]);
        let old_body = fake_b_body(&old, BYTE_CAP).expect("fake_bb-old-body-present");
        equal(
            LABEL,
            &serde_json::from_slice(&old_body).unwrap_or_else(|_| panic!("{LABEL}")),
            &expected,
        );
        assert!(old_body != new_body, "{LABEL}");
        assert!(fake_b_body(&new, new_body.len()).is_some(), "{LABEL}");
        assert!(
            fake_b_body(&new, new_body.len().checked_sub(1).unwrap()).is_none(),
            "{LABEL}"
        );
        let (allowed_old, allowed_new) = pair(scope, decisions(true, true), LABEL);
        let original_body = fake_b_body(scope, BYTE_CAP).expect("fake_bb-source-body-present");
        assert!(
            fake_b_body(&allowed_old, BYTE_CAP).as_ref() == Some(&original_body),
            "{LABEL}"
        );
        assert!(
            fake_b_body(&allowed_new, BYTE_CAP).as_ref() == Some(&original_body),
            "{LABEL}"
        );
        assert!(
            scope.user.as_ref().unwrap().id.as_deref() == Some("synthetic-user"),
            "{LABEL}"
        );
        assert!(
            source.user.as_ref().unwrap().id.as_deref() == Some("synthetic-user"),
            "{LABEL}"
        );
    }

    #[test]
    fn mismatch_diagnostics_report_only_fixed_label_not_payload_or_unknown_keys() {
        const LABEL: &str = "fixed-diagnostic-fixture";
        const SECRET: &str = "never-report-synthetic-secret";
        let payload = json!({"never-report-synthetic-key": {"uid2": SECRET}});
        equal(LABEL, &payload, &payload);
        let panic = catch_unwind(|| equal(LABEL, &payload, &json!({})))
            .expect_err("diagnostic-mismatch-must-fail");
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied())
            .expect("fixed-panic-text");
        assert!(message == LABEL, "fixed-diagnostic-message");
        assert!(
            !message.contains(SECRET) && !message.contains("never-report-synthetic-key"),
            "fixed-diagnostic-no-payload"
        );
    }
}
