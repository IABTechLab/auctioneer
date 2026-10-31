use crate::bidders::url::write_endpoint;
use crate::{
    BidderError, BidderRequestInfo, ConstructionDiagnostic, META_BIDDER_REGION_BYTES,
    OutboundRequestMetaBuilder, OutboundRequestSink, SinkAbort,
};
use auctioneer_openrtb::types::bid_request::{BidRequest, Imp, Video};
use serde::{
    Serialize, Serializer,
    ser::{Error as _, SerializeMap as _, SerializeSeq as _},
};
use serde_json::Value;

pub(super) fn make_requests(
    request: &BidRequest,
    info: &BidderRequestInfo<'_>,
    sink: &mut OutboundRequestSink<'_>,
) -> Result<(), Vec<BidderError>> {
    let mut has_survivor = false;
    for imp in &request.imp {
        match prepare(imp) {
            Ok(prepared) => {
                if prepared.floor.is_some_and(|floor| !floor.is_finite()) {
                    return Err(vec![BidderError::failed_to_request_bids(
                        "sovrn request contains a non-finite floor",
                    )]);
                }
                has_survivor = true;
            }
            Err(reason) => sink.diagnostic(reason),
        }
    }
    if !has_survivor {
        return if request.imp.is_empty() {
            Ok(())
        } else {
            Err(vec![BidderError::bad_input(
                "No valid impressions for sovrn",
            )])
        };
    }
    let _recorded = sink.write_json(
        info,
        |meta| {
            meta.set_method("POST")?;
            write_endpoint(meta, info)?;
            headers(meta, request)?;
            for imp in request.imp.iter().filter_map(|imp| prepare(imp).ok()) {
                meta.imp_id(&imp.imp.id)?;
            }
            Ok(())
        },
        &Request(request),
    );
    Ok(())
}

fn headers(
    meta: &mut OutboundRequestMetaBuilder<'_>,
    request: &BidRequest,
) -> Result<(), SinkAbort> {
    let over_cap = |_error| SinkAbort::MetaOverCap {
        limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap_or(u64::MAX),
    };
    meta.header("Content-Type", "application/json")?;
    if let Some(device) = &request.device {
        for (name, candidate) in [
            ("User-Agent", device.ua.as_deref()),
            ("X-Forwarded-For", device.ip.as_deref()),
            (
                "Accept-Language",
                device.language.as_ref().map(String::as_str),
            ),
        ] {
            if let Some(header_value) = candidate.filter(|entry| !entry.is_empty()) {
                meta.header(name, header_value)?;
            }
        }
        if let Some(dnt) = device.dnt {
            meta.header_with("DNT", |writer| write!(writer, "{dnt}").map_err(over_cap))?;
        }
    }
    if let Some(raw_buyeruid) = request
        .user
        .as_ref()
        .and_then(|user| user.buyeruid.as_deref())
    {
        let buyeruid = raw_buyeruid.trim();
        if !buyeruid.is_empty() {
            meta.header_with("Cookie", |writer| {
                write!(writer, "ljt_reader={buyeruid}").map_err(over_cap)
            })?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Parameter {
    Tag,
    CamelTag,
    Floor,
    AdUnit,
}

fn binds(key: &str, field: Parameter) -> bool {
    if key == "tagId" {
        return field == Parameter::CamelTag;
    }
    let name = match field {
        Parameter::Tag => "tagid",
        Parameter::CamelTag => return false,
        Parameter::Floor => "bidfloor",
        Parameter::AdUnit => "adunitcode",
    };
    // The colliding exact tag declarations force jsoniter's general Unicode path.
    key.chars()
        .map(|character| match character {
            '\u{0130}' => 'i',
            _ => character.to_ascii_lowercase(),
        })
        .eq(name.chars())
}

fn parameter(
    fields: &serde_json::Map<String, Value>,
    name: Parameter,
) -> Result<Option<&Value>, ConstructionDiagnostic> {
    let mut found = fields.iter().filter(|(key, _)| binds(key, name));
    let matched = found.next().map(|(_, field_value)| field_value);
    if found.next().is_some() {
        return Err(ConstructionDiagnostic::InvalidImpressionParameters);
    }
    Ok(matched)
}

fn string_parameter(entry: Option<&Value>) -> Result<&str, ConstructionDiagnostic> {
    match entry {
        None | Some(Value::Null) => Ok(""),
        Some(value) => value
            .as_str()
            .ok_or(ConstructionDiagnostic::InvalidImpressionParameters),
    }
}

fn extension_floor(entry: Option<&Value>) -> Option<f64> {
    match entry {
        Some(Value::String(text)) => {
            let unsigned = text
                .strip_prefix('+')
                .or_else(|| text.strip_prefix('-'))
                .unwrap_or(text);
            if unsigned.eq_ignore_ascii_case("inf") || unsigned.eq_ignore_ascii_case("infinity") {
                return (!text.starts_with('-')).then_some(f64::INFINITY);
            }
            // Ignore decimal overflow because source ParseFloat returns ErrRange.
            // Legacy hexadecimal/underscore spellings are intentionally unsupported.
            text.parse::<f64>()
                .ok()
                .filter(|floor| floor.is_finite() && *floor > 0.0_f64)
        }
        Some(Value::Number(number)) => number.as_f64().filter(|floor| *floor > 0.0_f64),
        None | Some(Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_)) => None,
    }
}

struct Impression<'a> {
    imp: &'a Imp,
    tag: &'a str,
    floor: Option<f64>,
}

fn prepare(imp: &Imp) -> Result<Impression<'_>, ConstructionDiagnostic> {
    let invalid = ConstructionDiagnostic::InvalidImpressionParameters;
    let ext = imp.ext.as_ref().ok_or(invalid)?;
    let mut present = !ext.bidder.is_missing();
    let mut bidder = ext.bidder.as_ref();
    for (key, value) in &ext.other {
        if key.eq_ignore_ascii_case("bidder") {
            if present {
                return Err(invalid);
            }
            present = true;
            bidder = Some(value);
        }
    }
    // Source null parameters still lack the required tag.
    let Some(Value::Object(fields)) = bidder else {
        return Err(invalid);
    };
    let primary_tag = string_parameter(parameter(fields, Parameter::Tag)?)?;
    let camel = string_parameter(parameter(fields, Parameter::CamelTag)?)?;
    let tag = if primary_tag.is_empty() {
        camel
    } else {
        primary_tag
    };
    let supplied_floor = extension_floor(parameter(fields, Parameter::Floor)?);
    let _adunit = string_parameter(parameter(fields, Parameter::AdUnit)?)?;
    if tag.is_empty() {
        return Err(invalid);
    }
    if let Some(video) = &imp.video
        && (video.mimes.as_ref().is_none()
            || video.maxduration.unwrap_or_default() == 0_i32
            || !video
                .other
                .get("protocols")
                .and_then(Value::as_array)
                .is_some_and(|protocols| protocols.iter().all(|value| protocol(value).is_some())))
    {
        return Err(ConstructionDiagnostic::InvalidImpression);
    }
    let floor = imp
        .bidfloor
        .filter(|original_floor| *original_floor != 0.0_f64)
        .or(supplied_floor);
    Ok(Impression { imp, tag, floor })
}

