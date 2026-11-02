use crate::auction_support::{Harness, request, second_imp};
use auctioneer_bidder::BidderName;
use auctioneer_exchange::AuctionError;
use auctioneer_runtime::{Rng as _, SeededRng};
use serde_json::{Value, json};
use std::time::Duration;

#[test]
fn sampled_out_fpd_is_not_validated() {
    for patch in [json!({"banner": null}), json!({"banner": 7_i32})] {
        let mut harness = Harness::default();
        harness.host.resource_limits.max_bidders_per_auction = 1;
        harness
            .sample_rates
            .insert(BidderName::new("fake_bb"), 0.0_f64);
        let mut req = request(&["fake_bb", "fake_ccc"]);
        req["imp"][0]["ext"]["prebid"] = json!({"imp": {"fake_bb": patch}});
        let observed = harness.run(req);
        assert!(
            observed.result.is_ok(),
            "sampled-out FPD: {:?}",
            observed.result
        );
        observed.assert_built(&["fake_ccc"]);
        assert!(observed.ext()["warnings"].get("fake_bb").is_none());
    }
}

#[test]
fn cap_dropped_fpd_is_not_validated() {
    for patch in [json!({"banner": null}), json!({"banner": 7_i32})] {
        let mut harness = Harness::default();
        harness.host.resource_limits.max_bidders_per_auction = 1;
        let mut req = request(&["fake_bb", "fake_ccc"]);
        req["imp"][0]["ext"]["prebid"] = json!({"imp": {"fake_ccc": patch}});
        let observed = harness.run(req);
        assert!(
            observed.result.is_ok(),
            "cap-dropped FPD: {:?}",
            observed.result
        );
        observed.assert_built(&["fake_bb"]);
        assert_eq!(
            observed.ext()["warnings"]["fake_ccc"][0]["code"],
            10_999_i32
        );
    }
}

#[test]
fn survivor_fpd_still_fails_typed_roundtrip_and_validation() {
    for patch in [json!({"banner": null}), json!({"banner": 7_i32})] {
        let mut harness = Harness::default();
        harness.host.resource_limits.max_bidders_per_auction = 1;
        let mut req = request(&["fake_bb", "fake_ccc"]);
        req["imp"][0]["ext"]["prebid"] = json!({"imp": {"fake_bb": patch}});
        let observed = harness.run(req);
        assert!(matches!(
            observed.result,
            Err(AuctionError::InvalidImpFirstPartyData(_))
        ));
        observed.assert_built(&[]);
        assert!(observed.outbound.is_empty());
    }
}

fn mixed_fixture() -> (Harness, Value) {
    let mut harness = Harness::default();
    harness.host.resource_limits.max_bidders_per_auction = 2;
    harness.disabled.insert(BidderName::new("fake_aaa"));
    harness.activities(json!({
        "fetchBids": {"rules": [{"condition": {"componentName": ["fake_a_denied"]}, "allow": false}]},
        "transmitUfpd": {"rules": [{"condition": {"componentName": ["fake_c~"]}, "allow": false}]},
        "transmitTid": {"rules": [{"condition": {"componentName": ["fake_c~"]}, "allow": false}]}
    }));
    harness
        .sample_rates
        .insert(BidderName::new("fake_a_denied"), 0.0_f64);
    harness
        .sample_rates
        .insert(BidderName::new("fake_b_sampled"), 0.0_f64);
    let mut req = json!({
        "id": "mixed", "tmax": 1000_i32, "site": {"id": "s"},
        "source": {"tid": "source-tid"}, "user": {"id": "u", "buyeruid": "buyer"},
        "imp": [{"id": "imp-1", "banner": {"w": 300_i32, "h": 250_i32}, "ext": {
            "tid": "imp-tid", "hint": "keep", "fake_aaa": {"disabled": true},
            "bidder": {"fake_a_denied": {}, "fake_b_sampled": {}, "fake_c~": {"own": 1_i32}, "fake_d/": {"own": 2_i32}, "fake_e_cap": {}},
            "prebid": {"bidder": {"fake_c~": {"own": 99_i32}}, "imp": {
                "fake_c~": {"tagid": "scrub-fpd", "ext": {"tid": "fpd-tid"}},
                "fake_d/": {"tagid": "allow-fpd"}
            }}
        }}],
        "ext": {"prebid": {"returnallbidstatus": true,
            "aliases": {"fake_a_denied": "fake_bb", "fake_b_sampled": "fake_bb", "fake_c~": "fake_bb", "fake_d/": "fake_bb", "fake_e_cap": "fake_bb"},
            "bidderparams": {"fake_c~": {"own": 99_i32, "supplement": "tilde"}, "fake_d/": {"supplement": "slash"}}
        }}
    });
    second_imp(&mut req);
    req["imp"][1]["ext"]["bidder"]
        .as_object_mut()
        .unwrap()
        .remove("fake_d/");
    (harness, req)
}

#[test]
fn mixed_pipeline_matches_pre_grouping_wire_snapshot() {
    let (harness, req) = mixed_fixture();
    let observed = harness.run(req);
    observed.assert_built(&["fake_c~", "fake_d/"]);
    observed.assert_blocked("fake_a_denied", &["imp-1", "imp-2"]);
    let wire = json!({
        "scrubbed": observed.body("fake_c~"),
        "allowed": observed.body("fake_d/"),
        "response": observed.response()
    });
    let baseline: Value = serde_json::from_str(include_str!("grouped_snapshot.json")).unwrap();
    assert_eq!(
        wire, baseline,
        "full bidder bodies and response before grouping refactor"
    );
}

