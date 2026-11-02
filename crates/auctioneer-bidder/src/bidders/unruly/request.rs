use crate::bidders::url::{json_utf8_headers, write_endpoint};
use crate::{BidderError, BidderRequestInfo, OutboundRequestSink};
use auctioneer_openrtb::{
    ext::imp::ExtImpPrebid,
    types::{
        bid_request::{BidRequest, Imp},
        field::JsonField,
    },
};
use serde::{
    Serialize, Serializer,
    ser::{Error as SerializeError, SerializeMap as _, SerializeSeq as _},
};
use serde_json::{Map, Value};

pub(super) fn make_requests(
    request: &BidRequest,
    info: &BidderRequestInfo<'_>,
    sink: &mut OutboundRequestSink<'_>,
) -> Result<(), Vec<BidderError>> {
    // One invalid impression aborts the whole source request. Check every
    // impression before reserving a call or retaining any output bytes.
    for imp in &request.imp {
        if prepare(imp).is_err() {
            return Err(vec![BidderError::bad_input(
                "invalid unruly impression extension",
            )]);
        }
    }
    if info.endpoint_template.is_empty() {
        return Ok(());
    }
    let _recorded = sink.write_json(
        info,
        |meta| {
            meta.set_method("POST")?;
            write_endpoint(meta, info)?;
            json_utf8_headers(meta)?;
            for imp in &request.imp {
                meta.imp_id(&imp.id)?;
            }
            Ok(())
        },
        &Request(request),
    );
    Ok(())
}

#[derive(Default, Serialize)]
struct Parameters {
    #[serde(rename = "siteid")]
    old: i64,
    #[serde(rename = "siteId")]
    preferred: i64,
}

fn general_key(key: &str, lowercase: &str) -> bool {
    // Only dotted capital I can lowercase to a nontrivial ASCII character
    // occurring in these source field names. This is not Unicode case folding.
    key.chars()
        .map(|character| match character {
            '\u{0130}' => 'i',
            _ => character.to_ascii_lowercase(),
        })
        .eq(lowercase.chars())
}

fn unique_field<'a>(
    fields: &'a Map<String, Value>,
    name: &str,
    general: bool,
) -> Result<Option<&'a Value>, ()> {
    let mut matching = fields.iter().filter(|(key, _value)| {
        if general {
            general_key(key, name)
        } else {
            key.eq_ignore_ascii_case(name)
        }
    });
    let value = matching.next().map(|(_key, value)| value);
    if matching.next().is_some() {
        return Err(());
    }
    Ok(value)
}

fn integer(value: &Value) -> Result<i64, ()> {
    if value.is_null() {
        Ok(0)
    } else {
        value.as_i64().ok_or(())
    }
}

fn signed_byte(value: &Value) -> Result<(), ()> {
    i8::try_from(integer(value)?)
        .map(|_number| ())
        .map_err(|_error| ())
}

fn string(value: &Value) -> Result<(), ()> {
    if value.is_null() || value.is_string() {
        Ok(())
    } else {
        Err(())
    }
}

fn boolean(value: &Value) -> Result<(), ()> {
    if value.is_null() || value.is_boolean() {
        Ok(())
    } else {
        Err(())
    }
}

fn object(value: &Value) -> Result<Option<&Map<String, Value>>, ()> {
    if value.is_null() {
        Ok(None)
    } else {
        value.as_object().map(Some).ok_or(())
    }
}

fn stored_request(value: &Value) -> Result<(), ()> {
    if let Some(fields) = object(value)?
        && let Some(id) = unique_field(fields, "id", false)?
    {
        string(id)?;
    }
    Ok(())
}

fn stored_bids(raw_bids: &Value) -> Result<(), ()> {
    if raw_bids.is_null() {
        return Ok(());
    }
    for entry in raw_bids.as_array().ok_or(())? {
        if let Some(fields) = object(entry)? {
            for name in ["id", "bidder"] {
                if let Some(value) = unique_field(fields, name, false)? {
                    string(value)?;
                }
            }
            if let Some(value) = unique_field(fields, "replaceimpid", false)? {
                boolean(value)?;
            }
        }
    }
    Ok(())
}