fn protocol(value: &Value) -> Option<i8> {
    if value.is_null() {
        Some(0)
    } else {
        let number = value.as_i64()?;
        i8::try_from(number).ok()
    }
}

struct Request<'a>(&'a BidRequest);

impl Serialize for Request<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
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
            bidfloor: _bidfloor,
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
            map.serialize_entry("video", &VideoView(value))?;
        }
        if let Some(value) = native {
            map.serialize_entry("native", value)?;
        }
        if let Some(value) = audio {
            map.serialize_entry("audio", value)?;
        }
        map.serialize_entry("tagid", self.tag)?;
        if let Some(floor) = self.floor {
            map.serialize_entry("bidfloor", &floor)?;
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
        for (key, value) in other {
            if key != "tagid" {
                map.serialize_entry(key, value)?;
            }
        }
        map.end()
    }
}

struct VideoView<'a>(&'a Video);

impl Serialize for VideoView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Video {
            mimes,
            minduration,
            maxduration,
            w,
            h,
            ext,
            other,
        } = self.0;
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
        if let Some(value) = ext {
            map.serialize_entry("ext", value)?;
        }
        for (key, value) in other {
            if key == "protocols" {
                let protocols = value
                    .as_array()
                    .ok_or_else(|| S::Error::custom("invalid sovrn video protocols"))?;
                if !protocols.is_empty() {
                    map.serialize_entry(key, &Protocols(protocols))?;
                }
                continue;
            }
            // Source's int-valued maxextended uses omitempty; keep every nonzero value.
            if key != "maxextended" || value.as_i64() != Some(0) {
                map.serialize_entry(key, value)?;
            }
        }
        map.end()
    }
}

struct Protocols<'a>(&'a [Value]);

impl Serialize for Protocols<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for source_value in self.0 {
            let encoded_value = protocol(source_value)
                .ok_or_else(|| S::Error::custom("invalid sovrn video protocol"))?;
            sequence.serialize_element(&encoded_value)?;
        }
        sequence.end()
    }
}

