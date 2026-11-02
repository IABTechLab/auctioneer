use crate::bidders::url::{json_utf8_headers, write_endpoint};
use crate::correlation::{CorrelationError, CorrelationMedia};
use crate::{BidderError, BidderRequestInfo, ConstructionDiagnostic, OutboundRequestSink};
use auctioneer_openrtb::{
    ext::imp::ImpExt,
    types::{
        bid_request::{BidRequest, Imp, Video},
        field::JsonField,
    },
};
use serde::{
    Serialize, Serializer,
    ser::{SerializeMap as _, SerializeSeq as _},
};
use serde_json::{Map, Number, Value};
use std::slice::from_ref;

pub(super) fn make_requests(
    request: &BidRequest,
    info: &BidderRequestInfo<'_>,
    sink: &mut OutboundRequestSink<'_>,
) -> Result<(), Vec<BidderError>> {
    let mut last_aggregate = None;
    let mut has_valid = false;
    let mut has_invalid = false;
    // Keep only borrowed group state. Reservation still precedes all retained output.
    for imp in &request.imp {
        let Some(media) = selected_media(imp) else {
            continue;
        };
        if let Ok(params) = parameters(imp) {
            has_valid = true;
            if media != CorrelationMedia::Video {
                last_aggregate = Some(params);
            }
        } else {
            has_invalid = true;
            sink.diagnostic(ConstructionDiagnostic::InvalidImpressionParameters);
        }
    }
    if let Some(params) = last_aggregate {
        send_group(
            request,
            RequestGroup::Aggregate(&request.imp),
            params,
            info,
            sink,
        );
    }
    for imp in &request.imp {
        if selected_media(imp) == Some(CorrelationMedia::Video)
            && let Ok(params) = parameters(imp)
        {
            send_group(request, RequestGroup::Video(imp), params, info, sink);
        }
    }
    // Sink refusals are call-local; they must not discard accepted sibling calls.
    if !has_valid && has_invalid {
        Err(vec![BidderError::bad_input("no valid openx impressions")])
    } else {
        Ok(())
    }
}

pub(super) fn known<'a>(
    fields: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a Value>, ()> {
    let mut matches = fields
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case(name));
    let value = matches.next().map(|(_, value)| value);
    if matches.next().is_some() {
        Err(())
    } else {
        Ok(value)
    }
}

#[derive(Clone, Copy, Default)]
enum Unit<'a> {
    #[default]
    Empty,
    Text(&'a str),
    Number(&'a Number),
}

impl Unit<'_> {
    fn is_empty(self) -> bool {
        matches!(self, Self::Empty | Self::Text(""))
    }
}

impl Serialize for Unit<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Empty => serializer.serialize_str(""),
            Self::Text(value) => serializer.serialize_str(value),
            Self::Number(value) => serializer.collect_str(value),
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Parameters<'a> {
    unit: Unit<'a>,
    domain: &'a str,
    platform: &'a str,
    floor: f64,
    custom: Option<&'a Map<String, Value>>,
}

fn parameter_string<'a>(fields: &'a Map<String, Value>, name: &str) -> Result<&'a str, ()> {
    match known(fields, name)? {
        None | Some(Value::Null) => Ok(""),
        Some(Value::String(value)) => Ok(value),
        Some(Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_)) => Err(()),
    }
}