fn floors(raw_floors: &Value) -> Result<(), ()> {
    let Some(fields) = object(raw_floors)? else {
        return Ok(());
    };
    // Source's floorminCur alias collides with its folded hash, selecting
    // jsoniter's general decoder for the entire floors object.
    for name in ["floorrule", "floormincur"] {
        if let Some(value) = unique_field(fields, name, true)? {
            string(value)?;
        }
    }
    for name in ["floorrulevalue", "floorvalue", "floormin"] {
        if let Some(value) = unique_field(fields, name, true)?
            && !value.is_null()
            && !value.as_f64().is_some_and(f64::is_finite)
        {
            return Err(());
        }
    }
    Ok(())
}

fn prebid_fields(fields: &Map<String, Value>) -> Result<(), ()> {
    // This is the source's ten-field optimized ASCII decoder, not a bidder
    // parameter-schema validator. Every value is borrowed and discarded.
    for name in [
        "storedrequest",
        "storedauctionresponse",
        "storedbidresponse",
        "is_rewarded_inventory",
        "bidder",
        "options",
        "adunitcode",
        "passthrough",
        "floors",
        "imp",
    ] {
        let Some(value) = unique_field(fields, name, false)? else {
            continue;
        };
        match name {
            "storedrequest" | "storedauctionresponse" => stored_request(value)?,
            "storedbidresponse" => stored_bids(value)?,
            "is_rewarded_inventory" => signed_byte(value)?,
            "bidder" | "imp" => {
                let _fields = object(value)?;
            }
            "options" => {
                if let Some(options) = object(value)?
                    && let Some(echo) = unique_field(options, "echovideoattrs", false)?
                {
                    boolean(echo)?;
                }
            }
            "adunitcode" => string(value)?,
            "floors" => floors(value)?,
            _ => {}
        }
    }
    Ok(())
}

fn typed_prebid(prebid: &ExtImpPrebid) -> Result<(), ()> {
    prebid_fields(&prebid.other)?;
    if let Some(stored) = &prebid.storedrequest {
        if unique_field(&prebid.other, "storedrequest", false)?.is_some() {
            return Err(());
        }
        stored_request(stored)?;
    }
    for (name, populated) in [
        ("bidder", !prebid.bidder.is_empty()),
        ("imp", !prebid.imp.is_empty()),
    ] {
        if populated && unique_field(&prebid.other, name, false)?.is_some() {
            return Err(());
        }
    }
    // Shared typed ingress loses empty-map presence and storedrequest:null;
    // ambiguity involving those discarded canonical names remains a shared gap.
    Ok(())
}

fn prepare(imp: &Imp) -> Result<Parameters, ()> {
    let ext = imp.ext.as_ref().ok_or(())?;
    if let Some(ae) = unique_field(&ext.other, "ae", false)? {
        signed_byte(ae)?;
    }
    let prebid_alias = unique_field(&ext.other, "prebid", false)?;
    if !ext.prebid.is_missing() && prebid_alias.is_some() {
        return Err(());
    }
    if let Some(prebid) = ext.prebid.as_ref() {
        typed_prebid(prebid)?;
    }
    if let Some(alias) = prebid_alias
        && let Some(fields) = object(alias)?
    {
        prebid_fields(fields)?;
    }
    let alias = unique_field(&ext.other, "bidder", false)?;
    let params = match (&ext.bidder, alias) {
        (JsonField::Missing, Some(value)) | (JsonField::Value(value), None) => Some(value),
        (JsonField::Null, None) => None,
        _ => return Err(()),
    };
    let Some(value) = params else {
        return Ok(Parameters::default());
    };
    let Some(fields) = object(value)? else {
        return Ok(Parameters::default());
    };
    let mut old_fields = fields
        .iter()
        .filter(|(key, _value)| key.as_str() != "siteId" && general_key(key, "siteid"));
    let old = old_fields
        .next()
        .map(|(_key, old_value)| integer(old_value))
        .transpose()?
        .unwrap_or(0);
    if old_fields.next().is_some() {
        return Err(());
    }
    let preferred = fields.get("siteId").map(integer).transpose()?.unwrap_or(0);
    Ok(Parameters { old, preferred })
}

