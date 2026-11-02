use crate::bidders::url::{json_utf8_headers, write_endpoint};
use crate::{BidderError, BidderRequestInfo, ConstructionDiagnostic, OutboundRequestSink};
use auctioneer_openrtb::types::{
    bid_request::{BidRequest, Imp},
    field::JsonField,
};
use serde::{Serialize, Serializer, ser::SerializeMap as _};
use serde_json::Value;

pub(super) fn make_requests(
    request: &BidRequest,
    info: &BidderRequestInfo<'_>,
    sink: &mut OutboundRequestSink<'_>,
) -> Result<(), Vec<BidderError>> {
    let mut has_survivor = false;
    for imp in &request.imp {
        match prepare(imp) {
            Ok(_) => has_survivor = true,
            Err(reason) => sink.diagnostic(reason),
        }
    }
    if !has_survivor {
        return Err(vec![BidderError::unknown(
            "No valid impressions for triplelift",
        )]);
    }
    let _recorded = sink.write_json(
        info,
        |meta| {
            meta.set_method("POST")?;
            write_endpoint(meta, info)?;
            json_utf8_headers(meta)?;
            for survivor in request.imp.iter().filter_map(|imp| prepare(imp).ok()) {
                meta.imp_id(&survivor.imp.id)?;
            }
            Ok(())
        },
        &Request(request),
    );
    Ok(())
}

struct Impression<'a> {
    imp: &'a Imp,
    inventory: &'a str,
    floor: Option<f64>,
}

pub(super) fn unique_field<'a>(
    fields: &'a serde_json::Map<String, Value>,
    name: &str,
) -> Result<Option<&'a Value>, ()> {
    // Parsed maps retain case variants, but not the source's wire precedence.
    let mut matches = fields.iter().filter(|(key, _value)| {
        if name == "inventoryCode" {
            // Source's camel-case parameter takes json-iterator's general
            // lowercase path; the already-lowercase media keys do not.
            key.chars()
                .map(|character| match character {
                    '\u{0130}' => 'i',
                    _ => character.to_ascii_lowercase(),
                })
                .eq("inventorycode".chars())
        } else {
            key.eq_ignore_ascii_case(name)
        }
    });
    let found = matches.next().map(|(_key, value)| value);
    if matches.next().is_some() {
        return Err(());
    }
    Ok(found)
}

fn prepare(imp: &Imp) -> Result<Impression<'_>, ConstructionDiagnostic> {
    let invalid = ConstructionDiagnostic::InvalidImpressionParameters;
    let params = match &imp.ext.as_ref().ok_or(invalid)?.bidder {
        JsonField::Missing => return Err(invalid),
        JsonField::Null => &Value::Null,
        JsonField::Value(value) => value,
    };
    let (inventory, floor) = match params {
        Value::Null => ("", None),
        Value::Object(fields) => {
            let inventory = match unique_field(fields, "inventoryCode").map_err(|()| invalid)? {
                None | Some(Value::Null) => "",
                Some(value) => value.as_str().ok_or(invalid)?,
            };
            let floor = match unique_field(fields, "floor").map_err(|()| invalid)? {
                None | Some(Value::Null) => None,
                Some(value) => Some(
                    value
                        .as_f64()
                        .filter(|number| number.is_finite())
                        .ok_or(invalid)?,
                ),
            };
            (inventory, floor)
        }
        Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
            return Err(invalid);
        }
    };
    if imp.banner.is_none() && imp.video.is_none() {
        return Err(ConstructionDiagnostic::UnsupportedMediaType);
    }
    Ok(Impression {
        imp,
        inventory,
        floor: floor.or(imp.bidfloor),
    })
}

struct Request<'a>(&'a BidRequest);

impl Serialize for Request<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // New typed fields must force a review of this borrowed wire view.
        let BidRequest {
            id,
            imp,
            site,
            app,
            dooh,
            device,
            user,
            regs,
            source,
            at,
            tmax,
            cur,
            test,
            ext,
            other,
        } = self.0;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", id)?;
        map.serialize_entry("imp", &Survivors(imp))?;
        if let Some(value) = site {
            map.serialize_entry("site", value)?;
        }
        if let Some(value) = app {
            map.serialize_entry("app", value)?;
        }
        if let Some(value) = dooh {
            map.serialize_entry("dooh", value)?;
        }
        if let Some(value) = device {
            map.serialize_entry("device", value)?;
        }
        if let Some(value) = user {
            map.serialize_entry("user", value)?;
        }
        if let Some(value) = regs {
            map.serialize_entry("regs", value)?;
        }
        if let Some(value) = source {
            map.serialize_entry("source", value)?;
        }
        if let Some(value) = at {
            map.serialize_entry("at", value)?;
        }
        if let Some(value) = tmax {
            map.serialize_entry("tmax", value)?;
        }
        if !cur.is_empty() {
            map.serialize_entry("cur", cur)?;
        }
        if let Some(value) = test {
            map.serialize_entry("test", value)?;
        }
        if let Some(value) = ext {
            map.serialize_entry("ext", value)?;
        }
        for (key, value) in other {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

struct Survivors<'a>(&'a [Imp]);