fn parameters(imp: &Imp) -> Result<Parameters<'_>, ()> {
    let ext = imp.ext.as_ref().ok_or(())?;
    let alias = known(&ext.other, "bidder")?;
    let payload = match (&ext.bidder, alias) {
        (JsonField::Missing, Some(value)) | (JsonField::Value(value), None) => value,
        (JsonField::Null, None) => return Ok(Parameters::default()),
        (JsonField::Missing, None) | (JsonField::Null | JsonField::Value(_), Some(_)) => {
            return Err(());
        }
    };
    let fields = match payload {
        Value::Null => return Ok(Parameters::default()),
        Value::Object(fields) => fields,
        Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => return Err(()),
    };
    let unit = match known(fields, "unit")? {
        None | Some(Value::Null) => Unit::Empty,
        Some(Value::String(value)) => Unit::Text(value),
        Some(Value::Number(value)) => Unit::Number(value),
        Some(Value::Bool(_) | Value::Array(_) | Value::Object(_)) => return Err(()),
    };
    let floor = match known(fields, "customFloor")? {
        None | Some(Value::Null) => 0.0_f64,
        Some(Value::Number(value)) => value.as_f64().unwrap_or_default(),
        Some(Value::String(value)) => value.parse::<f64>().unwrap_or_default(),
        Some(Value::Bool(_) | Value::Array(_) | Value::Object(_)) => return Err(()),
    };
    let custom = match known(fields, "customParams")? {
        None | Some(Value::Null) => None,
        Some(Value::Object(custom_fields)) => Some(custom_fields),
        Some(Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_)) => {
            return Err(());
        }
    };
    Ok(Parameters {
        unit,
        domain: parameter_string(fields, "delDomain")?,
        platform: parameter_string(fields, "platform")?,
        floor: if floor.is_finite() && floor > 0.0_f64 {
            floor
        } else {
            0.0_f64
        },
        custom,
    })
}

fn selected_media(imp: &Imp) -> Option<CorrelationMedia> {
    if imp.banner.is_some() {
        Some(CorrelationMedia::Banner)
    } else if imp.video.is_some() {
        Some(CorrelationMedia::Video)
    } else if imp.native.is_some() {
        Some(CorrelationMedia::Native)
    } else {
        None
    }
}

#[derive(Clone, Copy)]
enum RequestGroup<'a> {
    Aggregate(&'a [Imp]),
    Video(&'a Imp),
}

impl<'a> RequestGroup<'a> {
    fn impressions(self) -> impl Iterator<Item = Impression<'a>> {
        let (imps, video) = match self {
            Self::Aggregate(imps) => (imps, false),
            Self::Video(imp) => (from_ref(imp), true),
        };
        imps.iter().filter_map(move |imp| {
            let media = selected_media(imp)?;
            if (media == CorrelationMedia::Video) != video {
                return None;
            }
            parameters(imp)
                .ok()
                .map(|params| Impression { imp, params, media })
        })
    }
}

fn send_group(
    request: &BidRequest,
    group: RequestGroup<'_>,
    params: Parameters<'_>,
    info: &BidderRequestInfo<'_>,
    sink: &mut OutboundRequestSink<'_>,
) {
    let _recorded = sink.write_json(
        info,
        |meta| {
            meta.set_method("POST")?;
            write_endpoint(meta, info)?;
            json_utf8_headers(meta)?;
            for (index, impression) in group.impressions().enumerate() {
                meta.imp_id(&impression.imp.id)?;
                let ordinal =
                    u16::try_from(index).map_err(|_error| CorrelationError::UnknownOrdinal)?;
                meta.correlate_imp(ordinal, impression.media, 0)?;
            }
            Ok(())
        },
        &Request {
            input: request,
            group,
            params,
        },
    );
}

struct Request<'a> {
    input: &'a BidRequest,
    group: RequestGroup<'a>,
    params: Parameters<'a>,
}

impl Serialize for Request<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let BidRequest {
            id,
            imp: _selected,
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
            ext: _replaced,
            other,
        } = self.input;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", id)?;
        map.serialize_entry("imp", &self.group)?;
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
        map.serialize_entry("ext", &RequestExtension(&self.params))?;
        for (key, value) in other {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

struct RequestExtension<'a>(&'a Parameters<'a>);

impl Serialize for RequestExtension<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("bc", "hb_pbs_1.0.0")?;
        if !self.0.domain.is_empty() {
            map.serialize_entry("delDomain", self.0.domain)?;
        }
        if !self.0.platform.is_empty() {
            map.serialize_entry("platform", self.0.platform)?;
        }
        map.end()
    }
}

impl Serialize for RequestGroup<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(None)?;
        for impression in self.impressions() {
            sequence.serialize_element(&impression)?;
        }
        sequence.end()
    }
}

struct Impression<'a> {
    imp: &'a Imp,
    params: Parameters<'a>,
    media: CorrelationMedia,
}