#[test]
fn expired_tmax_keeps_privacy_disabled_and_cap_bookkeeping_without_dropped_fpd() {
    let (mut harness, mut req) = mixed_fixture();
    harness.elapsed = Duration::from_secs(2);
    req["imp"][0]["ext"]["prebid"]["imp"]["fake_a_denied"] = json!({"banner": null});
    req["imp"][0]["ext"]["prebid"]["imp"]["fake_b_sampled"] = json!({"banner": null});
    req["imp"][0]["ext"]["prebid"]["imp"]["fake_e_cap"] = json!({"banner": null});
    let observed = harness.run(req);
    assert!(
        observed.result.is_ok(),
        "expired FPD: {:?}",
        observed.result
    );
    observed.assert_built(&[]);
    observed.assert_blocked("fake_a_denied", &["imp-1", "imp-2"]);
    assert_eq!(observed.ext()["warnings"]["fake_aaa"][0]["code"], 6_i32);
    assert_eq!(
        observed.ext()["warnings"]["fake_e_cap"][0]["code"],
        10_999_i32
    );
    assert!(observed.ext()["warnings"].get("fake_b_sampled").is_none());
    assert_eq!(observed.ext()["errors"]["prebid"][0]["code"], 12_i32);
}

#[test]
fn sampling_keeps_name_order_base_fallback_and_draws_after_cap_is_full() {
    let names = [
        "fake_z_zero",
        "fake_e_sampled",
        "fake_d_cap",
        "fake_c_sampled",
        "fake_b_kept",
        "fake_a_denied",
    ];
    let mut harness = Harness::default();
    harness.host.resource_limits.max_bidders_per_auction = 1;
    harness.activities(json!({"fetchBids": {"rules": [
        {"condition": {"componentName": ["fake_a_denied"]}, "allow": false}
    ]}}));
    for (name, rate) in [
        ("fake_bb", 0.5_f64),
        ("fake_d_cap", 1.0_f64),
        ("fake_z_zero", 0.0_f64),
    ] {
        harness.sample_rates.insert(BidderName::new(name), rate);
    }
    let mut req = request(&names);
    req["ext"]["prebid"]["aliases"] = names
        .iter()
        .map(|name| ((*name).to_owned(), json!("fake_bb")))
        .collect::<serde_json::Map<_, _>>()
        .into();
    second_imp(&mut req);
    let observed = harness.run(req);
    observed.assert_built(&["fake_b_kept"]);
    observed.assert_blocked("fake_a_denied", &["imp-1", "imp-2"]);
    assert_eq!(
        observed.ext()["warnings"]["fake_d_cap"][0]["code"],
        10_999_i32
    );
    for name in ["fake_c_sampled", "fake_e_sampled", "fake_z_zero"] {
        assert!(observed.ext()["warnings"].get(name).is_none());
    }
    let expected_rng = SeededRng::new(42);
    for _ in 0_u8..3 {
        expected_rng.next_u64();
    }
    assert_eq!(
        observed.rng_next,
        expected_rng.next_u64(),
        "one draw per fractional-rate allowed identity, including after cap fills"
    );
}

#[test]
fn slash_and_pointer_escape_aliases_keep_params_fpd_and_competitor_scoping() {
    for name in ["fake_my/alias", "fake_my~0alias", "fake_my~1alias"] {
        for slot in ["hoisted", "typed", "legacy"] {
            let harness = Harness::default();
            let mut req = request(&["fake_ccc"]);
            req["ext"]["prebid"]["aliases"] = json!({(name): "fake_bb"});
            req["ext"]["prebid"]["bidderparams"] = json!({
                (name): {"own": 99_i32, "nested": {"own": 99_i32, "supplement": 2_i32}},
                "fake_bb": {"must_not_create_participation": true}
            });
            req["imp"][0]["ext"]["prebid"] = json!({"imp": {
                (name): {"tagid": "found-fpd"}, "fake_ccc": {"tagid": "sibling"}
            }});
            let params = json!({"own": 1_i32, "nested": {"own": 1_i32}});
            match slot {
                "hoisted" => req["imp"][0]["ext"]["bidder"][name] = params,
                "typed" => req["imp"][0]["ext"]["prebid"]["bidder"] = json!({(name): params}),
                _ => req["imp"][0]["ext"][name] = params,
            }
            let observed = harness.run(req);
            observed.assert_built(&[name, "fake_ccc"]);
            let body = observed.body(name);
            assert_eq!(body["imp"][0]["tagid"], "found-fpd");
            assert_eq!(
                body["imp"][0]["ext"]["bidder"],
                json!({"own": 1_i32, "nested": {"own": 1_i32, "supplement": 2_i32}})
            );
            let sibling = observed.body("fake_ccc");
            for outbound in [&body, &sibling] {
                let ext = &outbound["imp"][0]["ext"];
                assert!(ext.get(name).is_none());
                assert!(ext["prebid"].get("bidder").is_none());
                assert!(ext["prebid"].get("imp").is_none());
            }
            assert_eq!(sibling["imp"][0]["tagid"], "sibling");
        }
    }
}
