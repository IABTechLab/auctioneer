// Mixed-media and post-FPD evidence for default-active site banner/video factories.
// Generic targeting controls in the parent module use the canonical fake.

use super::*;
use auctioneer_bidder::test_support::serving_catalog::default_active_site_media;

#[test]
fn mixed_media_response_preserves_winner_targeting_and_bid_local_errors() {
    block_on(async {
        for bidder in default_active_site_media(&[MediaType::Banner, MediaType::Video]) {
            let name = bidder.name();
            let mut request = base_request();
            request.imp[0].video = Some(Video::default());
            request.imp[0].ext.as_mut().unwrap().bidder = JsonField::Value(serde_json::json!({
                name: bidder.params,
            }));
            let mut targeting = ExtRequestTargeting {
                includeformat: Some(true),
                pricegranularity: Some(serde_json::json!("low")),
                ..ExtRequestTargeting::default()
            };
            targeting.other.insert(
                "mediatypepricegranularity".into(),
                serde_json::json!({"video": "high"}),
            );
            request.ext = Some(BidRequestExt {
                prebid: Some(ExtRequestPrebid {
                    targeting: Some(targeting),
                    ..Default::default()
                }),
                ..Default::default()
            });
            let body = serde_json::to_vec(&serde_json::json!({
                "id": "r", "cur": "USD", "seatbid": [
                    {"seat": "fake_first", "bid": [
                        {"id": "bad-first", "impid": "imp-1", "price": 99.0_f64, "mtype": 0_i32},
                        {"id": "video-winner", "impid": "imp-1", "price": 9.5_f64, "mtype": 2_i32},
                    ]},
                    {"seat": "fake_second", "bid": [
                        {"id": "banner-sibling", "impid": "imp-1", "price": 0.75_f64, "mtype": 1_i32},
                        {"id": "bad-second", "impid": "imp-1", "price": 100.0_f64, "mtype": 3_i32},
                    ]},
                ],
            })).unwrap();
            let response = drive_with_adapters(
                request,
                &AccountConfig::default(),
                body,
                vec![(name, (bidder.factory)())],
            )
            .await;
            let wire = serde_json::to_value(&response).unwrap();
            assert_eq!(
                response.seatbid.len(),
                1,
                "{name}: preserve classified bids"
            );
            assert_eq!(
                response.seatbid[0].bid.len(),
                2,
                "{name}: one rejection per invalid bid"
            );
            let winner = response.seatbid[0]
                .bid
                .iter()
                .find(|bid| bid.id == "video-winner")
                .unwrap();
            let winner_targeting = &winner
                .ext
                .as_ref()
                .unwrap()
                .prebid
                .as_ref()
                .unwrap()
                .targeting;
            assert_eq!(
                winner_targeting.get("hb_format").map(String::as_str),
                Some("video"),
                "{name}"
            );
            assert_eq!(
                winner_targeting.get("hb_pb").map(String::as_str),
                Some("9.50"),
                "{name}"
            );
            assert_eq!(
                winner_targeting
                    .get(&format!("hb_format_{name}"))
                    .map(String::as_str),
                Some("video")
            );
            let errors = wire["ext"]["errors"][name].as_array().unwrap();
            assert_eq!(
                errors.len(),
                2,
                "{name}: both bid-local errors reach the wire"
            );
            assert!(errors.iter().all(|error| error["code"] == 4_i32));
        }
    });
}

#[test]
fn response_classification_uses_post_fpd_formats() {
    block_on(async {
        for bidder in default_active_site_media(&[MediaType::Banner, MediaType::Video]) {
            let name = bidder.name();
            for (original_banner, patch, fields, expect_error) in [
                (
                    false,
                    serde_json::json!({"banner": {"w": 300_i32, "h": 250_i32}}),
                    serde_json::json!({"mtype": 1_i32}),
                    false,
                ),
                (
                    true,
                    serde_json::json!({"video": null}),
                    // Missing-mtype inference is pinned by the canonical-fake
                    // counterpart; default-active protocols may require the field.
                    serde_json::json!({"mtype": 1_i32}),
                    false,
                ),
                (
                    true,
                    serde_json::json!({"video": null}),
                    serde_json::json!({"mtype": 2_i32}),
                    true,
                ),
            ] {
                let mut request = base_request();
                request.imp[0].video = Some(Video::default());
                if !original_banner {
                    request.imp[0].banner = None;
                }
                request.imp[0].ext = Some(
                    serde_json::from_value(serde_json::json!({
                        "bidder": {name: bidder.params},
                        "prebid": {"imp": {name: patch}},
                    }))
                    .unwrap(),
                );
                let mut bid =
                    serde_json::json!({"id": "result", "impid": "imp-1", "price": 9.5_f64});
                bid.as_object_mut()
                    .unwrap()
                    .extend(fields.as_object().unwrap().clone());
                let body = serde_json::to_vec(&serde_json::json!({"id": "r", "cur": "USD", "seatbid": [{"seat": name, "bid": [bid]}]})).unwrap();
                let response = drive_with_adapters(
                    request,
                    &AccountConfig::default(),
                    body,
                    vec![(name, (bidder.factory)())],
                )
                .await;
                let wire = serde_json::to_value(&response).unwrap();
                if expect_error {
                    assert!(
                        response.seatbid.is_empty(),
                        "{name}: removed media cannot be admitted"
                    );
                    assert_eq!(wire["ext"]["errors"][name][0]["code"], 4_i32);
                } else {
                    assert_eq!(
                        response.seatbid.len(),
                        1,
                        "{name}: post-FPD banner must be classified"
                    );
                    assert_eq!(response.seatbid[0].bid[0].id, "result");
                    assert!(wire["ext"]["errors"][name].is_null());
                }
            }
        }
    });
}