#[cfg(test)]
mod tests {
    use super::super::{SovrnBidder, tests::request};
    use crate::bidders::harness;
    use crate::{
        BidderErrorKind, BidderName, BidderRequestInfo, ConstructionDiagnostic,
        EndpointRejectReason, EndpointValidator, EntryPoint, OutboundBuildError, SinkLimits,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
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

    fn info(endpoint: &str) -> BidderRequestInfo<'_> {
        BidderRequestInfo::new(
            BidderName::new("sovrn-alias"),
            BidderName::new("sovrn"),
            "account",
            None,
            endpoint,
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        )
    }

    fn impression(id: &str, parameters: Value) -> Value {
        let mut document = json!({"id":id, "banner":{}, "ext":{}});
        document["ext"]["bidder"] = parameters;
        document
    }

    fn body(parameters: Value) -> Value {
        let request = request(json!([impression("i", parameters)]));
        let calls = harness::drive_ok(
            &SovrnBidder,
            &request,
            &info("https://bidder.example.test/auction?fixed=1"),
        );
        assert_eq!(calls.len(), 1);
        serde_json::from_slice(calls[0].body()).unwrap()
    }

    fn assert_filtered(parameters: Value) {
        let request = request(json!([
            impression("bad", parameters),
            impression("good", json!({"tagid":"survivor"})),
        ]));
        let before = serde_json::to_value(&request).unwrap();
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &SovrnBidder,
            &request,
            &info("https://bidder.example.test/auction"),
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
            ["good"]
        );
        let actual: Value = serde_json::from_slice(outcome.accepted[0].body()).unwrap();
        assert_eq!(actual["imp"].as_array().unwrap().len(), 1);
        assert_eq!(actual["imp"][0]["tagid"], json!("survivor"));
        assert_eq!(
            outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
            [ConstructionDiagnostic::InvalidImpressionParameters]
        );
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn aggregate_call_transforms_only_survivors_and_preserves_scoped_input() {
        for count in [1_usize, 8, 100] {
            let mut impressions = vec![impression("bad", json!({}))];
            impressions.extend((0..count).map(|index| json!({
                "id":format!("i-{index}"), "banner":{"w":300_i32,"h":250_i32},
                "tagid":"old", "bidfloor":1.5_f64, "bidfloorcur":"EUR", "secure":1_i32,
                "native":{}, "audio":{}, "custom":[null,true],
                "ext":{"bidder":{"tagid":"new","bidfloor":4.2_f64,"adunitcode":"slot"}, "gpid":"g", "keep":{"x":1_i32}},
            })));
            let mut request = request(json!(impressions));
            request.other.insert("opaque".into(), json!([true, null]));
            let before = serde_json::to_value(&request).unwrap();
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &SovrnBidder,
                &request,
                &info("https://bidder.example.test/auction"),
                SinkLimits::new(256 * 1024, 100),
                1,
                &EndpointPolicy(true),
            );
            semantic.unwrap();
            assert_eq!(remaining, 0);
            assert!(outcome.errors.is_empty());
            assert_eq!(outcome.accepted.len(), 1);
            assert_eq!(outcome.diagnostics.len(), 1);
            let call = &outcome.accepted[0];
            assert_eq!(call.method(), "POST");
            assert_eq!(call.url(), "https://bidder.example.test/auction");
            assert_eq!(
                call.headers().collect::<Vec<_>>(),
                [("Content-Type", "application/json")]
            );
            assert_eq!(
                call.covered_imp_ids().collect::<Vec<_>>(),
                request.imp[1..]
                    .iter()
                    .map(|imp| imp.id.as_str())
                    .collect::<Vec<_>>()
            );
            let mut expected = before.clone();
            expected["imp"].as_array_mut().unwrap().remove(0);
            for imp in expected["imp"].as_array_mut().unwrap() {
                imp["tagid"] = json!("new");
            }
            assert_eq!(
                serde_json::from_slice::<Value>(call.body()).unwrap(),
                expected
            );
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
    }