impl Serialize for Impression<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Imp {
            id,
            banner,
            video,
            native,
            audio,
            bidfloor,
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
            let rewarded = other.get("rwdd").and_then(Value::as_i64) == Some(1);
            map.serialize_entry(
                "video",
                &VideoView {
                    video: value,
                    rewarded,
                },
            )?;
        }
        if let Some(value) = native {
            map.serialize_entry("native", value)?;
        }
        if let Some(value) = audio {
            map.serialize_entry("audio", value)?;
        }
        if !self.params.unit.is_empty() {
            map.serialize_entry("tagid", &self.params.unit)?;
        }
        let floor = bidfloor
            .filter(|value| *value != 0.0_f64)
            .unwrap_or(self.params.floor);
        if floor != 0.0_f64 {
            map.serialize_entry("bidfloor", &floor)?;
        }
        if let Some(value) = bidfloorcur {
            map.serialize_entry("bidfloorcur", value)?;
        }
        if let Some(value) = secure {
            map.serialize_entry("secure", value)?;
        }
        if let Some(impression_ext) = ext {
            let extension = ImpressionExtension {
                ext: impression_ext,
                custom: self.params.custom,
            };
            if !extension.is_empty() {
                map.serialize_entry("ext", &extension)?;
            }
        }
        for (key, value) in other {
            if key != "tagid" {
                map.serialize_entry(key, value)?;
            }
        }
        map.end()
    }
}

struct ImpressionExtension<'a> {
    ext: &'a ImpExt,
    custom: Option<&'a Map<String, Value>>,
}

impl ImpressionExtension<'_> {
    fn is_empty(&self) -> bool {
        self.ext.tid.is_none()
            && self.custom.is_none()
            && !self
                .ext
                .other
                .keys()
                .any(|key| key != "bidder" && key != "prebid")
    }
}

impl Serialize for ImpressionExtension<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let ImpExt {
            tid,
            prebid: _removed,
            bidder: _removed_bidder,
            other,
        } = self.ext;
        let mut map = serializer.serialize_map(None)?;
        if let Some(value) = tid {
            map.serialize_entry("tid", value)?;
        }
        for (key, value) in other {
            if key != "bidder"
                && key != "prebid"
                && (key != "customParams" || self.custom.is_none())
            {
                map.serialize_entry(key, value)?;
            }
        }
        if let Some(value) = self.custom {
            map.serialize_entry("customParams", value)?;
        }
        map.end()
    }
}

struct VideoView<'a> {
    video: &'a Video,
    rewarded: bool,
}

#[derive(Serialize)]
struct Rewarded {
    rewarded: u8,
}