impl Serialize for Survivors<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.0.iter().filter_map(|imp| prepare(imp).ok()))
    }
}

impl Serialize for Impression<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Imp {
            id,
            banner,
            video,
            native,
            audio,
            bidfloor: _original_floor,
            bidfloorcur,
            secure,
            ext,
            other,
        } = self.imp;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", id)?;
        if let Some(value) = banner {
            map.serialize_entry("banner", value)?;
        }
        if let Some(value) = video {
            map.serialize_entry("video", value)?;
        }
        if let Some(value) = native {
            map.serialize_entry("native", value)?;
        }
        if let Some(value) = audio {
            map.serialize_entry("audio", value)?;
        }
        if let Some(value) = self.floor.filter(|floor| *floor != 0.0_f64) {
            map.serialize_entry("bidfloor", &value)?;
        }
        if let Some(value) = bidfloorcur {
            map.serialize_entry("bidfloorcur", value)?;
        }
        if let Some(value) = secure {
            map.serialize_entry("secure", value)?;
        }
        if let Some(value) = ext {
            map.serialize_entry("ext", value)?;
        }
        if !self.inventory.is_empty() {
            map.serialize_entry("tagid", self.inventory)?;
        }
        for (key, value) in other {
            if key != "tagid" {
                map.serialize_entry(key, value)?;
            }
        }
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use super::super::TripleliftBidder;
    use crate::bidders::harness;
    use crate::{
        BidderError, BidderName, BidderRequestInfo, ConstructionDiagnostic, EndpointRejectReason,
        EndpointValidator, EntryPoint, OutboundBuildError, SinkLimits,
    };
    use auctioneer_openrtb::types::{bid_request::BidRequest, field::JsonField};
    use serde_json::{Value, json};
    use url::Url;

    struct EndpointPolicy(bool);
    impl EndpointValidator for EndpointPolicy {
        fn validate(
            &self,
            _url: &Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            if self.0 {
                Ok(())
            } else {
                Err(EndpointRejectReason::HostNotAllowed)
            }
        }
    }

    fn info() -> BidderRequestInfo<'static> {
        BidderRequestInfo::new(
            BidderName::new("triplelift-alias"),
            BidderName::new("triplelift"),
            "account",
            None,
            "https://bidder.example.test/auction?sra=1&supplier_id=20",
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        )
    }
    fn request(impressions: &Value) -> BidRequest {
        serde_json::from_value(json!({"id":"r", "imp":impressions, "site":{"id":"publisher"}, "ext":{"keep":{"nested":true}}, "custom":[true,null]})).unwrap()
    }
    fn minimal() -> BidRequest {
        request(&json!([{"id":"i", "banner":{}, "ext":{"bidder":{"inventoryCode":"slot"}}}]))
    }

    #[test]
    fn transformed_survivors_preserve_input_extensions_and_order() {
        for count in [1_usize, 8, 100] {
            let mut impressions = vec![json!({"id":"bad", "banner":{}, "ext":{"bidder":false}})];
            impressions.extend((0..count).map(|index| json!({
                "id":format!("i-{index}"), "banner":{"w":300_i32,"h":250_i32}, "video":{}, "native":{}, "audio":{},
                "tagid":"old", "bidfloor":0.75_f64, "bidfloorcur":"EUR", "secure":1_i32, "custom":{"keep":true},
                "ext":{"bidder":{"inventoryCode":"new","floor":2.5_f64}, "unrelated":{"keep":true}},
            })));
            let request = request(&json!(impressions));
            let before = serde_json::to_value(&request).unwrap();
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &TripleliftBidder,
                &request,
                &info(),
                SinkLimits::new(256 * 1024, 100),
                1,
                &EndpointPolicy(true),
            );
            semantic.unwrap();
            assert_eq!(remaining, 0);
            assert!(outcome.errors.is_empty());
            assert_eq!(
                outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
                [ConstructionDiagnostic::InvalidImpressionParameters]
            );
            assert_eq!(outcome.accepted.len(), 1);
            let call = &outcome.accepted[0];
            assert_eq!(call.method(), "POST");
            assert_eq!(call.url(), info().endpoint_template);
            assert_eq!(
                call.headers().collect::<Vec<_>>(),
                [
                    ("Content-Type", "application/json;charset=utf-8"),
                    ("Accept", "application/json")
                ]
            );
            assert_eq!(
                call.covered_imp_ids().collect::<Vec<_>>(),
                request.imp[1..]
                    .iter()
                    .map(|imp| imp.id.as_str())
                    .collect::<Vec<_>>()
            );
            let mut expected = before.clone();
            let expected_imps = expected["imp"].as_array_mut().unwrap();
            expected_imps.remove(0);
            for imp in expected_imps {
                imp["tagid"] = json!("new");
                imp["bidfloor"] = json!(2.5_f64);
            }
            assert_eq!(
                serde_json::from_slice::<Value>(call.body()).unwrap(),
                expected
            );
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
    }

    #[test]
    fn every_populated_root_field_survives_the_impression_only_transform() {
        let request: BidRequest = serde_json::from_value(json!({
            "id":"r", "imp":[{"id":"i", "banner":{}, "ext":{"bidder":{"inventoryCode":"slot"}}}],
            "site":{"id":"site"}, "app":{"id":"app"}, "dooh":{"id":"dooh"},
            "device":{"make":"device"}, "user":{"id":"already-scoped"},
            "regs":{"coppa":1_i32}, "source":{"tid":"already-scoped"},
            "at":2_i32, "tmax":150_i64, "cur":["EUR"], "test":1_i32,
            "ext":{"keep":true}, "opaque":[false,null],
        }))
        .unwrap();
        let before = serde_json::to_value(&request).unwrap();
        assert_eq!(before.as_object().unwrap().len(), 15);
        let calls = harness::drive_ok(&TripleliftBidder, &request, &info());
        assert_eq!(calls.len(), 1);
        let mut expected = before.clone();
        expected["imp"][0]["tagid"] = json!("slot");
        assert_eq!(
            serde_json::from_slice::<Value>(calls[0].body()).unwrap(),
            expected
        );
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn absent_and_null_parameter_fields_preserve_floor_but_clear_the_tag() {
        for parameters in [
            json!({}),
            json!({"inventoryCode":null}),
            json!({"floor":null}),
        ] {
            let request = request(
                &json!([{"id":"i", "banner":{}, "tagid":"old", "bidfloor":4.25_f64, "ext":{"bidder":parameters}}]),
            );
            let calls = harness::drive_ok(&TripleliftBidder, &request, &info());
            let body: Value = serde_json::from_slice(calls[0].body()).unwrap();
            assert!(!body["imp"][0].as_object().unwrap().contains_key("tagid"));
            assert_eq!(body["imp"][0]["bidfloor"], json!(4.25_f64));
            assert_eq!(
                body["imp"][0]["ext"],
                serde_json::to_value(&request.imp[0].ext).unwrap()
            );
            assert_eq!(request.imp[0].other["tagid"], json!("old"));
        }
    }

    #[test]
    fn single_case_insensitive_parameter_keys_preserve_values_and_extensions() {
        for inventory in [
            "inventoryCode",
            "InventoryCode",
            "INVENTORYCODE",
            "\u{0130}nventoryCode",
        ] {
            for floor in ["floor", "Floor", "FLOOR"] {
                let parameters = json!({inventory:"CaseSensitiveSlot", floor:2.75_f64});
                let request = request(&json!([{"id":"i", "banner":{}, "tagid":"old",
                    "bidfloor":5.0_f64, "ext":{"bidder":parameters}}]));
                let before = serde_json::to_value(&request).unwrap();
                let calls = harness::drive_ok(&TripleliftBidder, &request, &info());
                let body: Value = serde_json::from_slice(calls[0].body()).unwrap();
                assert_eq!(body["imp"][0]["tagid"], json!("CaseSensitiveSlot"));
                assert_eq!(body["imp"][0]["bidfloor"], json!(2.75_f64));
                assert_eq!(body["imp"][0]["ext"]["bidder"], parameters);
                assert_eq!(serde_json::to_value(&request).unwrap(), before);
            }
        }
    }

    #[test]
    fn ambiguous_parameter_keys_reject_only_the_affected_impression() {
        for parameters in [
            json!({"inventoryCode":"a", "INVENTORYCODE":"b"}),
            json!({"inventoryCode":"a", "InventoryCode":"a"}),
            json!({"inventoryCode":null, "InventoryCode":"a"}),
            json!({"InventoryCode":"a", "INVENTORYCODE":"b"}),
            json!({"inventoryCode":"a", "\u{0130}nventoryCode":"b"}),
            json!({"InventoryCode":"a", "\u{0130}nventoryCode":null}),
            json!({"inventoryCode":"a", "floor":1.0_f64, "FLOOR":2.0_f64}),
            json!({"inventoryCode":"a", "floor":1.0_f64, "Floor":1.0_f64}),
            json!({"inventoryCode":"a", "floor":null, "Floor":1.0_f64}),
            json!({"inventoryCode":"a", "Floor":1.0_f64, "FLOOR":2.0_f64}),
        ] {
            let request = request(&json!([
                {"id":"ambiguous", "banner":{}, "ext":{"bidder":parameters}},
                {"id":"valid", "banner":{}, "ext":{"bidder":{"inventoryCode":"slot"}}},
            ]));
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &TripleliftBidder,
                &request,
                &info(),
                SinkLimits::new(4096, 2),
                1,
                &EndpointPolicy(true),
            );
            semantic.unwrap();
            assert_eq!(remaining, 0);
            assert!(outcome.errors.is_empty());
            assert_eq!(outcome.accepted.len(), 1);
            assert_eq!(
                outcome.accepted[0].covered_imp_ids().collect::<Vec<_>>(),
                ["valid"]
            );
            let body: Value = serde_json::from_slice(outcome.accepted[0].body()).unwrap();
            assert_eq!(body["imp"].as_array().unwrap().len(), 1);
            assert_eq!(body["imp"][0]["id"], json!("valid"));
            assert_eq!(
                outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
                [ConstructionDiagnostic::InvalidImpressionParameters]
            );
        }
    }

    #[test]
    fn dotted_inventory_key_values_are_validated_instead_of_ignored() {
        for value in [json!(false), json!(1_i32), json!({}), json!([])] {
            let request = request(&json!([{"id":"i", "banner":{},
                "ext":{"bidder":{"\u{0130}nventoryCode":value}}}]));
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &TripleliftBidder,
                &request,
                &info(),
                SinkLimits::new(4096, 1),
                1,
                &EndpointPolicy(true),
            );
            assert_eq!(
                semantic.unwrap_err(),
                [BidderError::unknown("No valid impressions for triplelift")]
            );
            assert!(outcome.accepted.is_empty());
            assert!(outcome.errors.is_empty());
            assert_eq!(remaining, 1);
            assert_eq!(
                outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
                [ConstructionDiagnostic::InvalidImpressionParameters]
            );
        }
    }

    #[test]
    fn floor_override_includes_negative_values_and_elides_both_zero_signs() {
        for floor in [0.0_f64, -0.0_f64, -2.0_f64, 3.125_f64] {
            let request = request(&json!([{"id":"i", "banner":{}, "bidfloor":5.0_f64,
                "ext":{"bidder":{"inventoryCode":"slot", "floor":floor}}}]));
            let calls = harness::drive_ok(&TripleliftBidder, &request, &info());
            let body: Value = serde_json::from_slice(calls[0].body()).unwrap();
            assert_eq!(
                body["imp"][0].get("bidfloor").and_then(Value::as_f64),
                (floor != 0.0_f64).then_some(floor)
            );
            assert_eq!(body["imp"][0]["ext"]["bidder"]["floor"], json!(floor));
            assert_eq!(request.imp[0].bidfloor, Some(5.0_f64));
        }
    }

    #[test]
    fn missing_bidder_containers_refuse_without_inventing_parameters() {
        for impressions in [
            json!([{"id":"i", "banner":{}}]),
            json!([{"id":"i","banner":{},"ext":{}}]),
        ] {
            let request = request(&impressions);
            let (semantic, calls) = harness::drive(&TripleliftBidder, &request, &info());
            assert_eq!(
                semantic.unwrap_err(),
                [BidderError::unknown("No valid impressions for triplelift")]
            );
            assert!(calls.is_empty());
        }
    }

    #[test]
    fn explicit_null_bidder_uses_source_defaults_without_erasing_wire_presence() {
        let request = request(&json!([{"id":"i", "banner":{}, "tagid":"old",
            "bidfloor":1.25_f64, "ext":{"bidder":null}}]));
        assert!(matches!(
            request.imp[0].ext.as_ref().unwrap().bidder,
            JsonField::Null
        ));
        let calls = harness::drive_ok(&TripleliftBidder, &request, &info());
        assert_eq!(calls.len(), 1);
        let body: Value = serde_json::from_slice(calls[0].body()).unwrap();
        assert!(body["imp"][0].get("tagid").is_none());
        assert_eq!(body["imp"][0]["bidfloor"], json!(1.25_f64));
        assert_eq!(body["imp"][0]["ext"].get("bidder"), Some(&Value::Null));
        assert_eq!(request.imp[0].other["tagid"], json!("old"));
    }

    #[test]
    fn invalid_parameter_and_media_rejections_are_independent_and_emit_no_calls() {
        for (parameters, media, expected) in [
            (
                json!(false),
                true,
                ConstructionDiagnostic::InvalidImpressionParameters,
            ),
            (
                json!([]),
                true,
                ConstructionDiagnostic::InvalidImpressionParameters,
            ),
            (
                json!({"inventoryCode":3_i32}),
                true,
                ConstructionDiagnostic::InvalidImpressionParameters,
            ),
            (
                json!({"floor":"bad"}),
                true,
                ConstructionDiagnostic::InvalidImpressionParameters,
            ),
            (
                json!({}),
                false,
                ConstructionDiagnostic::UnsupportedMediaType,
            ),
            (
                json!({"floor":false}),
                false,
                ConstructionDiagnostic::InvalidImpressionParameters,
            ),
        ] {
            let mut impression =
                json!({"id":"i", "native":{}, "audio":{}, "ext":{"bidder":parameters}});
            if media {
                impression["banner"] = json!({});
            }
            let request = request(&json!([impression]));
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &TripleliftBidder,
                &request,
                &info(),
                SinkLimits::new(32, 1),
                3,
                &EndpointPolicy(true),
            );
            assert_eq!(
                semantic.unwrap_err(),
                [BidderError::unknown("No valid impressions for triplelift")]
            );
            assert_eq!(
                outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
                [expected]
            );
            assert!(outcome.accepted.is_empty());
            assert!(outcome.errors.is_empty());
            assert_eq!(remaining, 3);
        }
    }

    #[test]
    fn diagnostics_are_bounded_and_preserve_the_valid_sibling() {
        let mut impressions: Vec<_> = (0..40_usize)
            .map(|index| json!({"id":format!("bad-{index}"), "banner":{},"ext":{"bidder":false}}))
            .collect();
        impressions.push(json!({"id":"i", "banner":{},"ext":{"bidder":{"inventoryCode":"slot"}}}));
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &TripleliftBidder,
            &request(&json!(impressions)),
            &info(),
            SinkLimits::new(4096, 1),
            1,
            &EndpointPolicy(true),
        );
        semantic.unwrap();
        assert_eq!(remaining, 0);
        assert_eq!(outcome.accepted.len(), 1);
        assert_eq!(
            outcome.accepted[0].covered_imp_ids().collect::<Vec<_>>(),
            ["i"]
        );
        let actual = outcome.diagnostics.iter().copied().collect::<Vec<_>>();
        let mut expected = vec![ConstructionDiagnostic::InvalidImpressionParameters; 32];
        expected[31] = ConstructionDiagnostic::AdditionalDiagnostics;
        assert_eq!(actual, expected);
        assert!(outcome.errors.is_empty());
    }

    #[test]
    fn raw_bytes_impression_slots_calls_and_endpoint_policy_bind_independently() {
        let request = minimal();
        let calls = harness::drive_ok(&TripleliftBidder, &request, &info());
        let bytes = calls[0].body().len();
        for (raw, ids, call_slots, allow, error, remaining) in [
            (bytes, 1_u16, 1_u32, true, None, 0_u32),
            (
                bytes - 1,
                1,
                1,
                true,
                Some(OutboundBuildError::OversizeRaw {
                    limit: u64::try_from(bytes - 1).unwrap(),
                }),
                1,
            ),
            (
                bytes,
                0,
                1,
                true,
                Some(OutboundBuildError::TooManyImpIds { limit: 0 }),
                1,
            ),
            (
                bytes,
                1,
                0,
                true,
                Some(OutboundBuildError::CallBudgetExhausted { max_calls: 0 }),
                0,
            ),
            (
                bytes,
                1,
                1,
                false,
                Some(OutboundBuildError::EndpointRejected {
                    reason: EndpointRejectReason::HostNotAllowed,
                }),
                1,
            ),
        ] {
            let (semantic, outcome, actual_remaining) = harness::drive_with_limits(
                &TripleliftBidder,
                &request,
                &info(),
                SinkLimits::new(raw, ids),
                call_slots,
                &EndpointPolicy(allow),
            );
            semantic.unwrap();
            assert_eq!(outcome.accepted.len(), usize::from(error.is_none()));
            assert_eq!(outcome.errors, error.into_iter().collect::<Vec<_>>());
            assert_eq!(actual_remaining, remaining);
        }
    }
}