    #[test]
    fn every_root_field_survives_the_impression_only_transform() {
        let request: BidRequest = serde_json::from_value(json!({
            "id":"r", "imp":[impression("i", json!({"tagid":"new"}))],
            "site":{"id":"site"}, "app":{"id":"app"}, "dooh":{"id":"dooh"},
            "device":{"make":"device"}, "user":{"id":"scoped"},
            "regs":{"coppa":1_i32}, "source":{"tid":"scoped"}, "at":2_i32,
            "tmax":100_i64, "cur":["EUR"], "test":1_i32, "ext":{"keep":true}, "opaque":[null],
        }))
        .unwrap();
        let before = serde_json::to_value(&request).unwrap();
        assert_eq!(before.as_object().unwrap().len(), 15);
        let calls = harness::drive_ok(
            &SovrnBidder,
            &request,
            &info("https://bidder.example.test/"),
        );
        assert_eq!(calls.len(), 1);
        let mut expected = before.clone();
        expected["imp"][0]["tagid"] = json!("new");
        assert_eq!(
            serde_json::from_slice::<Value>(calls[0].body()).unwrap(),
            expected
        );
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn identifying_headers_use_only_the_already_scoped_device_and_buyeruid() {
        for (field, value, name, expected) in [
            ("ua", json!("scoped-agent"), "User-Agent", "scoped-agent"),
            ("ip", json!("192.0.2.1"), "X-Forwarded-For", "192.0.2.1"),
            ("language", json!("en-US"), "Accept-Language", "en-US"),
            ("dnt", json!(0_i32), "DNT", "0"),
            ("dnt", json!(1_i32), "DNT", "1"),
        ] {
            let mut request = request(json!([impression("i", json!({"tagid":"slot"}))]));
            request.device = Some(serde_json::from_value(json!({field:value})).unwrap());
            let calls = harness::drive_ok(
                &SovrnBidder,
                &request,
                &info("https://bidder.example.test/"),
            );
            assert_eq!(calls.len(), 1);
            let headers = calls[0].headers().collect::<Vec<_>>();
            assert_eq!(headers.len(), 2);
            assert!(headers.contains(&("Content-Type", "application/json")));
            assert!(
                headers
                    .iter()
                    .any(|(key, header_value)| key.eq_ignore_ascii_case(name)
                        && *header_value == expected)
            );
        }
    }

    #[test]
    fn cookie_trims_buyeruid_but_never_uses_other_user_identifiers() {
        for (buyeruid, cookie) in [
            ("  scoped-reader\t", Some("ljt_reader=scoped-reader")),
            ("\u{0085}\u{00a0}reader\u{3000}", Some("ljt_reader=reader")),
            ("", None),
            (" \t\n", None),
        ] {
            let mut request = request(json!([impression("i", json!({"tagid":"slot"}))]));
            request.user = Some(
                serde_json::from_value(json!({"id":"not-buyeruid", "buyeruid":buyeruid})).unwrap(),
            );
            let before = serde_json::to_value(&request).unwrap();
            let calls = harness::drive_ok(
                &SovrnBidder,
                &request,
                &info("https://bidder.example.test/"),
            );
            assert_eq!(calls.len(), 1);
            assert_eq!(
                calls[0]
                    .headers()
                    .find(|(key, _)| key.eq_ignore_ascii_case("Cookie"))
                    .map(|(_, value)| value),
                cookie
            );
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
    }

    #[test]
    fn absent_empty_and_null_identifying_fields_do_not_regain_headers() {
        for fields in [
            json!({}),
            json!({"device":{},"user":{}}),
            json!({"device":{"ua":"","ip":"","language":""},"user":{"id":"not-buyeruid"}}),
            json!({"device":{"ua":null,"ip":null,"language":null,"dnt":null},"user":{"buyeruid":null}}),
            json!({"device":{"ipv6":"2001:db8::1","ifa":"not-ip"},"user":{"id":"not-buyeruid"}}),
        ] {
            let mut value = fields;
            value["id"] = json!("r");
            value["imp"] = json!([impression("i", json!({"tagid":"slot"}))]);
            let request: BidRequest = serde_json::from_value(value).unwrap();
            let calls = harness::drive_ok(
                &SovrnBidder,
                &request,
                &info("https://bidder.example.test/"),
            );
            assert_eq!(calls.len(), 1);
            assert_eq!(
                calls[0].headers().collect::<Vec<_>>(),
                [("Content-Type", "application/json")]
            );
        }
    }

    #[test]
    fn exact_tag_fields_coexist_and_lowercase_nonempty_tag_wins() {
        for (parameters, expected) in [
            (json!({"tagid":"lower","tagId":"camel"}), "lower"),
            (json!({"tagid":"same","tagId":"same"}), "same"),
            (json!({"tagid":"","tagId":"camel"}), "camel"),
            (json!({"tagid":null,"tagId":"camel"}), "camel"),
            (json!({"tagid":"lower","tagId":null}), "lower"),
            (json!({"tagid":" ","tagId":"camel"}), " "),
            (json!({"tagId":"camel","TAGID":"fallback"}), "fallback"),
            (json!({"tagId":"camel","TagId":null}), "camel"),
        ] {
            let actual = body(parameters.clone());
            assert_eq!(actual["imp"][0]["tagid"], json!(expected));
            assert_eq!(actual["imp"][0]["ext"]["bidder"], parameters);
        }
    }

    #[test]
    fn single_tag_spellings_bind_exactly_before_general_unicode_lowercase() {
        for key in ["tagid", "tagId", "TagId", "TAGID", "tagID", "tag\u{0130}d"] {
            assert_eq!(
                body(json!({key:"CaseSensitiveSlot"}))["imp"][0]["tagid"],
                json!("CaseSensitiveSlot")
            );
        }
    }

    #[test]
    fn each_general_decoder_parameter_key_has_an_independent_case_fallback() {
        for key in ["bidfloor", "BidFloor", "BIDFLOOR", "b\u{0130}dfloor"] {
            let actual = body(json!({"tagId":"slot", key:2.75_f64}));
            assert_eq!(actual["imp"][0]["bidfloor"], json!(2.75_f64));
        }
        for key in [
            "adunitcode",
            "AdUnitCode",
            "ADUNITCODE",
            "adun\u{0130}tcode",
        ] {
            let parameters = json!({"tagId":"slot", key:"unit"});
            let actual = body(parameters.clone());
            assert_eq!(actual["imp"][0]["tagid"], json!("slot"));
            assert_eq!(actual["imp"][0]["ext"]["bidder"], parameters);
        }
    }

    #[test]
    fn competing_spellings_binding_to_one_field_filter_only_that_impression() {
        for parameters in [
            json!({"tagid":"a","TAGID":"b"}),
            json!({"tagid":"a","TagId":"a"}),
            json!({"tagid":null,"TAGID":"a"}),
            json!({"TagId":"a","TAGID":"b"}),
            json!({"TagId":null,"TAGID":null}),
            json!({"tagId":"distinct","TAGID":"a","TagID":"b"}),
            json!({"tagid":"a","tag\u{0130}d":"b"}),
            json!({"tagId":"slot","bidfloor":1_i32,"BIDFLOOR":2_i32}),
            json!({"tagId":"slot","BidFloor":1_i32,"BIDFLOOR":1_i32}),
            json!({"tagId":"slot","BidFloor":null,"BIDFLOOR":null}),
            json!({"tagId":"slot","BidFloor":null,"b\u{0130}dfloor":1_i32}),
            json!({"tagId":"slot","adunitcode":"a","ADUNITCODE":"b"}),
            json!({"tagId":"slot","AdUnitCode":"a","ADUNITCODE":"a"}),
            json!({"tagId":"slot","AdUnitCode":null,"ADUNITCODE":null}),
            json!({"tagId":"slot","AdUnitCode":"a","adun\u{0130}tcode":null}),
        ] {
            assert_filtered(parameters);
        }
    }

    #[test]
    fn malformed_recognized_string_parameters_do_not_hide_behind_case_variants() {
        for key in [
            "tagid",
            "tagId",
            "TAGID",
            "tag\u{0130}d",
            "adunitcode",
            "ADUNITCODE",
            "adun\u{0130}tcode",
        ] {
            for malformed in [json!(false), json!(1_i32), json!([]), json!({})] {
                let mut parameters = json!({"tagId":"valid"});
                parameters[key] = malformed;
                assert_filtered(parameters);
            }
        }
        // Even the lower-priority exact field must successfully decode.
        assert_filtered(json!({"tagid":"valid","tagId":1_i32}));
    }

    #[test]
    fn malformed_and_missing_parameter_containers_preserve_a_valid_sibling() {
        for parameters in [
            json!(null),
            json!(false),
            json!(1_i32),
            json!("x"),
            json!([]),
            json!({}),
            json!({"tagId":""}),
        ] {
            assert_filtered(parameters);
        }
    }

    #[test]
    fn all_invalid_requests_emit_no_call_and_keep_the_reservation() {
        for impressions in [
            json!([impression("i", json!({}))]),
            json!([{"id":"i","banner":{}}]),
        ] {
            let request = request(impressions);
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &SovrnBidder,
                &request,
                &info("https://bidder.example.test/"),
                SinkLimits::new(4096, 2),
                1,
                &EndpointPolicy(true),
            );
            let errors = semantic.unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 2);
            assert_eq!(errors[0].kind, BidderErrorKind::Other);
            assert!(outcome.accepted.is_empty());
            assert!(outcome.errors.is_empty());
            assert_eq!(remaining, 1);
        }
    }