impl Serialize for VideoView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Video {
            mimes,
            minduration,
            maxduration,
            w,
            h,
            ext: _replaced,
            other,
        } = self.video;
        let mut map = serializer.serialize_map(None)?;
        if !mimes.is_missing() {
            map.serialize_entry("mimes", mimes)?;
        }
        if let Some(value) = minduration {
            map.serialize_entry("minduration", value)?;
        }
        if let Some(value) = maxduration {
            map.serialize_entry("maxduration", value)?;
        }
        if let Some(value) = w {
            map.serialize_entry("w", value)?;
        }
        if let Some(value) = h {
            map.serialize_entry("h", value)?;
        }
        if self.rewarded {
            map.serialize_entry("ext", &Rewarded { rewarded: 1 })?;
        }
        for (key, value) in other {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use super::super::OpenxBidder;
    use super::super::tests::{ENDPOINT, imp, request};
    use crate::bidders::harness;
    use crate::correlation::CorrelationMedia;
    use crate::{
        BidderHttpRequest, BidderName, BidderRequestInfo, ConstructionDiagnostic,
        EndpointRejectReason, EndpointValidator, EntryPoint, OutboundBuildError, SinkLimits,
    };
    use auctioneer_openrtb::types::field::JsonField;
    use serde_json::{Value, json};

    fn info() -> BidderRequestInfo<'static> {
        BidderRequestInfo::new(
            BidderName::new("fake_alias"),
            BidderName::new("openx"),
            "fake_account",
            None,
            ENDPOINT,
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        )
    }

    fn body(call: &BidderHttpRequest) -> Value {
        serde_json::from_slice(call.body()).unwrap()
    }

    struct AllowEndpoint;

    impl EndpointValidator for AllowEndpoint {
        fn validate(
            &self,
            _url: &url::Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            Ok(())
        }
    }

    #[test]
    fn groups_banner_native_before_individual_video_calls_with_exact_coverage() {
        let request = request(json!([
            imp("fake_video_a", "video"),
            imp("fake_banner", "banner"),
            imp("fake_native", "native"),
            imp("fake_video_b", "video")
        ]));
        let before = serde_json::to_value(&request).unwrap();
        let calls = harness::drive_ok(&OpenxBidder, &request, &info());
        assert_eq!(calls.len(), 3);
        assert_eq!(
            calls[0].covered_imp_ids().collect::<Vec<_>>(),
            ["fake_banner", "fake_native"]
        );
        assert_eq!(
            calls[1].covered_imp_ids().collect::<Vec<_>>(),
            ["fake_video_a"]
        );
        assert_eq!(
            calls[2].covered_imp_ids().collect::<Vec<_>>(),
            ["fake_video_b"]
        );
        for call in &calls {
            assert_eq!(call.method(), "POST");
            assert_eq!(call.url(), ENDPOINT);
            assert_eq!(
                call.headers().collect::<Vec<_>>(),
                [
                    ("Content-Type", "application/json;charset=utf-8"),
                    ("Accept", "application/json")
                ]
            );
            assert_eq!(
                body(call)["ext"],
                json!({"bc":"hb_pbs_1.0.0", "delDomain":"fake.example.test"})
            );
        }
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn grouping_precedence_records_fallback_without_removing_other_media() {
        let mut banner = imp("fake_banner", "banner");
        banner["video"] = json!({"mimes":["video/mp4"]});
        banner["native"] = json!({});
        let mut video = imp("fake_video", "video");
        video["native"] = json!({});
        let calls = harness::drive_ok(&OpenxBidder, &request(json!([banner, video])), &info());
        assert_eq!(calls.len(), 2);
        assert!(body(&calls[0])["imp"][0].get("video").is_some());
        assert!(body(&calls[0])["imp"][0].get("native").is_some());
        assert!(body(&calls[1])["imp"][0].get("native").is_some());
        assert_eq!(
            calls[0].meta().imp_correlation(0).unwrap().media(),
            CorrelationMedia::Banner
        );
        assert_eq!(
            calls[1].meta().imp_correlation(0).unwrap().media(),
            CorrelationMedia::Video
        );
    }

    #[test]
    fn last_valid_impression_sets_group_domain_and_platform_without_cross_call_leakage() {
        let mut first = imp("fake_first", "banner");
        first["ext"]["bidder"]["platform"] = json!("fake_platform_a");
        let mut last = imp("fake_last", "native");
        last["ext"]["bidder"] = json!({"unit":"456", "platform":"fake_platform_b"});
        let video = imp("fake_video", "video");
        let calls = harness::drive_ok(&OpenxBidder, &request(json!([first, last, video])), &info());
        assert_eq!(calls.len(), 2);
        assert_eq!(
            body(&calls[0])["ext"],
            json!({"bc":"hb_pbs_1.0.0", "platform":"fake_platform_b"})
        );
        assert_eq!(
            body(&calls[1])["ext"],
            json!({"bc":"hb_pbs_1.0.0", "delDomain":"fake.example.test"})
        );
    }

    #[test]
    fn scoped_parameters_strip_only_exact_bidder_and_prebid_extension_keys() {
        let mut value = imp("fake_imp", "banner");
        value["ext"] = json!({
            "bidder":{"unit":"123", "customParams":{"fake_key":[1_i32, "two", null]}},
            "prebid":{"fake_discard":true}, "ae":1_i32, "data":{"fake_keep":true},
            "tid":"fake_tid", "other":null, "customParams":{"fake_old":true}
        });
        let mut request = request(json!([value]));
        request.ext = serde_json::from_value(json!({"fake_discard":true})).unwrap();
        let before = serde_json::to_value(&request).unwrap();
        let calls = harness::drive_ok(&OpenxBidder, &request, &info());
        assert_eq!(calls.len(), 1);
        assert_eq!(
            body(&calls[0])["imp"][0]["ext"],
            json!({
                "ae":1_i32, "data":{"fake_keep":true}, "tid":"fake_tid", "other":null,
                "customParams":{"fake_key":[1_i32,"two",null]}
            })
        );
        assert_eq!(body(&calls[0])["ext"], json!({"bc":"hb_pbs_1.0.0"}));
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn numeric_or_decimal_string_unit_becomes_tagid_and_empty_extension_is_omitted() {
        for (unit, expected) in [(json!(123_i32), "123"), (json!("00123"), "00123")] {
            let mut value = imp("fake_imp", "banner");
            value["ext"]["bidder"]["unit"] = unit;
            let calls = harness::drive_ok(&OpenxBidder, &request(json!([value])), &info());
            assert_eq!(calls.len(), 1);
            let outgoing = body(&calls[0]);
            assert_eq!(outgoing["imp"][0]["tagid"], expected);
            assert!(outgoing["imp"][0].get("ext").is_none());
        }
    }

    #[test]
    fn custom_floor_only_replaces_zero_floor_and_keeps_currency() {
        for custom in [json!(0.4_f64), json!("0.4")] {
            for (floor, expected) in [(0.0_f64, 0.4_f64), (0.7_f64, 0.7_f64)] {
                let mut value = imp("fake_imp", "banner");
                value["bidfloor"] = json!(floor);
                value["bidfloorcur"] = json!("EUR");
                value["ext"]["bidder"]["customFloor"] = custom.clone();
                let calls = harness::drive_ok(&OpenxBidder, &request(json!([value])), &info());
                assert_eq!(calls.len(), 1);
                assert_eq!(body(&calls[0])["imp"][0]["bidfloor"], expected);
                assert_eq!(body(&calls[0])["imp"][0]["bidfloorcur"], "EUR");
            }
        }
    }

    #[test]
    fn rewarded_replaces_video_extension_and_nonrewarded_removes_it() {
        for (reward, expected) in [
            (0_i32, None),
            (1_i32, Some(json!({"rewarded":1_i32}))),
            (2_i32, None),
        ] {
            let mut value = imp("fake_imp", "video");
            value["video"] = json!({"mimes":["video/mp4"], "ext":{"fake_discard":true}});
            value["rwdd"] = json!(reward);
            let request = request(json!([value]));
            let before = serde_json::to_value(&request).unwrap();
            let calls = harness::drive_ok(&OpenxBidder, &request, &info());
            assert_eq!(calls.len(), 1);
            assert_eq!(
                body(&calls[0])["imp"][0]["video"].get("ext"),
                expected.as_ref()
            );
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
    }

    #[test]
    fn invalid_impression_does_not_poison_valid_group_or_last_valid_parameters() {
        let mut bad = imp("fake_bad", "banner");
        bad["ext"]["bidder"] = json!({"unit":{}, "delDomain":"SECRET_DOMAIN"});
        let request = request(json!([
            imp("fake_good", "banner"),
            bad,
            imp("fake_video", "video")
        ]));
        let (semantic, outcome, _) = harness::drive_with_limits(
            &OpenxBidder,
            &request,
            &info(),
            SinkLimits::new(8192, 100),
            4,
            &AllowEndpoint,
        );
        semantic.unwrap();
        assert_eq!(outcome.accepted.len(), 2);
        assert_eq!(
            outcome.accepted[0].covered_imp_ids().collect::<Vec<_>>(),
            ["fake_good"]
        );
        assert_eq!(
            body(&outcome.accepted[0])["ext"]["delDomain"],
            "fake.example.test"
        );
        assert_eq!(
            outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
            [ConstructionDiagnostic::InvalidImpressionParameters]
        );
        assert!(!format!("{:?}", outcome.diagnostics).contains("SECRET_DOMAIN"));
    }

    #[test]
    fn missing_bidder_refuses_but_null_and_empty_object_keep_source_direct_adapter_defaults() {
        for params in [Value::Null, json!({})] {
            let mut value = imp("fake_imp", "banner");
            value["ext"]["bidder"] = params;
            let calls = harness::drive_ok(&OpenxBidder, &request(json!([value])), &info());
            assert_eq!(calls.len(), 1);
            assert_eq!(body(&calls[0])["ext"], json!({"bc":"hb_pbs_1.0.0"}));
            assert!(body(&calls[0])["imp"][0].get("tagid").is_none());
        }
        let mut value = imp("fake_imp", "banner");
        value["ext"] = json!({"fake_keep":true});
        let (semantic, calls) = harness::drive(&OpenxBidder, &request(json!([value])), &info());
        assert!(semantic.is_err());
        assert!(calls.is_empty());
    }

    #[test]
    fn one_case_equivalent_parameter_spelling_is_accepted() {
        let mut value = imp("fake_imp", "banner");
        value["ext"]["bidder"] =
            json!({"UNIT":"123", "DELDOMAIN":"fake.example.test", "CUSTOMFLOOR":"0.4"});
        let calls = harness::drive_ok(&OpenxBidder, &request(json!([value])), &info());
        assert_eq!(calls.len(), 1);
        assert_eq!(body(&calls[0])["imp"][0]["tagid"], "123");
        assert_eq!(body(&calls[0])["imp"][0]["bidfloor"], 0.4_f64);
    }

    #[test]
    fn ambiguous_parameter_spellings_refuse_only_affected_impression_even_if_equal_or_null() {
        for other in [json!("123"), Value::Null] {
            let mut bad = imp("fake_bad", "banner");
            bad["ext"]["bidder"]["UNIT"] = other;
            let calls = harness::drive_ok(
                &OpenxBidder,
                &request(json!([bad, imp("fake_good", "banner")])),
                &info(),
            );
            assert_eq!(calls.len(), 1);
            assert_eq!(
                calls[0].covered_imp_ids().collect::<Vec<_>>(),
                ["fake_good"]
            );
        }
    }

    #[test]
    fn audio_only_and_empty_input_never_create_empty_calls() {
        for impressions in [json!([]), json!([imp("fake_audio", "audio")])] {
            let (_semantic, calls) = harness::drive(&OpenxBidder, &request(impressions), &info());
            assert!(calls.is_empty());
        }
    }

    #[test]
    fn exhausted_call_budget_preserves_first_group_and_exact_sent_coverage() {
        let request = request(json!([
            imp("fake_banner", "banner"),
            imp("fake_a", "video"),
            imp("fake_b", "video")
        ]));
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &OpenxBidder,
            &request,
            &info(),
            SinkLimits::new(8192, 100),
            1,
            &AllowEndpoint,
        );
        semantic.unwrap();
        assert_eq!(remaining, 0);
        assert_eq!(outcome.accepted.len(), 1);
        assert_eq!(
            outcome.accepted[0].covered_imp_ids().collect::<Vec<_>>(),
            ["fake_banner"]
        );
        assert_eq!(
            outcome.errors,
            [OutboundBuildError::CallBudgetExhausted { max_calls: 1 }]
        );
        assert_eq!(outcome.rejected_call_count, 2_u32);
    }

    #[test]
    fn oversize_group_refunds_slot_for_later_small_video() {
        let mut large = imp("fake_large", "banner");
        large["ext"]["fake_payload"] = json!("x".repeat(4096));
        let request = request(json!([large, imp("fake_small", "video")]));
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &OpenxBidder,
            &request,
            &info(),
            SinkLimits::new(1024, 100),
            1,
            &AllowEndpoint,
        );
        semantic.unwrap();
        assert_eq!(remaining, 0);
        assert_eq!(outcome.accepted.len(), 1);
        assert_eq!(
            outcome.accepted[0].covered_imp_ids().collect::<Vec<_>>(),
            ["fake_small"]
        );
        assert_eq!(
            outcome.errors,
            [OutboundBuildError::OversizeRaw { limit: 1024 }]
        );
        assert_eq!(outcome.rejected_call_count, 0_u32);
    }

    #[test]
    fn shared_imp_prebid_null_presence_survives_alongside_an_ascii_alias() {
        let request = request(json!([{
            "id":"fake_imp", "banner":{}, "ext":{"bidder":{}, "prebid":null, "PREBID":null}
        }]));
        let extension = request.imp[0].ext.as_ref().unwrap();
        assert!(matches!(extension.prebid, JsonField::Null));
        assert!(!extension.other.contains_key("prebid"));
        assert_eq!(extension.other.get("PREBID"), Some(&Value::Null));
    }
}
