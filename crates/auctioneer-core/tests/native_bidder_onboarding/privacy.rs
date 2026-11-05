//! Funded native outbound privacy acceptance. The coordinator calls the entrypoint
//! from the existing boot-once onboarding case; this module never boots another App.

use super::fixtures::{self, Flow};
use super::harness::{self, ObservedCall, Secrets};
use auctioneer_config_data::types::MediaType;
use auctioneer_privacy::{gpp::Limits as GppLimits, sources::normalize};
use edgezero_core::app::App;
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeSet,
    io::{Write as _, stderr},
};

const IDS: &[&str] = &[
    "private-user-id",
    "private-buyer-id",
    "native-eid",
    "legacy-eid",
    "private-ifa",
    "private-dpidsha1",
    "private-dpidmd5",
    "private-didsha1",
    "private-didmd5",
    "private-macsha1",
    "private-macmd5",
    "private-uid2",
    "private-email",
];
const FPD: &[&str] = &[
    "private-keywords",
    "private-keyword-array",
    "private-segment",
    "private-audience",
];
const ID_PATHS: &[&str] = &[
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

#[derive(Clone, Copy, Default)]
struct Policy {
    coppa: bool,
    lmt: bool,
    sale: bool,
    disable_ccpa: bool,
    no_sale: bool,
    malformed_usp: bool,
    deny_ufpd: bool,
    deny_geo: bool,
    deny_tid: bool,
}

impl Policy {
    fn ids_denied(self) -> bool {
        self.coppa || self.lmt || self.deny_ufpd || self.ccpa()
    }

    fn ccpa(self) -> bool {
        (self.sale || self.malformed_usp) && !self.disable_ccpa && !self.no_sale
    }

    fn coarse(self) -> bool {
        self.coppa || self.lmt || self.ccpa() || self.deny_geo
    }
}

#[test]
fn selected_nonempty_malformed_gpp_usp_is_fail_closed_not_a_legacy_fallback() {
    let input = serde_json::from_value(json!({"id":"fake_usp_selection","imp":[],
        "regs":{"gpp":"DBACNY~opaque-tcf~invalid-selected-usp","gpp_sid":[6_i32,2_i32,6_i32],
            "us_privacy":"1YNN"}}))
    .unwrap();
    let selected = normalize(&input, GppLimits::default()).unwrap();
    assert_eq!(selected.active_tcf(), Some("opaque-tcf"));
    assert!(
        selected.usp_is_malformed(),
        "active GPP section 6 wins over valid permissive legacy USP"
    );
    assert!(
        selected.active_usp().is_none(),
        "malformed USP must never be backfilled as valid consent"
    );
    let malformed = Policy {
        malformed_usp: true,
        ..Policy::default()
    };
    assert!(malformed.ids_denied());
    assert!(malformed.coarse());
    assert!(
        !Policy {
            disable_ccpa: true,
            ..malformed
        }
        .ids_denied()
    );
    assert!(
        !Policy {
            no_sale: true,
            ..malformed
        }
        .ids_denied()
    );
}

fn geo() -> Value {
    json!({"lat":48.858_844_f64,"lon":-1.125_f64,"country":"USA","region":"CA",
        "city":"San Francisco","zip":"94107","accuracy":7,"type":1,
        "ext":{"location":"private-location"},"extra_geo":"private-extra-location"})
}

fn request(flows: &[&Flow], inventory: &str, policy: Policy) -> Value {
    assert!(IDS.len() > 12);
    assert!(FPD.len() > 3);
    let mut input = fixtures::input(flows);
    input.as_object_mut().unwrap().remove("site");
    input.as_object_mut().unwrap().remove("app");
    input[inventory] = json!({"id":"contextual-inventory","publisher":{"id":flows[0].account()}});
    input["source"] = json!({"tid":"private-source-tid","ext":{"schain":{
        "ver":"1.0","complete":1_i32,"nodes":[{"asi":"seller.example","sid":"seller","hp":1_i32}]}}});
    input["user"] = json!({"id":IDS[0],"buyeruid":IDS[1],"yob":1_980_i32,"gender":"F",
        "keywords":FPD[0],"kwarray":[FPD[1]],"data":[{"id":FPD[2]}],
        "eids":[{"source":"native.example","uids":[{"id":IDS[2]}]}],
        "ext":{"eids":[{"source":"legacy.example","uids":[{"id":IDS[3]}]}],
            "data":{"audience":FPD[3]}},"geo":geo()});
    input["device"] = json!({"ip":"8.8.4.123","ipv6":"2001:4860:4860::8844",
        "lmt":i32::from(policy.lmt),"make":"Example","model":"Handset","devicetype":4_i32,
        "ifa":IDS[4],"dpidsha1":IDS[5],"dpidmd5":IDS[6],"didsha1":IDS[7],"didmd5":IDS[8],
        "macsha1":IDS[9],"macmd5":IDS[10],"uid2":IDS[11],"metadata":{"sha256email":IDS[12]},
        "geo":geo(),"ext":{"context":"private-device-context"}});
    let usp = if policy.malformed_usp {
        "invalid-selected-usp"
    } else if policy.sale {
        "1YYN"
    } else {
        "1YNN"
    };
    input["regs"] = json!({"coppa":i32::from(policy.coppa),"gpp":format!("DBACNY~opaque-tcf~{usp}"),
        "gpp_sid":[6_i32,2_i32,6_i32],"us_privacy":"invalid-legacy-usp"});
    for imp in input["imp"].as_array_mut().unwrap() {
        imp["ext"]["tid"] = json!("private-imp-tid");
    }
    if policy.no_sale {
        input["ext"]["prebid"]["nosale"] = json!([flows[0].name()]);
    }
    input
}

fn runtime(flows: &[&Flow], policy: Policy, scoped: bool) -> Value {
    let mut config = fixtures::config(flows);
    config["host"]["ccpa"] = json!({"enforce":true});
    config["host"]["lmt"] = json!({"enforce":true});
    let mut activities = Map::new();
    for (activity, deny) in [
        ("transmitUfpd", policy.deny_ufpd),
        ("transmitPreciseGeo", policy.deny_geo),
        ("transmitTid", policy.deny_tid),
    ] {
        activities.insert(activity.into(), if scoped && deny {
            json!({"default":true,"rules":[{"allow":false,"condition":{"componentName":[flows[0].name()]}}]})
        } else { json!({"default":!deny}) });
    }
    config["accounts"] = json!({(flows[0].account()):{
        "ccpa":{"enabled":if policy.disable_ccpa {"disabled"} else {"enabled"}},
        "privacy":{"ipv4":{"anon_keep_bits":24_i32},"ipv6":{"anon_keep_bits":64_i32},"allowactivities":activities}}});
    config
}

fn wire(call: &ObservedCall) -> String {
    format!("{} {} {:?}", call.body, call.url, call.headers)
}

fn assert_absent(call: &ObservedCall, markers: &[&str]) {
    let rendered = wire(call);
    for marker in markers {
        assert!(
            !rendered.contains(marker),
            "{} leaked {marker} in actual outbound wire",
            call.owner
        );
    }
}

fn preserves_impression_tid(flow: &Flow) -> bool {
    if let Some(facts) = &flow.privacy_contract {
        return facts.preserves_impression_extension();
    }
    // The authored witness demonstrates either an unchanged bidder extension
    // or preservation of a sibling extension field while removing parameters.
    // This selects the reviewed extension-preserving protocols, not real names
    // or observations of the factory being tested.
    flow.case.outbound.iter().any(|outbound| {
        outbound.body["imp"].as_array().unwrap().iter().any(|imp| {
            let source = flow.case.request["imp"]
                .as_array()
                .unwrap()
                .iter()
                .find(|source| source["id"] == imp["id"])
                .unwrap();
            source
                .get("ext")
                .and_then(Value::as_object)
                .is_some_and(|fields| {
                    fields.iter().any(|(key, value)| {
                        key != "prebid"
                            && imp.get("ext").and_then(|ext| ext.get(key)) == Some(value)
                    })
                })
        })
    })
}

fn assert_allowed_impression_tids(body: &Value, preserves: bool) {
    if preserves {
        for imp in body["imp"].as_array().unwrap() {
            assert_eq!(
                imp.pointer("/ext/tid"),
                Some(&json!("private-imp-tid")),
                "owner-preserved allowed impression TID"
            );
        }
    }
}

fn assert_legacy_projection(body: &Value, gpp_supported: bool, policy: Policy) {
    if gpp_supported {
        for path in [
            "/user/consent",
            "/user/ext/consent",
            "/regs/gdpr",
            "/regs/ext/gdpr",
        ] {
            assert!(
                body.pointer(path).is_none(),
                "GPP-capable owner must not gain legacy backfill at {path}"
            );
        }
    } else {
        assert_eq!(
            body.pointer("/user/consent"),
            Some(&json!("opaque-tcf")),
            "legacy consent projection"
        );
        assert_eq!(
            body.pointer("/regs/gdpr"),
            Some(&json!(1_i32)),
            "legacy GDPR projection from active section 2"
        );
    }
    if gpp_supported || policy.malformed_usp {
        for path in ["/regs/us_privacy", "/regs/ext/us_privacy"] {
            assert!(
                body.pointer(path).is_none(),
                "no unsupported or malformed legacy USP projection at {path}"
            );
        }
    } else {
        let expected = if policy.sale { "1YYN" } else { "1YNN" };
        assert_eq!(
            body.pointer("/regs/us_privacy"),
            Some(&json!(expected)),
            "validated legacy USP projection"
        );
    }
}

#[test]
fn legacy_projection_controls_follow_capability_not_scrub_or_sale_exemption() {
    let body = json!({"user":{"consent":"opaque-tcf"},"regs":{"gdpr":1_i32,"us_privacy":"1YYN"}});
    assert_legacy_projection(
        &body,
        false,
        Policy {
            sale: true,
            no_sale: true,
            ..Policy::default()
        },
    );
    assert_legacy_projection(
        &json!({"user":{"consent":"opaque-tcf"},"regs":{"gdpr":1_i32}}),
        false,
        Policy {
            malformed_usp: true,
            ..Policy::default()
        },
    );
    assert_legacy_projection(
        &json!({"regs":{"gpp":"DBACNY~opaque-tcf~1YNN"}}),
        true,
        Policy::default(),
    );
}

#[test]
#[should_panic(expected = "legacy consent projection")]
fn raw_gpp_cannot_substitute_for_missing_legacy_consent_projection() {
    assert_legacy_projection(
        &json!({"regs":{"gpp":"DBACNY~opaque-tcf~1YNN","gdpr":1_i32,"us_privacy":"1YNN"}}),
        false,
        Policy::default(),
    );
}

#[test]
#[should_panic(expected = "owner-preserved allowed impression TID")]
fn allowed_source_tid_cannot_substitute_for_a_missing_impression_tid() {
    assert_allowed_impression_tids(
        &json!({"source":{"tid":"private-source-tid"},"imp":[{"id":"fake_imp","ext":{}}]}),
        true,
    );
}

fn assert_privacy(call: &ObservedCall, policy: Policy, flow: &Flow) {
    let body = &call.body;
    let rendered = wire(call);
    if policy.ids_denied() {
        assert_absent(call, IDS);
        assert_absent(call, &["private-device-context"]);
        for path in ID_PATHS {
            assert!(
                body.pointer(path).is_none(),
                "{} retained {path}",
                call.owner
            );
        }
    } else {
        // These survive every selected source protocol. EIDs may move to a
        // legacy extension and one protocol selects native over legacy EIDs.
        assert!(IDS.len() > 4);
        for marker in [IDS[0], IDS[1], IDS[2], IDS[4]] {
            assert!(
                rendered.contains(marker),
                "{} lost allowed positive-control {marker}",
                call.owner
            );
        }
    }
    if policy.coppa || policy.deny_ufpd {
        assert_absent(call, FPD);
        for path in [
            "/user/keywords",
            "/user/kwarray",
            "/user/data",
            "/user/ext/data",
        ] {
            assert!(
                body.pointer(path).is_none(),
                "{} retained user FPD {path}",
                call.owner
            );
        }
    } else {
        assert!(
            rendered.contains(FPD[0]),
            "{} lost allowed user FPD",
            call.owner
        );
    }
    if policy.deny_tid {
        assert_absent(call, &["private-source-tid", "private-imp-tid"]);
        assert!(body.pointer("/source/tid").is_none());
        for imp in body["imp"].as_array().unwrap() {
            assert!(imp.pointer("/ext/tid").is_none());
        }
    } else {
        assert_eq!(
            body.pointer("/source/tid"),
            Some(&json!("private-source-tid")),
            "{} must preserve allowed TID independently of ID policy",
            call.owner
        );
        assert_allowed_impression_tids(body, preserves_impression_tid(flow));
    }
    assert_eq!(
        body.pointer("/device/lmt"),
        Some(&json!(i32::from(policy.lmt)))
    );
    assert_eq!(body.pointer("/device/make"), Some(&json!("Example")));
    assert_eq!(body.pointer("/device/model"), Some(&json!("Handset")));
    assert_geo(call, policy);
    assert_signals(call, policy, flow);
}

fn assert_geo(call: &ObservedCall, policy: Policy) {
    let body = &call.body;
    if policy.coarse() {
        assert_absent(
            call,
            &[
                "8.8.4.123",
                "2001:4860:4860::8844",
                "private-location",
                "private-extra-location",
            ],
        );
        assert_eq!(body.pointer("/device/ip"), Some(&json!("8.8.4.0")));
        assert_eq!(
            body.pointer("/device/ipv6"),
            Some(&json!("2001:4860:4860::"))
        );
        for path in ["/user/geo", "/device/geo"] {
            if let Some(geo) = body.pointer(path) {
                if policy.coppa {
                    assert_eq!(
                        geo,
                        &json!({}),
                        "{}: COPPA dominates coarsening",
                        call.owner
                    );
                } else {
                    for key in ["zip", "accuracy", "ext", "extra_geo"] {
                        assert!(
                            geo.get(key).is_none(),
                            "{}: precise {path}/{key}",
                            call.owner
                        );
                    }
                    // Some source protocols omit coordinates even when allowed.
                    for (key, expected) in [("lat", 48.86_f64), ("lon", -1.12_f64)] {
                        if let Some(coordinate) = geo.get(key) {
                            assert_eq!(coordinate, &json!(expected));
                        }
                    }
                }
            }
        }
    } else {
        assert_eq!(body.pointer("/device/ip"), Some(&json!("8.8.4.123")));
        assert_eq!(
            body.pointer("/device/ipv6"),
            Some(&json!("2001:4860:4860::8844"))
        );
        assert_eq!(body.pointer("/device/geo/country"), Some(&json!("USA")));
        assert_eq!(
            body.pointer("/device/geo/zip"),
            Some(&json!("94107")),
            "{} needs a positive typed precise-geo control: {body}",
            call.owner
        );
    }
}

fn assert_signals(call: &ObservedCall, policy: Policy, flow: &Flow) {
    let body = &call.body;
    assert_absent(call, &["invalid-legacy-usp"]);
    assert_legacy_projection(body, flow.info.openrtb.gpp_supported, policy);
    // Protocols may project these native signals into regs.ext; neither the
    // signal value nor the original ordered section list may change.
    let regs = body.get("regs").unwrap();
    for (key, expected) in [
        ("coppa", json!(i32::from(policy.coppa))),
        (
            "gpp",
            json!(format!(
                "DBACNY~opaque-tcf~{}",
                if policy.malformed_usp {
                    "invalid-selected-usp"
                } else if policy.sale {
                    "1YYN"
                } else {
                    "1YNN"
                }
            )),
        ),
        ("gpp_sid", json!([6_i32, 2_i32, 6_i32])),
    ] {
        assert_eq!(
            regs.get(key).or_else(|| regs.get("ext")?.get(key)),
            Some(&expected),
            "{} privacy signal {key}",
            call.owner
        );
    }
}

fn policies() -> [Policy; 11] {
    let allow = Policy::default();
    [
        allow,
        Policy {
            coppa: true,
            sale: true,
            no_sale: true,
            ..allow
        },
        Policy { lmt: true, ..allow },
        Policy {
            sale: true,
            ..allow
        },
        Policy {
            sale: true,
            disable_ccpa: true,
            ..allow
        },
        Policy {
            sale: true,
            no_sale: true,
            ..allow
        },
        Policy {
            sale: true,
            malformed_usp: true,
            ..allow
        },
        Policy {
            deny_ufpd: true,
            ..allow
        },
        Policy {
            deny_geo: true,
            ..allow
        },
        Policy {
            deny_tid: true,
            ..allow
        },
        Policy {
            coppa: true,
            lmt: true,
            sale: true,
            deny_ufpd: true,
            deny_geo: true,
            deny_tid: true,
            ..allow
        },
    ]
}

/// Wire from the coordinator's existing boot-once case:
/// `super::privacy::all_priority_outbound_privacy(&app, &cohort);`.
pub(super) fn all_priority_outbound_privacy(app: &App, cohort: &[Flow]) {
    let policies = policies();
    let mut site_names = BTreeSet::new();
    let mut app_names = BTreeSet::new();
    let mut cases = 0_usize;
    for flow in cohort {
        for inventory in ["site", "app"] {
            let supported = if inventory == "site" {
                &flow.info.capabilities.site
            } else {
                &flow.info.capabilities.app
            };
            let applicable = flow.case.request["imp"]
                .as_array()
                .unwrap()
                .iter()
                .all(|imp| {
                    [
                        ("banner", MediaType::Banner),
                        ("video", MediaType::Video),
                        ("audio", MediaType::Audio),
                        ("native", MediaType::Native),
                    ]
                    .into_iter()
                    .filter(|(key, _)| imp.get(*key).is_some())
                    .all(|(_, media)| supported.contains(&media))
                });
            if !applicable {
                assert_eq!(
                    inventory, "app",
                    "every frozen priority site flow is mandatory"
                );
                continue;
            }
            for policy in policies {
                let requested = [flow];
                let secrets = Secrets::new(&requested, false);
                let answer = harness::send_privacy(
                    app,
                    &requested,
                    &request(&requested, inventory, policy),
                    &runtime(&requested, policy, false),
                    &secrets,
                );
                harness::assert_success(&answer, &requested);
                for call in &answer.calls {
                    assert_privacy(call, policy, flow);
                }
                cases = cases.checked_add(1).unwrap();
            }
            if inventory == "site" {
                site_names.insert(flow.name().to_owned());
            } else {
                app_names.insert(flow.name().to_owned());
            }
        }
        assert_scoped_sibling_policies(app, cohort, flow);
        cases = cases.checked_add(1).unwrap();
    }
    assert_eq!(
        site_names,
        cohort.iter().map(|flow| flow.name().to_owned()).collect()
    );
    assert_eq!(
        site_names.len(),
        14,
        "no default-active-only cohort substitution"
    );
    assert!(
        !app_names.is_empty(),
        "exercise applicable app channel policies"
    );
    let expected_cases = 11_usize
        .checked_mul(site_names.len().checked_add(app_names.len()).unwrap())
        .unwrap()
        .checked_add(14)
        .unwrap();
    assert_eq!(cases, expected_cases);
    writeln!(
        stderr().lock(),
        "native privacy: {} site identities, {} applicable app identities, {cases} dispatch cases",
        site_names.len(),
        app_names.len()
    )
    .unwrap();
}

fn assert_scoped_sibling_policies(app: &App, cohort: &[Flow], flow: &Flow) {
    let allow = Policy::default();
    let sibling = cohort
        .iter()
        .find(|sibling| {
            sibling.name() != flow.name()
                && sibling.credential().is_none()
                && sibling.case.outbound.len() == 1
        })
        .unwrap();
    let requested = [flow, sibling];
    let denied = Policy {
        deny_ufpd: true,
        deny_geo: true,
        deny_tid: true,
        ..allow
    };
    let secrets = Secrets::new(&requested, false);
    let answer = harness::send_privacy(
        app,
        &requested,
        &request(&requested, "site", denied),
        &runtime(&requested, denied, true),
        &secrets,
    );
    harness::assert_success(&answer, &requested);
    for call in &answer.calls {
        assert_privacy(
            call,
            if call.owner == flow.name() {
                denied
            } else {
                allow
            },
            requested
                .iter()
                .copied()
                .find(|requested_flow| requested_flow.name() == call.owner)
                .unwrap(),
        );
    }
}