    #[test]
    fn empty_request_has_no_call_or_semantic_error() {
        let request = request(json!([]));
        let (semantic, calls) = harness::drive(
            &SovrnBidder,
            &request,
            &info("https://bidder.example.test/"),
        );
        semantic.unwrap();
        assert!(calls.is_empty());
    }

    #[test]
    fn floor_only_fills_an_absent_or_zero_original_and_preserves_currency() {
        for original in [
            None,
            Some(0.0_f64),
            Some(-0.0_f64),
            Some(1.25_f64),
            Some(-2.0_f64),
        ] {
            for extension in [json!(4.2_f64), json!("4.2"), json!("4.2e0")] {
                let mut imp = impression("i", json!({"tagid":"slot","bidfloor":extension}));
                imp["bidfloorcur"] = json!("EUR");
                if let Some(value) = original {
                    imp["bidfloor"] = json!(value);
                }
                let request = request(json!([imp]));
                let calls = harness::drive_ok(
                    &SovrnBidder,
                    &request,
                    &info("https://bidder.example.test/"),
                );
                let actual: Value = serde_json::from_slice(calls[0].body()).unwrap();
                assert_eq!(
                    actual["imp"][0]["bidfloor"].as_f64(),
                    Some(
                        original
                            .filter(|value| *value != 0.0_f64)
                            .unwrap_or(4.2_f64)
                    )
                );
                assert_eq!(actual["imp"][0]["bidfloorcur"], json!("EUR"));
                assert_eq!(request.imp[0].bidfloor, original);
            }
        }
    }