struct Request<'a>(&'a BidRequest);

impl Serialize for Request<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Exhaustiveness makes additions to the shared model require review.
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
        if !imp.is_empty() {
            map.serialize_entry("imp", &Impressions(imp))?;
        }
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

struct Impressions<'a>(&'a [Imp]);

impl Serialize for Impressions<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for imp in self.0 {
            let params = prepare(imp).map_err(|()| {
                <S::Error as SerializeError>::custom("invalid unruly impression extension")
            })?;
            sequence.serialize_element(&Impression { imp, params })?;
        }
        sequence.end()
    }
}

struct Impression<'a> {
    imp: &'a Imp,
    params: Parameters,
}

#[derive(Serialize)]
struct Extension<'a> {
    bidder: &'a Parameters,
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
            ext: _discarded_extension,
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
        if let Some(value) = bidfloor {
            map.serialize_entry("bidfloor", value)?;
        }
        if let Some(value) = bidfloorcur {
            map.serialize_entry("bidfloorcur", value)?;
        }
        if let Some(value) = secure {
            map.serialize_entry("secure", value)?;
        }
        map.serialize_entry(
            "ext",
            &Extension {
                bidder: &self.params,
            },
        )?;
        for (key, value) in other {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use super::super::UnrulyBidder;
    use super::super::tests::{info, parameters, request};
    use crate::bidders::harness;
    use crate::{
        BidderError, BidderRequestInfo, EndpointRejectReason, EndpointValidator,
        OutboundBuildError, SinkLimits,
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

    fn assert_parameters(params: Value, old: i64, preferred: i64) {
        let request = parameters(params);
        let before = serde_json::to_value(&request).unwrap();
        let calls = harness::drive_ok(&UnrulyBidder, &request, &info());
        assert_eq!(calls.len(), 1);
        let body: Value = serde_json::from_slice(calls[0].body()).unwrap();
        assert_eq!(
            body["imp"][0]["ext"],
            json!({"bidder":{"siteid":old,"siteId":preferred}})
        );
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    fn assert_prebid_presence_ambiguity(alias_value: &Value) {
        for alias in ["Prebid", "PREBID", "pReBiD"] {
            for reversed in [false, true] {
                let mut impressions = json!([
                    {"id":"good","banner":{},"ext":{"bidder":{"siteId":7_i32}}},
                    {"id":"bad","banner":{},"ext":{
                        "bidder":{"siteId":34_i32},"prebid":null,alias:alias_value
                    }}
                ]);
                if reversed {
                    impressions.as_array_mut().unwrap().reverse();
                }
                let request = request(impressions);
                let before = serde_json::to_value(&request).unwrap();
                let (semantic, outcome, remaining) = harness::drive_with_limits(
                    &UnrulyBidder,
                    &request,
                    &info(),
                    SinkLimits::new(4096, 2),
                    1,
                    &EndpointPolicy(true),
                );
                assert_eq!(
                    semantic,
                    Err(vec![BidderError::bad_input(
                        "invalid unruly impression extension"
                    )]),
                    "canonical null conflicts with {alias}, reversed={reversed}"
                );
                assert!(outcome.accepted.is_empty());
                assert!(outcome.errors.is_empty());
                assert_eq!(remaining, 1);
                assert_eq!(serde_json::to_value(&request).unwrap(), before);
            }
        }
    }

    fn assert_discarded_prebid(prebid: Value, accepted: bool) {
        let mut impressions = json!([
            {"id":"good","banner":{},"ext":{"bidder":{"siteId":7_i32}}},
            {"id":"nested","video":{},"ext":{"bidder":{"siteId":34_i32}}}
        ]);
        impressions[1]["ext"]["prebid"] = prebid;
        let request = request(impressions);
        let before = serde_json::to_value(&request).unwrap();
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &UnrulyBidder,
            &request,
            &info(),
            SinkLimits::new(256 * 1024, 2),
            1,
            &EndpointPolicy(true),
        );
        assert!(outcome.errors.is_empty());
        assert!(outcome.diagnostics.is_empty());
        if accepted {
            semantic.unwrap();
            assert_eq!(remaining, 0);
            assert_eq!(outcome.accepted.len(), 1);
            let call = &outcome.accepted[0];
            assert_eq!(
                call.covered_imp_ids().collect::<Vec<_>>(),
                ["good", "nested"]
            );
            assert_eq!(
                serde_json::from_slice::<Value>(call.body()).unwrap(),
                json!({
                    "id":"r","imp":[
                        {"id":"good","banner":{},"ext":{"bidder":{"siteid":0_i32,"siteId":7_i32}}},
                        {"id":"nested","video":{},"ext":{"bidder":{"siteid":0_i32,"siteId":34_i32}}}
                    ]
                })
            );
        } else {
            assert_eq!(remaining, 1);
            assert!(outcome.accepted.is_empty());
            assert_eq!(
                semantic.unwrap_err(),
                [BidderError::bad_input(
                    "invalid unruly impression extension"
                )]
            );
        }
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn typed_extension_rewrite_preserves_every_other_request_and_impression_subtree() {
        let request: BidRequest = serde_json::from_value(json!({
            "id":"r", "site":{"id":"site"}, "app":{"id":"app"}, "dooh":{"id":"dooh"},
            "device":{"make":"device"}, "user":{"id":"already-scoped"},
            "regs":{"coppa":1_i32}, "source":{"tid":"already-scoped"}, "at":2_i32,
            "tmax":150_i32, "cur":["EUR"], "test":1_i32, "ext":{"keep":{"nested":true}},
            "opaque":[false,null],
            "imp":[
                {"id":"first", "banner":{"w":300_i32,"h":250_i32,"opaque":{"x":1_i32}},
                 "video":{"mimes":["video/mp4"]}, "audio":{}, "native":{},
                 "bidfloor":1.25_f64, "bidfloorcur":"EUR", "secure":1_i32,
                 "tagid":"unchanged", "opaque":{"keep":[true,null]},
                 "ext":{"bidder":{"siteid":12_i32,"siteId":34_i32,"discard":{"nested":true}},
                        "prebid":{"adunitcode":"discard"}, "tid":"discard", "ae":1_i32,
                        "discard":{"nested":true}}},
                {"id":"second", "video":{}, "ext":{"bidder":{"siteId":56_i32}}}
            ]
        }))
        .unwrap();
        let before = serde_json::to_value(&request).unwrap();
        assert_eq!(before.as_object().unwrap().len(), 15);
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &UnrulyBidder,
            &request,
            &info(),
            SinkLimits::new(256 * 1024, 2),
            1,
            &EndpointPolicy(true),
        );
        semantic.unwrap();
        assert_eq!(remaining, 0);
        assert!(outcome.errors.is_empty());
        assert!(outcome.diagnostics.is_empty());
        assert_eq!(outcome.accepted.len(), 1);
        let call = &outcome.accepted[0];
        assert_eq!(call.method(), "POST");
        assert_eq!(call.url(), info().endpoint_template);
        assert_eq!(
            call.headers().collect::<Vec<_>>(),
            [
                ("Content-Type", "application/json;charset=utf-8"),
                ("Accept", "application/json"),
            ]
        );
        assert_eq!(
            call.covered_imp_ids().collect::<Vec<_>>(),
            ["first", "second"]
        );
        let mut expected = before.clone();
        expected["imp"][0]["ext"] = json!({"bidder":{"siteid":12_i32,"siteId":34_i32}});
        expected["imp"][1]["ext"] = json!({"bidder":{"siteid":0_i32,"siteId":56_i32}});
        assert_eq!(
            serde_json::from_slice::<Value>(call.body()).unwrap(),
            expected
        );
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn distinct_exact_siteid_and_site_id_fields_coexist_without_precedence_loss() {
        for (params, old, preferred) in [
            (json!({"siteid":12_i32}), 12, 0),
            (json!({"siteId":34_i32}), 0, 34),
            (json!({"siteid":12_i32,"siteId":34_i32}), 12, 34),
            (json!({"siteid":0_i32,"siteId":34_i32}), 0, 34),
            (json!({"siteid":12_i32,"siteId":0_i32}), 12, 0),
            (json!({"siteid":null,"siteId":34_i32}), 0, 34),
            (json!({"siteid":12_i32,"siteId":null}), 12, 0),
            (json!({"siteid":-12_i32,"siteId":-34_i32}), -12, -34),
            (
                json!({"siteid":i64::MIN,"siteId":i64::MAX}),
                i64::MIN,
                i64::MAX,
            ),
        ] {
            assert_parameters(params, old, preferred);
        }
    }

    #[test]
    fn general_decoder_fallback_binds_only_to_old_siteid_including_dotted_i() {
        for key in [
            "SiteId",
            "SITEID",
            "sItEiD",
            "s\u{0130}teid",
            "site\u{0130}d",
        ] {
            assert_parameters(json!({key:12_i32}), 12, 0);
            assert_parameters(json!({key:12_i32,"siteId":34_i32}), 12, 34);
        }
        // Go strings.ToLower is not Unicode case folding (long s stays long s).
        assert_parameters(json!({"\u{017f}iteid":"ignored", "siteId":34_i32}), 0, 34);
    }

    #[test]
    fn competing_spellings_of_old_siteid_are_bidder_fatal_even_when_equal_or_null() {
        for (first, second) in [
            ("siteid", "SITEID"),
            ("SiteId", "SITEID"),
            ("SITEID", "s\u{0130}teid"),
        ] {
            for (left, right) in [
                (json!(12_i32), json!(34_i32)),
                (json!(12_i32), json!(12_i32)),
                (Value::Null, json!(12_i32)),
                (Value::Null, Value::Null),
            ] {
                let request = parameters(json!({first:left,second:right,"siteId":56_i32}));
                let (semantic, calls) = harness::drive(&UnrulyBidder, &request, &info());
                assert!(calls.is_empty());
                let errors = semantic.unwrap_err();
                assert_eq!(errors.len(), 1);
                assert_eq!(errors[0].code, 2);
            }
        }
    }

    #[test]
    fn malformed_values_for_each_recognized_parameter_abort_all_impressions() {
        for key in ["siteid", "siteId", "SITEID", "s\u{0130}teid"] {
            for value in [
                json!("private-marker"),
                json!(true),
                json!([]),
                json!({}),
                json!(1.5_f64),
                json!(u64::MAX),
            ] {
                for bad_first in [true, false] {
                    let good = json!({"id":"good","banner":{},"ext":{"bidder":{"siteId":7_i32}}});
                    let bad =
                        json!({"id":"private-marker","banner":{},"ext":{"bidder":{key:value}}});
                    let request = request(if bad_first {
                        json!([bad, good])
                    } else {
                        json!([good, bad])
                    });
                    let before = serde_json::to_value(&request).unwrap();
                    let (semantic, outcome, remaining) = harness::drive_with_limits(
                        &UnrulyBidder,
                        &request,
                        &info(),
                        SinkLimits::new(256 * 1024, 2),
                        1,
                        &EndpointPolicy(true),
                    );
                    assert!(outcome.accepted.is_empty());
                    assert!(outcome.errors.is_empty());
                    assert!(outcome.diagnostics.is_empty());
                    assert_eq!(remaining, 1);
                    let errors = semantic.unwrap_err();
                    assert_eq!(errors.len(), 1);
                    assert_eq!(errors[0].code, 2);
                    assert!(!errors[0].message.contains("private-marker"));
                    assert_eq!(serde_json::to_value(&request).unwrap(), before);
                }
            }
        }
    }

    #[test]
    fn empty_and_null_struct_parameters_write_both_zero_fields() {
        assert_parameters(json!({}), 0, 0);
        assert_parameters(
            json!({"siteid":null,"siteId":null,"unknown":"ignored"}),
            0,
            0,
        );
        // The coordinator-owned JsonField migration preserves canonical null
        // separately from missing, as required by source struct decoding.
        assert_parameters(Value::Null, 0, 0);
        for key in ["Bidder", "BIDDER"] {
            let request = request(json!([{"id":"i","banner":{},"ext":{key:null}}]));
            let calls = harness::drive_ok(&UnrulyBidder, &request, &info());
            assert_eq!(calls.len(), 1);
            assert_eq!(
                serde_json::from_slice::<Value>(calls[0].body()).unwrap()["imp"][0]["ext"],
                json!({"bidder":{"siteid":0_i32,"siteId":0_i32}})
            );
        }
    }

    #[test]
    fn missing_extensions_missing_bidder_and_nonobject_parameters_are_bidder_fatal() {
        for imp in [
            json!({"id":"i","banner":{}}),
            json!({"id":"i","banner":{},"ext":{}}),
            json!({"id":"i","banner":{},"ext":{"bidder":false}}),
            json!({"id":"i","banner":{},"ext":{"bidder":[]}}),
            json!({"id":"i","banner":{},"ext":{"bidder":"private-marker"}}),
        ] {
            let request = request(json!([imp]));
            let (semantic, calls) = harness::drive(&UnrulyBidder, &request, &info());
            assert!(calls.is_empty());
            let errors = semantic.unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 2);
            assert!(!errors[0].message.contains("private-marker"));
        }
    }

    #[test]
    fn outer_bidder_prebid_and_ae_keys_use_the_source_ascii_decoder() {
        for ext in [
            json!({"BIDDER":{"siteId":34_i32}}),
            json!({"Bidder":{"siteId":34_i32},"PREBID":{"adunitcode":"ignored"}}),
            json!({"bidder":{"siteId":34_i32},"AE":127_i32}),
            json!({"bidder":{"siteId":34_i32},"ae":-128_i32}),
            json!({"bidder":{"siteId":34_i32},"AE":null}),
            json!({"bidder":{"siteId":34_i32},"b\u{0130}dder":false,
                           "preb\u{0130}d":false}),
        ] {
            let request = request(json!([{"id":"i","banner":{},"ext":ext}]));
            let calls = harness::drive_ok(&UnrulyBidder, &request, &info());
            assert_eq!(calls.len(), 1);
            assert_eq!(
                serde_json::from_slice::<Value>(calls[0].body()).unwrap()["imp"][0]["ext"],
                json!({"bidder":{"siteid":0_i32,"siteId":34_i32}})
            );
        }
    }

    #[test]
    fn outer_recognized_key_ambiguity_is_fatal_without_canonical_spelling() {
        for (first, second, left, right) in [
            (
                "bidder",
                "BIDDER",
                json!({"siteId":34_i32}),
                json!({"siteId":34_i32}),
            ),
            (
                "Bidder",
                "BIDDER",
                json!({"siteId":34_i32}),
                json!({"siteId":56_i32}),
            ),
            ("prebid", "PREBID", json!({}), json!({})),
            ("Prebid", "PREBID", json!({}), json!({})),
            ("ae", "AE", json!(1_i32), json!(1_i32)),
            ("Ae", "AE", json!(1_i32), json!(2_i32)),
            ("bidder", "BIDDER", Value::Null, Value::Null),
            ("Bidder", "BIDDER", Value::Null, Value::Null),
            ("Prebid", "PREBID", Value::Null, Value::Null),
            ("ae", "AE", Value::Null, Value::Null),
            ("Ae", "AE", Value::Null, Value::Null),
        ] {
            let mut ext = json!({first:left,second:right});
            if first != "bidder" && first != "Bidder" {
                ext["bidder"] = json!({"siteId":34_i32});
            }
            let request = request(json!([{"id":"i","banner":{},"ext":ext}]));
            let (semantic, calls) = harness::drive(&UnrulyBidder, &request, &info());
            assert!(calls.is_empty());
            let errors = semantic.unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 2);
        }
    }

    #[test]
    fn prebid_presence_absence_null_and_single_ascii_spellings_are_valid() {
        for ext in [
            json!({"bidder":{"siteId":34_i32}}),
            json!({"bidder":{"siteId":34_i32},"prebid":null}),
            json!({"bidder":{"siteId":34_i32},"prebid":{}}),
            json!({"bidder":{"siteId":34_i32},"Prebid":null}),
            json!({"bidder":{"siteId":34_i32},"PREBID":null}),
            json!({"bidder":{"siteId":34_i32},"pReBiD":null}),
            json!({"bidder":{"siteId":34_i32},"Prebid":{}}),
            json!({"bidder":{"siteId":34_i32},"PREBID":{"adunitcode":"ignored"}}),
        ] {
            let request = request(json!([{"id":"i","banner":{},"ext":ext}]));
            let before = serde_json::to_value(&request).unwrap();
            let calls = harness::drive_ok(&UnrulyBidder, &request, &info());
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].covered_imp_ids().collect::<Vec<_>>(), ["i"]);
            assert_eq!(
                serde_json::from_slice::<Value>(calls[0].body()).unwrap()["imp"][0]["ext"],
                json!({"bidder":{"siteid":0_i32,"siteId":34_i32}})
            );
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
    }

    #[test]
    fn prebid_presence_canonical_null_and_null_ascii_alias_are_bidder_fatal() {
        assert_prebid_presence_ambiguity(&json!(null));
    }

    #[test]
    fn prebid_presence_canonical_null_and_object_ascii_alias_are_bidder_fatal() {
        assert_prebid_presence_ambiguity(&json!({}));
        assert_prebid_presence_ambiguity(&json!({"adunitcode":"discarded"}));
    }

    #[test]
    fn malformed_outer_recognized_values_are_not_ignored_when_rewritten_away() {
        for (key, value) in [
            ("BIDDER", json!(false)),
            ("BIDDER", json!([])),
            ("PREBID", json!(false)),
            ("PREBID", json!([])),
            ("PREBID", json!({"adunitcode":false})),
            ("ae", json!(128_i32)),
            ("AE", json!(-129_i32)),
            ("AE", json!(1.5_f64)),
            ("AE", json!("1")),
            ("AE", json!({})),
        ] {
            let mut ext = json!({key:value});
            if key != "BIDDER" {
                ext["bidder"] = json!({"siteId":34_i32});
            }
            let request = request(json!([{"id":"i","banner":{},"ext":ext}]));
            let (semantic, calls) = harness::drive(&UnrulyBidder, &request, &info());
            assert!(calls.is_empty());
            let errors = semantic.unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 2);
        }
    }

    #[test]
    fn request_construction_does_not_filter_media_before_the_response() {
        let request = request(json!([
            {"id":"native","native":{},"ext":{"bidder":{"siteId":7_i32}}},
            {"id":"audio","audio":{},"ext":{"bidder":{"siteId":7_i32}}},
            {"id":"none","ext":{"bidder":{"siteId":7_i32}}}
        ]));
        let calls = harness::drive_ok(&UnrulyBidder, &request, &info());
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].covered_imp_ids().collect::<Vec<_>>(),
            ["native", "audio", "none"]
        );
    }

    #[test]
    fn body_id_call_and_endpoint_caps_bind_independently_and_refund_reservations() {
        let request = parameters(json!({"siteId":7_i32}));
        let calls = harness::drive_ok(&UnrulyBidder, &request, &info());
        assert_eq!(calls.len(), 1);
        let bytes = calls[0].body().len();
        for (raw, ids, slots, allow, error, remaining) in [
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
                &UnrulyBidder,
                &request,
                &info(),
                SinkLimits::new(raw, ids),
                slots,
                &EndpointPolicy(allow),
            );
            semantic.unwrap();
            assert_eq!(outcome.accepted.len(), usize::from(error.is_none()));
            assert_eq!(outcome.errors, error.into_iter().collect::<Vec<_>>());
            assert_eq!(actual_remaining, remaining);
        }
    }

    #[test]
    fn discarded_replaceimpid_keeps_source_boolean_null_and_ascii_matching_rules() {
        for key in ["replaceimpid", "REPLACEIMPID", "ReplaceImpId"] {
            for value in [json!(true), json!(false), Value::Null] {
                assert_discarded_prebid(json!({"storedbidresponse":[{key:value}]}), true);
            }
            for value in [json!(1_i32), json!("false"), json!([]), json!({})] {
                assert_discarded_prebid(json!({"storedbidresponse":[{key:value}]}), false);
            }
        }
        for value in [json!(true), Value::Null] {
            assert_discarded_prebid(
                json!({"storedbidresponse":[{"ReplaceImpId":value,"REPLACEIMPID":value}]}),
                false,
            );
        }
        assert_discarded_prebid(
            json!({"storedbidresponse":[{"replace\u{0130}mpid":"ignored"}]}),
            true,
        );
    }

    #[test]
    fn discarded_echovideoattrs_keeps_source_boolean_null_and_ascii_matching_rules() {
        for key in ["echovideoattrs", "ECHOVIDEOATTRS", "EchoVideoAttrs"] {
            for value in [json!(true), json!(false), Value::Null] {
                assert_discarded_prebid(json!({"options":{key:value}}), true);
            }
            for value in [json!(1_i32), json!("false"), json!([]), json!({})] {
                assert_discarded_prebid(json!({"options":{key:value}}), false);
            }
        }
        for value in [json!(true), Value::Null] {
            assert_discarded_prebid(
                json!({"options":{"EchoVideoAttrs":value,"ECHOVIDEOATTRS":value}}),
                false,
            );
        }
        assert_discarded_prebid(json!({"options":{"echov\u{0130}deoattrs":"ignored"}}), true);
    }

    #[test]
    fn discarded_rewarded_inventory_checks_the_source_signed_byte_not_a_boolean_schema() {
        for key in [
            "is_rewarded_inventory",
            "IS_REWARDED_INVENTORY",
            "Is_Rewarded_Inventory",
        ] {
            for value in [
                json!(-128_i32),
                json!(-1_i32),
                json!(0_i32),
                json!(1_i32),
                json!(127_i32),
                Value::Null,
            ] {
                assert_discarded_prebid(json!({key:value}), true);
            }
            for value in [
                json!(-129_i32),
                json!(128_i32),
                json!(u64::MAX),
                json!(1.0_f64),
                json!(true),
                json!("1"),
                json!([]),
                json!({}),
            ] {
                assert_discarded_prebid(json!({key:value}), false);
            }
        }
        for value in [json!(1_i32), Value::Null] {
            assert_discarded_prebid(
                json!({"IS_REWARDED_INVENTORY":value,"Is_Rewarded_Inventory":value}),
                false,
            );
        }
    }

    #[test]
    fn discarded_floormin_cur_uses_general_lowercase_and_string_null_validation() {
        for key in [
            "floorminCur",
            "FLOORMINCUR",
            "FloorMinCur",
            "floorm\u{0130}nCur",
        ] {
            for value in [json!("USD"), Value::Null] {
                assert_discarded_prebid(json!({"floors":{key:value}}), true);
            }
            for value in [json!(1_i32), json!(true), json!([]), json!({})] {
                assert_discarded_prebid(json!({"floors":{key:value}}), false);
            }
        }
        for (first, second) in [
            ("floorminCur", "FLOORMINCUR"),
            ("FloorMinCur", "FLOORMINCUR"),
            ("floorm\u{0130}nCur", "FLOORMINCUR"),
        ] {
            for value in [json!("USD"), Value::Null] {
                assert_discarded_prebid(json!({"floors":{first:value,second:value}}), false);
            }
        }
    }
}