    #[test]
    fn interface_floor_shapes_and_unparseable_strings_are_ignored_not_rejected() {
        for key in ["bidfloor", "BidFloor", "BIDFLOOR", "b\u{0130}dfloor"] {
            for floor in [
                json!(null),
                json!(false),
                json!([]),
                json!({}),
                json!("bad"),
                json!(" 4.2 "),
                json!(-2.0_f64),
                json!(0_i32),
                json!("NaN"),
                json!("1e1000"),
                json!("0x1.8p+1"),
                json!("1_000.5"),
            ] {
                let actual = body(json!({"tagid":"slot",key:floor}));
                assert!(actual["imp"][0].get("bidfloor").is_none());
                assert_eq!(actual["imp"][0]["ext"]["bidder"][key], floor);
            }
        }
    }

    #[test]
    fn null_unused_string_field_defaults_do_not_remove_valid_impressions() {
        for key in [
            "adunitcode",
            "AdUnitCode",
            "ADUNITCODE",
            "adun\u{0130}tcode",
        ] {
            let actual = body(json!({"tagid":"slot",key:null}));
            assert_eq!(actual["imp"][0]["tagid"], json!("slot"));
            assert_eq!(actual["imp"][0]["ext"]["bidder"][key], Value::Null);
        }
    }

    #[test]
    fn zero_floor_is_omitted_when_no_positive_extension_replaces_it() {
        for floor in [0.0_f64, -0.0_f64] {
            let mut imp = impression("i", json!({"tagid":"slot","bidfloor":-1_i32}));
            imp["bidfloor"] = json!(floor);
            let request = request(json!([imp]));
            let calls = harness::drive_ok(
                &SovrnBidder,
                &request,
                &info("https://bidder.example.test/"),
            );
            let actual: Value = serde_json::from_slice(calls[0].body()).unwrap();
            assert!(actual["imp"][0].get("bidfloor").is_none());
        }
    }

    #[test]
    fn video_requires_nonnull_slices_and_nonzero_duration_not_nonempty_slices() {
        for video in [
            json!({"mimes":[],"protocols":[],"maxduration":1_i32}),
            json!({"mimes":["video/mp4"],"protocols":[2_i32],"maxduration":-1_i32}),
        ] {
            let mut imp = impression("i", json!({"tagid":"slot"}));
            imp["video"] = video;
            let request = request(json!([imp]));
            let calls = harness::drive_ok(
                &SovrnBidder,
                &request,
                &info("https://bidder.example.test/"),
            );
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].covered_imp_ids().collect::<Vec<_>>(), ["i"]);
        }
    }

    #[test]
    fn each_missing_video_requirement_filters_video_even_with_a_banner() {
        for video in [
            json!({}),
            json!({"protocols":[],"maxduration":1_i32}),
            json!({"mimes":null,"protocols":[],"maxduration":1_i32}),
            json!({"mimes":[],"maxduration":1_i32}),
            json!({"mimes":[],"protocols":null,"maxduration":1_i32}),
            json!({"mimes":[],"protocols":[]}),
            json!({"mimes":[],"protocols":[],"maxduration":0_i32}),
        ] {
            let mut invalid = impression("bad", json!({"tagid":"slot"}));
            invalid["video"] = video;
            let request = request(json!([
                invalid,
                impression("good", json!({"tagid":"slot"}))
            ]));
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &SovrnBidder,
                &request,
                &info("https://bidder.example.test/"),
                SinkLimits::new(4096, 2),
                1,
                &EndpointPolicy(true),
            );
            semantic.unwrap();
            assert_eq!(remaining, 0);
            assert_eq!(outcome.accepted.len(), 1);
            assert_eq!(
                outcome.accepted[0].covered_imp_ids().collect::<Vec<_>>(),
                ["good"]
            );
            assert_eq!(
                outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
                [ConstructionDiagnostic::InvalidImpression]
            );
        }
    }

    #[test]
    fn request_parameters_never_change_endpoint_authority_path_or_query() {
        let endpoint = "https://bidder.example.test/fixed/path?fixed=1&next=%2F";
        let request = request(json!([impression(
            "i",
            json!({"tagid":"//evil.test/?x=1#fragment", "adunitcode":"../escape", "endpoint":"https://evil.test/"})
        )]));
        let calls = harness::drive_ok(&SovrnBidder, &request, &info(endpoint));
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].url(), endpoint);
        assert_eq!(
            serde_json::from_slice::<Value>(calls[0].body()).unwrap()["imp"][0]["tagid"],
            json!("//evil.test/?x=1#fragment")
        );
    }

    #[test]
    fn endpoint_rejection_refunds_the_call_without_semantic_input_failure() {
        let request = request(json!([impression("i", json!({"tagid":"slot"}))]));
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &SovrnBidder,
            &request,
            &info("https://bidder.example.test/"),
            SinkLimits::new(4096, 1),
            1,
            &EndpointPolicy(false),
        );
        semantic.unwrap();
        assert!(outcome.accepted.is_empty());
        assert_eq!(remaining, 1);
        assert!(matches!(
            outcome.errors.as_slice(),
            [OutboundBuildError::EndpointRejected {
                reason: EndpointRejectReason::HostNotAllowed
            }]
        ));
    }

    #[test]
    fn body_limit_refunds_the_reserved_call() {
        let request = request(json!([impression("i", json!({"tagid":"slot"}))]));
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &SovrnBidder,
            &request,
            &info("https://bidder.example.test/"),
            SinkLimits::new(1, 1),
            1,
            &EndpointPolicy(true),
        );
        semantic.unwrap();
        assert!(outcome.accepted.is_empty());
        assert_eq!(remaining, 1);
        assert!(matches!(
            outcome.errors.as_slice(),
            [OutboundBuildError::OversizeRaw { limit: 1 }]
        ));
    }

    #[test]
    fn exhausted_call_budget_refuses_without_building_an_outbound_call() {
        let request = request(json!([impression("i", json!({"tagid":"slot"}))]));
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &SovrnBidder,
            &request,
            &info("https://bidder.example.test/"),
            SinkLimits::new(4096, 1),
            0,
            &EndpointPolicy(true),
        );
        semantic.unwrap();
        assert!(outcome.accepted.is_empty());
        assert_eq!(remaining, 0);
        assert!(matches!(
            outcome.errors.as_slice(),
            [OutboundBuildError::CallBudgetExhausted { max_calls: 0 }]
        ));
    }

    #[test]
    fn oversized_cookie_refunds_the_metadata_reservation_without_an_outbound_call() {
        let mut request = request(json!([impression("i", json!({"tagid":"slot"}))]));
        request.user = Some(
            serde_json::from_value(json!({
                "buyeruid":"x".repeat(crate::META_BIDDER_REGION_BYTES),
            }))
            .unwrap(),
        );
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &SovrnBidder,
            &request,
            &info("https://bidder.example.test/"),
            SinkLimits::new(4096, 1),
            1,
            &EndpointPolicy(true),
        );
        semantic.unwrap();
        assert_eq!(remaining, 1);
        assert!(outcome.accepted.is_empty());
        assert!(matches!(
            outcome.errors.as_slice(),
            [OutboundBuildError::MetaOverCap { .. }]
        ));
    }

    #[test]
    fn positive_infinity_floor_is_request_fatal_only_when_the_original_does_not_win() {
        for value in ["Inf", "+Infinity", "iNf"] {
            let request = request(json!([
                impression("bad", json!({"tagid":"slot", "bidfloor":value})),
                impression("good", json!({"tagid":"slot"})),
            ]));
            let (semantic, calls) = harness::drive(
                &SovrnBidder,
                &request,
                &info("https://bidder.example.test/"),
            );
            let errors = semantic.unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 5);
            assert!(calls.is_empty());
            let mut finite = request;
            finite.imp[0].bidfloor = Some(1.25_f64);
            let finite_calls =
                harness::drive_ok(&SovrnBidder, &finite, &info("https://bidder.example.test/"));
            assert_eq!(finite_calls.len(), 1);
            assert_eq!(
                serde_json::from_slice::<Value>(finite_calls[0].body()).unwrap()["imp"][0]["bidfloor"],
                json!(1.25_f64)
            );
        }
    }

    #[test]
    fn protocol_elements_must_fit_the_source_signed_integer_type() {
        for protocols in [
            json!(["bad"]),
            json!([false]),
            json!([1.0_f64]),
            json!([1.5_f64]),
            json!([[]]),
            json!([{}]),
            json!([128_i32]),
            json!([-129_i32]),
            json!([u64::MAX]),
            json!([1_i32, "bad", 2_i32]),
        ] {
            let request = request(json!([{
                "id":"i","banner":{},"video":{"mimes":["video/mp4"],"maxduration":1_i32,"protocols":protocols},
                "ext":{"bidder":{"tagid":"slot"}},
            }]));
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &SovrnBidder,
                &request,
                &info("https://bidder.example.test/"),
                SinkLimits::new(4096, 1),
                1,
                &EndpointPolicy(true),
            );
            let errors = semantic.unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 2);
            assert!(outcome.accepted.is_empty());
            assert!(outcome.errors.is_empty());
            assert_eq!(remaining, 1);
        }
    }

    #[test]
    fn protocol_wire_omits_empty_slices_and_preserves_signed_values() {
        for (protocols, expected) in [
            (json!([]), None),
            (
                json!([-128_i32, -1_i32, 0_i32, 127_i32]),
                Some(json!([-128_i32, -1_i32, 0_i32, 127_i32])),
            ),
            // Source's int8 codec leaves a null slice element at its zero value.
            (json!([null, 1_i32]), Some(json!([0_i32, 1_i32]))),
        ] {
            let request = request(json!([{
                "id":"i","video":{"mimes":["video/mp4"],"maxduration":1_i32,"protocols":protocols},
                "ext":{"bidder":{"tagid":"slot"}},
            }]));
            let before = serde_json::to_value(&request).unwrap();
            let calls = harness::drive_ok(
                &SovrnBidder,
                &request,
                &info("https://bidder.example.test/"),
            );
            assert_eq!(calls.len(), 1);
            let actual: Value = serde_json::from_slice(calls[0].body()).unwrap();
            assert_eq!(
                actual["imp"][0]["video"].get("protocols"),
                expected.as_ref()
            );
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
    }

    #[test]
    fn outer_bidder_accepts_single_ascii_spellings_and_rejects_malformed_values() {
        for key in ["bidder", "Bidder", "BIDDER"] {
            let valid_request =
                request(json!([{"id":"i","banner":{},"ext":{key:{"tagid":"slot"}}}]));
            let before = serde_json::to_value(&valid_request).unwrap();
            let calls = harness::drive_ok(
                &SovrnBidder,
                &valid_request,
                &info("https://bidder.example.test/"),
            );
            assert_eq!(calls.len(), 1);
            let actual: Value = serde_json::from_slice(calls[0].body()).unwrap();
            assert_eq!(actual["imp"][0]["tagid"], json!("slot"));
            assert_eq!(actual["imp"][0]["ext"], before["imp"][0]["ext"]);
            assert_eq!(serde_json::to_value(&valid_request).unwrap(), before);
            for malformed in [
                json!(false),
                json!(1_i32),
                json!([]),
                json!("bad"),
                Value::Null,
            ] {
                let request = request(json!([{"id":"i","banner":{},"ext":{key:malformed}}]));
                let (semantic, rejected_calls) = harness::drive(
                    &SovrnBidder,
                    &request,
                    &info("https://bidder.example.test/"),
                );
                assert_eq!(semantic.unwrap_err()[0].code, 2);
                assert!(rejected_calls.is_empty());
            }
        }
    }

    #[test]
    fn outer_bidder_ambiguity_rejects_only_its_impression() {
        for (left, right) in [("bidder", "Bidder"), ("Bidder", "BIDDER")] {
            for (first, second) in [
                (json!({"tagid":"same"}), json!({"tagid":"different"})),
                (json!({"tagid":"same"}), json!({"tagid":"same"})),
                (json!({"tagid":"same"}), Value::Null),
                (Value::Null, Value::Null),
            ] {
                let request = request(json!([
                    {"id":"ambiguous","banner":{},"ext":{left:first,right:second}},
                    impression("good", json!({"tagid":"survivor"})),
                ]));
                let (semantic, outcome, remaining) = harness::drive_with_limits(
                    &SovrnBidder,
                    &request,
                    &info("https://bidder.example.test/"),
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
                    ["good"]
                );
                assert_eq!(
                    outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
                    [ConstructionDiagnostic::InvalidImpressionParameters]
                );
            }
        }
    }
}
