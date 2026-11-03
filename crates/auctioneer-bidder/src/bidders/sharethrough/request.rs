use crate::bidders::url::{json_utf8_headers, write_endpoint};
use crate::correlation::CorrelationMedia;
use crate::{BidderError, BidderRequestInfo, ConstructionDiagnostic, OutboundRequestSink};
use auctioneer_openrtb::{
    ext::misc::SourceExt,
    types::{
        bid_request::{BidRequest, Imp},
        field::JsonField,
        source::Source,
    },
};
use serde::{
    Serialize, Serializer,
    ser::{Error as _, SerializeMap as _, SerializeSeq as _},
};
use serde_json::{Map, Value};

pub(super) fn make_requests(
    request: &BidRequest,
    info: &BidderRequestInfo<'_>,
    sink: &mut OutboundRequestSink<'_>,
) -> Result<(), Vec<BidderError>> {
    let categories = string_array(request.other.get("bcat"))
        .map_err(|_reason| vec![BidderError::bad_input("invalid sharethrough blocklists")])?;
    let advertisers = string_array(request.other.get("badv"))
        .map_err(|_reason| vec![BidderError::bad_input("invalid sharethrough blocklists")])?;
    let mut valid = false;
    for (index, imp) in request.imp.iter().enumerate() {
        let params = match parameters(imp) {
            Ok(params) => params,
            Err(reason) => {
                sink.diagnostic(reason);
                continue;
            }
        };
        let prefix = request.imp.get(..=index).ok_or_else(|| {
            vec![BidderError::failed_to_request_bids(
                "invalid sharethrough impression prefix",
            )]
        })?;
        let mut floor = imp.bidfloor;
        let mut currency = imp.bidfloorcur.as_deref();
        if let (Some(amount), Some(code)) = (floor, currency)
            && amount > 0.0_f64
            && !code.is_empty()
            && !code.eq_ignore_ascii_case("USD")
        {
            floor = Some(
                info.floor_currency
                    .convert_floor_to_usd(amount, code)
                    .map_err(|_reason| {
                        vec![BidderError::unknown(
                            "sharethrough floor currency conversion failed",
                        )]
                    })?,
            );
            currency = Some("USD");
        }
        if imp.banner.is_none() && imp.video.is_none() && imp.native.is_none() {
            sink.diagnostic(ConstructionDiagnostic::UnsupportedMediaType);
            continue;
        }
        valid = true;
        for (media, offered) in [
            (CorrelationMedia::Banner, imp.banner.is_some()),
            (CorrelationMedia::Video, imp.video.is_some()),
            (CorrelationMedia::Native, imp.native.is_some()),
        ] {
            if !offered {
                continue;
            }
            let _recorded = sink.write_json(
                info,
                |meta| {
                    meta.set_method("POST")?;
                    write_endpoint(meta, info)?;
                    json_utf8_headers(meta)?;
                    meta.imp_id(&imp.id)?;
                    meta.correlate_imp(0, media, 0)?;
                    Ok(())
                },
                &Request {
                    scoped: request,
                    impression: Impression {
                        imp,
                        pkey: params.pkey,
                        floor,
                        currency,
                        media,
                    },
                    categories: Blocklist {
                        root: categories,
                        prefix,
                        kind: BlocklistKind::Category,
                    },
                    advertisers: Blocklist {
                        root: advertisers,
                        prefix,
                        kind: BlocklistKind::Advertiser,
                    },
                },
            );
        }
    }
    if valid || request.imp.is_empty() {
        Ok(())
    } else {
        Err(vec![BidderError::bad_input(
            "no valid sharethrough impressions",
        )])
    }
}

#[derive(Clone, Copy)]
struct Parameters<'a> {
    pkey: &'a str,
    categories: &'a [Value],
    advertisers: &'a [Value],
}

fn unique_field<'a>(
    fields: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a Value>, ConstructionDiagnostic> {
    let mut matching = fields
        .iter()
        .filter(|(key, _value)| key.eq_ignore_ascii_case(name));
    let value = matching.next().map(|(_key, field)| field);
    if matching.next().is_some() {
        return Err(ConstructionDiagnostic::InvalidImpressionParameters);
    }
    Ok(value)
}

fn string_array(value: Option<&Value>) -> Result<&[Value], ConstructionDiagnostic> {
    match value {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(values)) if values.iter().all(Value::is_string) => Ok(values),
        _ => Err(ConstructionDiagnostic::InvalidImpressionParameters),
    }
}

fn parameters(imp: &Imp) -> Result<Parameters<'_>, ConstructionDiagnostic> {
    let invalid = ConstructionDiagnostic::InvalidImpressionParameters;
    let ext = imp.ext.as_ref().ok_or(invalid)?;
    let alias = unique_field(&ext.other, "bidder")?;
    let declared = match (&ext.bidder, alias) {
        (JsonField::Value(payload), None) | (JsonField::Missing, Some(payload)) => Some(payload),
        (JsonField::Null, None) => None,
        _ => return Err(invalid),
    };
    let empty = Parameters {
        pkey: "",
        categories: &[],
        advertisers: &[],
    };
    let Some(object) = declared.filter(|candidate| !candidate.is_null()) else {
        return Ok(empty);
    };
    let fields = object.as_object().ok_or(invalid)?;
    let pkey = match unique_field(fields, "pkey")? {
        None | Some(Value::Null) => "",
        Some(field) => field.as_str().ok_or(invalid)?,
    };
    Ok(Parameters {
        pkey,
        categories: string_array(unique_field(fields, "bcat")?)?,
        advertisers: string_array(unique_field(fields, "badv")?)?,
    })
}

#[derive(Clone, Copy)]
enum BlocklistKind {
    Category,
    Advertiser,
}

struct Blocklist<'a> {
    root: &'a [Value],
    prefix: &'a [Imp],
    kind: BlocklistKind,
}

impl Blocklist<'_> {
    fn entries<'a>(&self, params: Parameters<'a>) -> &'a [Value] {
        match self.kind {
            BlocklistKind::Category => params.categories,
            BlocklistKind::Advertiser => params.advertisers,
        }
    }

    fn is_empty(&self) -> bool {
        self.root.is_empty()
            && !self
                .prefix
                .iter()
                .filter_map(|imp| parameters(imp).ok())
                .any(|params| !self.entries(params).is_empty())
    }
}

impl Serialize for Blocklist<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(None)?;
        for value in self.root {
            seq.serialize_element(value)?;
        }
        for params in self.prefix.iter().filter_map(|imp| parameters(imp).ok()) {
            for value in self.entries(params) {
                seq.serialize_element(value)?;
            }
        }
        seq.end()
    }
}

#[derive(Clone, Copy)]
struct Impression<'a> {
    imp: &'a Imp,
    pkey: &'a str,
    floor: Option<f64>,
    currency: Option<&'a str>,
    media: CorrelationMedia,
}

struct Request<'a> {
    scoped: &'a BidRequest,
    impression: Impression<'a>,
    categories: Blocklist<'a>,
    advertisers: Blocklist<'a>,
}

impl Serialize for Request<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let BidRequest {
            id,
            imp: _original_impressions,
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
        } = self.scoped;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", id)?;
        map.serialize_entry("imp", &[self.impression])?;
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
        map.serialize_entry("source", &SourceView(source.as_ref()))?;
        if let Some(value) = at {
            map.serialize_entry("at", value)?;
        }
        if let Some(value) = tmax {
            map.serialize_entry("tmax", value)?;
        }
        if !cur.is_empty() {
            map.serialize_entry("cur", cur)?;
        }
        if let Some(value) = test
            && *value != 0_i32
        {
            map.serialize_entry("test", value)?;
        }
        if let Some(value) = ext {
            map.serialize_entry("ext", value)?;
        }
        for (key, value) in other {
            if key != "bcat" && key != "badv" {
                map.serialize_entry(key, value)?;
            }
        }
        if !self.categories.is_empty() {
            map.serialize_entry("bcat", &self.categories)?;
        }
        if !self.advertisers.is_empty() {
            map.serialize_entry("badv", &self.advertisers)?;
        }
        map.end()
    }
}

impl Serialize for Impression<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Imp {
            id,
            banner,
            video,
            native,
            audio: _removed_audio,
            bidfloor: _original_floor,
            bidfloorcur: _original_currency,
            secure,
            ext,
            other,
        } = self.imp;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", id)?;
        match self.media {
            CorrelationMedia::Banner => map.serialize_entry("banner", banner)?,
            CorrelationMedia::Video => map.serialize_entry("video", video)?,
            CorrelationMedia::Native => map.serialize_entry("native", native)?,
            CorrelationMedia::Audio => {
                return Err(S::Error::custom("unsupported sharethrough media"));
            }
        }
        if let Some(value) = self.floor {
            map.serialize_entry("bidfloor", &value)?;
        }
        if let Some(value) = self.currency {
            map.serialize_entry("bidfloorcur", value)?;
        }
        if let Some(value) = secure {
            map.serialize_entry("secure", value)?;
        }
        if let Some(value) = ext {
            map.serialize_entry("ext", value)?;
        }
        if !self.pkey.is_empty() {
            map.serialize_entry("tagid", self.pkey)?;
        }
        for (key, value) in other {
            if key != "tagid" {
                map.serialize_entry(key, value)?;
            }
        }
        map.end()
    }
}

struct SourceView<'a>(Option<&'a Source>);

impl Serialize for SourceView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        let mut extension = None;
        if let Some(Source {
            fd,
            tid,
            pchain,
            ext,
            other,
        }) = self.0
        {
            if let Some(value) = fd {
                map.serialize_entry("fd", value)?;
            }
            if let Some(value) = tid {
                map.serialize_entry("tid", value)?;
            }
            if let Some(value) = pchain {
                map.serialize_entry("pchain", value)?;
            }
            for (key, value) in other {
                map.serialize_entry(key, value)?;
            }
            extension = ext.as_ref();
        }
        map.serialize_entry("ext", &SourceExtension(extension))?;
        map.end()
    }
}

struct SourceExtension<'a>(Option<&'a SourceExt>);

impl Serialize for SourceExtension<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        if let Some(SourceExt { schain, other }) = self.0 {
            if let Some(value) = schain {
                map.serialize_entry("schain", value)?;
            }
            for (key, value) in other {
                if key != "str" && key != "version" {
                    map.serialize_entry(key, value)?;
                }
            }
        }
        map.serialize_entry("str", "10.0")?;
        map.serialize_entry("version", env!("CARGO_PKG_VERSION"))?;
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use super::super::SharethroughBidder;
    use super::super::tests::{ENDPOINT, banner, calls, info, mixed_request, request};
    use crate::bidders::harness;
    use crate::correlation::CorrelationMedia;
    use crate::{
        BidderHttpRequest, BidderRequestInfo, ConstructionDiagnostic, EndpointRejectReason,
        EndpointValidator, FloorConversion, SinkLimits,
    };
    use auctioneer_config_data::currency::FloorCurrencyConfig;
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

    fn body(call: &BidderHttpRequest) -> Value {
        serde_json::from_slice(call.body()).unwrap()
    }

    fn assert_foreign_floor_rollback(
        snapshot: Option<&FloorCurrencyConfig>,
        now: i64,
        amount: f64,
        currency: &str,
    ) {
        let mut info = info();
        info.floor_currency = FloorConversion::new(snapshot, now);
        let good = request(json!([banner("good")]));
        assert_eq!(
            harness::drive_ok(&SharethroughBidder, &good, &info).len(),
            1
        );
        let mut foreign = banner("foreign");
        foreign["bidfloor"] = json!(amount);
        foreign["bidfloorcur"] = json!(currency);
        let request = request(json!([banner("good"), foreign]));
        let before = serde_json::to_value(&request).unwrap();
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &SharethroughBidder,
            &request,
            &info,
            SinkLimits::new(4096, 2),
            2,
            &EndpointPolicy(true),
        );
        let errors = semantic.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(!errors[0].message.contains(currency));
        assert!(outcome.accepted.is_empty());
        assert_eq!(remaining, 2);
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn splits_banner_video_native_in_source_order_and_removes_audio() {
        let request = mixed_request();
        let before = serde_json::to_value(&request).unwrap();
        let calls = calls(&request);
        assert_eq!(calls.len(), 3);
        for (call, selected) in calls.iter().zip(["banner", "video", "native"]) {
            let document = body(call);
            assert_eq!(document["imp"].as_array().unwrap().len(), 1);
            assert_eq!(document["imp"][0]["tagid"], "placement");
            for media in ["banner", "video", "native", "audio"] {
                assert_eq!(document["imp"][0].get(media).is_some(), media == selected);
            }
            assert_eq!(document["imp"][0][selected], before["imp"][0][selected]);
        }
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn each_split_call_carries_exact_coverage_correlation_and_source_headers() {
        let request = mixed_request();
        let calls = calls(&request);
        assert_eq!(calls.len(), 3);
        for (call, media) in calls.iter().zip([
            CorrelationMedia::Banner,
            CorrelationMedia::Video,
            CorrelationMedia::Native,
        ]) {
            assert_eq!(call.method(), "POST");
            assert_eq!(call.url(), ENDPOINT);
            assert_eq!(call.covered_imp_ids().collect::<Vec<_>>(), ["i"]);
            let correlation = call.meta().imp_correlation(0).unwrap();
            assert_eq!(correlation.media(), media);
            assert_eq!(correlation.token(), 0);
            assert!(call.meta().imp_correlation(1).is_none());
            let headers = call.headers().collect::<Vec<_>>();
            assert!(headers.contains(&("Content-Type", "application/json;charset=utf-8")));
            assert!(headers.contains(&("Accept", "application/json")));
        }
    }

    #[test]
    fn each_impression_uses_its_own_placement_key() {
        let mut second = banner("second");
        second["ext"]["bidder"]["pkey"] = json!("other-placement");
        let calls = calls(&request(json!([banner("first"), second])));
        assert_eq!(calls.len(), 2);
        assert_eq!(body(&calls[0])["imp"][0]["tagid"], "placement");
        assert_eq!(body(&calls[1])["imp"][0]["tagid"], "other-placement");
        assert_eq!(calls[0].covered_imp_ids().collect::<Vec<_>>(), ["first"]);
        assert_eq!(calls[1].covered_imp_ids().collect::<Vec<_>>(), ["second"]);
    }

    #[test]
    fn placement_replaces_or_removes_an_existing_tag_without_mutating_input() {
        for (params, expected) in [
            (json!({"pkey":"replacement"}), Some("replacement")),
            (json!({"pkey":""}), None),
            (json!({"pkey":null}), None),
            (json!({}), None),
            (json!(null), None),
        ] {
            let mut imp = banner("i");
            imp["tagid"] = json!("original-tag");
            imp["ext"]["bidder"] = params;
            let request = request(json!([imp]));
            let before = serde_json::to_value(&request).unwrap();
            let emitted = calls(&request);
            assert_eq!(emitted.len(), 1);
            assert_eq!(
                body(&emitted[0])["imp"][0]
                    .get("tagid")
                    .and_then(Value::as_str),
                expected
            );
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
    }

    #[test]
    fn single_case_equivalent_parameter_keys_are_accepted() {
        for ext in [
            json!({"bidder":{"PKEY":"placement", "BCAT":["IAB1"], "BADV":["fake.example"]}}),
            json!({"BiDdEr":{"PkEy":"placement", "BcAt":["IAB1"], "BaDv":["fake.example"]}}),
        ] {
            let mut imp = banner("i");
            imp["ext"] = ext;
            let emitted = calls(&request(json!([imp])));
            assert_eq!(emitted.len(), 1);
            let document = body(&emitted[0]);
            assert_eq!(document["imp"][0]["tagid"], "placement");
            assert_eq!(document["bcat"], json!(["IAB1"]));
            assert_eq!(document["badv"], json!(["fake.example"]));
        }
    }

    #[test]
    fn competing_case_equivalent_parameter_envelopes_reject_only_their_impression() {
        for ext in [
            json!({"bidder":{"pkey":"one"}, "BIDDER":{"pkey":"two"}}),
            json!({"bidder":null, "BIDDER":{"pkey":"two"}}),
            json!({"Bidder":{"pkey":"one"}, "BIDDER":{"pkey":"two"}}),
        ] {
            let mut bad = banner("bad");
            bad["ext"] = ext;
            let request = request(json!([bad, banner("good")]));
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &SharethroughBidder,
                &request,
                &info(),
                SinkLimits::new(4096, 2),
                2,
                &EndpointPolicy(true),
            );
            semantic.unwrap();
            assert_eq!(outcome.accepted.len(), 1);
            assert_eq!(remaining, 1);
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

    #[test]
    fn source_version_is_frozen_for_missing_null_and_empty_source_extensions() {
        for source_case in [
            None,
            Some(json!(null)),
            Some(json!({})),
            Some(json!({"ext":null})),
        ] {
            let mut document = json!({"id":"r", "imp":[banner("i")]});
            if let Some(source) = source_case {
                document["source"] = source;
            }
            let request = serde_json::from_value(document).unwrap();
            let calls = calls(&request);
            assert_eq!(calls.len(), 1);
            assert_eq!(
                body(&calls[0])["source"],
                json!({"ext":{
                    "str":"10.0", "version":env!("CARGO_PKG_VERSION")
                }})
            );
        }
    }

    #[test]
    fn source_rewrite_preserves_supply_chain_unknown_fields_and_scoped_transaction_id() {
        let request: BidRequest = serde_json::from_value(json!({"id":"r", "imp":[banner("i")],
            "source":{"fd":1_i32, "tid":"scoped", "pchain":"chain", "keep":[true,null],
                "ext":{"str":"old", "version":"old", "keep":{"x":true},
                    "schain":{"ver":"1.0", "complete":1_i32, "nodes":[]}}}
        }))
        .unwrap();
        let before = serde_json::to_value(&request).unwrap();
        let calls = calls(&request);
        assert_eq!(calls.len(), 1);
        let mut expected = before["source"].clone();
        expected["ext"]["str"] = json!("10.0");
        expected["ext"]["version"] = json!(env!("CARGO_PKG_VERSION"));
        assert_eq!(body(&calls[0])["source"], expected);
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn nonobject_source_extension_is_safely_refused_by_the_typed_ingress_boundary() {
        for ext in [json!(false), json!(1_i32), json!("opaque"), json!([])] {
            let error = serde_json::from_value::<BidRequest>(json!({"id":"r", "imp":[banner("i")],
                "source":{"ext":ext}}))
            .unwrap_err();
            assert!(error.is_data());
        }
    }

    #[test]
    fn preserves_scoped_request_and_never_restores_scrubbed_identifiers() {
        let request: BidRequest = serde_json::from_value(json!({
            "id":"r", "imp":[banner("i")], "tmax":37_i32, "cur":["EUR"], "test":1_i32,
            "site":{"domain":"publisher.example", "publisher":{"id":"publisher"}},
            "device":{"ua":"scoped agent"}, "user":{"keywords":"scoped"},
            "regs":{"ext":{"keep":true}}, "ext":{"keep":[true,null]},
            "source":{"ext":{"keep":true}}, "bcat":["IAB1"], "badv":["root.example"]
        }))
        .unwrap();
        let before = serde_json::to_value(&request).unwrap();
        let calls = calls(&request);
        assert_eq!(calls.len(), 1);
        let mut expected = before.clone();
        expected["source"]["ext"]["str"] = json!("10.0");
        expected["source"]["ext"]["version"] = json!(env!("CARGO_PKG_VERSION"));
        expected["imp"][0]["tagid"] = json!("placement");
        assert_eq!(body(&calls[0]), expected);
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
        assert!(body(&calls[0])["device"].get("ip").is_none());
        assert!(body(&calls[0])["user"].get("id").is_none());
        assert!(body(&calls[0])["source"].get("tid").is_none());
    }

    #[test]
    fn zero_test_flag_is_omitted_without_mutating_scoped_input() {
        for flag in [None, Some(0_i32), Some(1_i32)] {
            let mut request = mixed_request();
            request.test = flag;
            let before = serde_json::to_value(&request).unwrap();
            let calls = calls(&request);
            assert_eq!(calls.len(), 3);
            let expected = flag
                .filter(|value| *value != 0_i32)
                .map(|nonzero| json!(nonzero));
            for call in &calls {
                assert_eq!(body(call).get("test"), expected.as_ref());
            }
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
    }

    #[test]
    fn cumulative_blocklists_include_preceding_unsupported_impressions_and_duplicates() {
        let request: BidRequest = serde_json::from_value(json!({"id":"r",
        "bcat":["root","dup"], "badv":["root.example"], "imp":[
            {"id":"unsupported", "audio":{"mimes":["audio/mp4"]},
                "ext":{"bidder":{"pkey":"audio", "bcat":["prior","dup"],
                    "badv":["prior.example"]}}},
            {"id":"first", "banner":{}, "ext":{"bidder":{"pkey":"first",
                "bcat":["first"], "badv":["first.example"]}}},
            {"id":"second", "video":{"mimes":["video/mp4"]},
                "ext":{"bidder":{"pkey":"second", "bcat":["second"],
                    "badv":["second.example"]}}}
        ]}))
        .unwrap();
        let before = serde_json::to_value(&request).unwrap();
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &SharethroughBidder,
            &request,
            &info(),
            SinkLimits::new(4096, 3),
            3,
            &EndpointPolicy(true),
        );
        semantic.unwrap();
        assert_eq!(outcome.accepted.len(), 2);
        assert_eq!(remaining, 1);
        assert_eq!(
            outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
            [ConstructionDiagnostic::UnsupportedMediaType]
        );
        assert_eq!(
            body(&outcome.accepted[0])["bcat"],
            json!(["root", "dup", "prior", "dup", "first"])
        );
        assert_eq!(
            body(&outcome.accepted[0])["badv"],
            json!(["root.example", "prior.example", "first.example"])
        );
        assert_eq!(
            body(&outcome.accepted[1])["bcat"],
            json!(["root", "dup", "prior", "dup", "first", "second"])
        );
        assert_eq!(
            body(&outcome.accepted[1])["badv"],
            json!([
                "root.example",
                "prior.example",
                "first.example",
                "second.example"
            ])
        );
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn malformed_parameters_skip_only_their_impression_without_polluting_blocklists() {
        for params in [
            json!(false),
            json!([]),
            json!({"pkey":4_i32}),
            json!({"pkey":"bad", "bcat":["must-not-leak",4_i32]}),
            json!({"pkey":"bad", "badv":"must-not-leak"}),
            json!({"pkey":"bad", "PKEY":"conflict"}),
            json!({"pkey":"bad", "bcat":[], "BCAT":["must-not-leak"]}),
            json!({"pkey":"bad", "badv":[], "BADV":["must-not-leak"]}),
        ] {
            let mut bad = banner("bad");
            bad["ext"]["bidder"] = params;
            let request = request(json!([bad, banner("good")]));
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &SharethroughBidder,
                &request,
                &info(),
                SinkLimits::new(4096, 2),
                2,
                &EndpointPolicy(true),
            );
            semantic.unwrap();
            assert_eq!(outcome.accepted.len(), 1);
            assert_eq!(remaining, 1);
            assert_eq!(
                outcome.accepted[0].covered_imp_ids().collect::<Vec<_>>(),
                ["good"]
            );
            assert_eq!(
                outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
                [ConstructionDiagnostic::InvalidImpressionParameters]
            );
            assert!(body(&outcome.accepted[0]).get("bcat").is_none());
            assert!(body(&outcome.accepted[0]).get("badv").is_none());
        }
    }

    #[test]
    fn empty_blocklists_do_not_synthesize_root_fields() {
        let mut imp = banner("i");
        imp["ext"]["bidder"]["bcat"] = json!([]);
        imp["ext"]["bidder"]["badv"] = json!([]);
        let calls = calls(&request(json!([imp])));
        assert_eq!(calls.len(), 1);
        assert!(body(&calls[0]).get("bcat").is_none());
        assert!(body(&calls[0]).get("badv").is_none());
    }

    #[test]
    fn usd_case_variants_missing_currency_and_nonpositive_floors_need_no_conversion() {
        for (floor, currency) in [
            (2.5_f64, Some("USD")),
            (2.5_f64, Some("usd")),
            (2.5_f64, Some("uSd")),
            (2.5_f64, Some("")),
            (2.5_f64, None),
            (0.0_f64, Some("EUR")),
            (-2.5_f64, Some("EUR")),
        ] {
            let mut imp = banner("i");
            imp["bidfloor"] = json!(floor);
            if let Some(currency_code) = currency {
                imp["bidfloorcur"] = json!(currency_code);
            }
            let calls = calls(&request(json!([imp])));
            assert_eq!(calls.len(), 1);
            assert_eq!(body(&calls[0])["imp"][0]["bidfloor"], json!(floor));
            assert_eq!(
                body(&calls[0])["imp"][0].get("bidfloorcur"),
                currency.map(|s| json!(s)).as_ref()
            );
        }
    }

    #[test]
    fn late_conversion_failure_discards_all_earlier_calls_and_refunds_their_slots() {
        assert_eq!(calls(&request(json!([banner("good")]))).len(), 1);
        let mut foreign = banner("foreign");
        foreign["bidfloor"] = json!(2.5_f64);
        foreign["bidfloorcur"] = json!("EUR");
        let request = request(json!([banner("good"), foreign]));
        let before = serde_json::to_value(&request).unwrap();
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &SharethroughBidder,
            &request,
            &info(),
            SinkLimits::new(4096, 2),
            2,
            &EndpointPolicy(true),
        );
        assert!(semantic.is_err());
        assert!(outcome.accepted.is_empty());
        assert_eq!(remaining, 2);
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn borrowed_operator_rates_convert_every_split_floor_without_changing_input_currency() {
        for (rate, expected) in [(2.5_f64, 10.0_f64), (0.25_f64, 1.0_f64)] {
            let snapshot = serde_json::from_value(json!({
                "expires_at":2000_i64, "usd_per_unit":{"QAA":rate}
            }))
            .unwrap();
            let mut info = info();
            info.floor_currency = FloorConversion::new(Some(&snapshot), 1000);
            let mut request = mixed_request();
            request.cur = vec!["EUR".to_owned()];
            request.imp[0].bidfloor = Some(4.0_f64);
            request.imp[0].bidfloorcur = Some("QAA".to_owned());
            let before = serde_json::to_value(&request).unwrap();
            let calls = harness::drive_ok(&SharethroughBidder, &request, &info);
            assert_eq!(calls.len(), 3);
            for call in &calls {
                let body = body(call);
                assert_eq!(body["imp"][0]["bidfloor"], json!(expected));
                assert_eq!(body["imp"][0]["bidfloorcur"], "USD");
                assert_eq!(body["cur"], json!(["EUR"]));
            }
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
    }

    #[test]
    fn missing_operator_rate_rolls_back_the_entire_bidder() {
        let snapshot = serde_json::from_value(json!({
            "expires_at":2000_i64, "usd_per_unit":{"QAA":2.5_f64}
        }))
        .unwrap();
        assert_foreign_floor_rollback(Some(&snapshot), 1000, 4.0, "QAB");
    }

    #[test]
    fn expired_operator_snapshot_rolls_back_the_entire_bidder() {
        let snapshot = serde_json::from_value(json!({
            "expires_at":2000_i64, "usd_per_unit":{"QAA":2.5_f64}
        }))
        .unwrap();
        for now in [2000_i64, 2001] {
            assert_foreign_floor_rollback(Some(&snapshot), now, 4.0, "QAA");
        }
    }

    #[test]
    fn lowercase_foreign_codes_use_operator_rates_after_shared_ascii_normalization() {
        let snapshot = serde_json::from_value(json!({
            "expires_at":2000_i64, "usd_per_unit":{"QAA":2.5_f64}
        }))
        .unwrap();
        let mut info = info();
        info.floor_currency = FloorConversion::new(Some(&snapshot), 1000);
        for code in ["qaa", "Qaa", "qAa", "qaA"] {
            let mut request = mixed_request();
            request.imp[0].bidfloor = Some(4.0_f64);
            request.imp[0].bidfloorcur = Some(code.to_owned());
            let before = serde_json::to_value(&request).unwrap();
            let calls = harness::drive_ok(&SharethroughBidder, &request, &info);
            assert_eq!(calls.len(), 3);
            for call in &calls {
                let document = body(call);
                assert_eq!(document["imp"][0]["bidfloor"], json!(10.0_f64));
                assert_eq!(document["imp"][0]["bidfloorcur"], "USD");
            }
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
    }

    #[test]
    fn malformed_foreign_codes_roll_back_the_entire_bidder() {
        let snapshot = serde_json::from_value(json!({
            "expires_at":2000_i64, "usd_per_unit":{"QAA":2.5_f64}
        }))
        .unwrap();
        for code in ["Q1A", "QAAA", " QAA", "QAA ", "QA", "Q\u{e9}A"] {
            assert_foreign_floor_rollback(Some(&snapshot), 1000, 4.0, code);
        }
    }

    #[test]
    fn conversion_overflow_and_underflow_roll_back_the_entire_bidder() {
        let overflow = serde_json::from_value(json!({
            "expires_at":2000_i64, "usd_per_unit":{"QAA":f64::MAX}
        }))
        .unwrap();
        assert_foreign_floor_rollback(Some(&overflow), 1000, f64::MAX, "QAA");
        let underflow = serde_json::from_value(json!({
            "expires_at":2000_i64, "usd_per_unit":{"QAA":0.25_f64}
        }))
        .unwrap();
        assert_foreign_floor_rollback(Some(&underflow), 1000, f64::from_bits(1), "QAA");
    }

    #[test]
    fn conversion_failure_is_fatal_even_when_the_failing_impression_has_no_supported_media() {
        assert_eq!(calls(&request(json!([banner("good")]))).len(), 1);
        let request = request(json!([banner("good"), {"id":"audio",
            "audio":{"mimes":["audio/mp4"]}, "bidfloor":2.5_f64,"bidfloorcur":"EUR",
            "ext":{"bidder":{"pkey":"audio"}}}]));
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &SharethroughBidder,
            &request,
            &info(),
            SinkLimits::new(4096, 2),
            2,
            &EndpointPolicy(true),
        );
        assert!(semantic.is_err());
        assert!(outcome.accepted.is_empty());
        assert_eq!(remaining, 2);
    }

    #[test]
    fn body_cap_refusal_refunds_its_slot_and_preserves_a_later_small_call() {
        let mut large = banner("large");
        large["keep"] = json!("x".repeat(4096));
        let request = request(json!([large, banner("small")]));
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &SharethroughBidder,
            &request,
            &info(),
            SinkLimits::new(1024, 1),
            1,
            &EndpointPolicy(true),
        );
        semantic.unwrap();
        assert_eq!(outcome.accepted.len(), 1);
        assert_eq!(
            outcome.accepted[0].covered_imp_ids().collect::<Vec<_>>(),
            ["small"]
        );
        assert_eq!(outcome.errors.len(), 1);
        assert_eq!(remaining, 0);
    }

    #[test]
    fn exhausted_call_budget_keeps_exact_first_split_coverage() {
        let request = mixed_request();
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &SharethroughBidder,
            &request,
            &info(),
            SinkLimits::new(4096, 1),
            1,
            &EndpointPolicy(true),
        );
        semantic.unwrap();
        assert_eq!(outcome.accepted.len(), 1);
        assert_eq!(remaining, 0);
        assert_eq!(
            outcome.accepted[0].covered_imp_ids().collect::<Vec<_>>(),
            ["i"]
        );
        assert_eq!(
            outcome.accepted[0]
                .meta()
                .imp_correlation(0)
                .unwrap()
                .media(),
            CorrelationMedia::Banner
        );
        assert_eq!(outcome.rejected_call_count, 2);
    }

    #[test]
    fn endpoint_refusal_never_dispatches_or_retains_a_call_slot() {
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &SharethroughBidder,
            &request(json!([banner("i")])),
            &info(),
            SinkLimits::new(4096, 1),
            1,
            &EndpointPolicy(false),
        );
        semantic.unwrap();
        assert!(outcome.accepted.is_empty());
        assert_eq!(outcome.errors.len(), 1);
        assert_eq!(remaining, 1);
    }

    #[test]
    fn empty_impression_list_is_a_successful_noop() {
        assert!(calls(&request(json!([]))).is_empty());
    }

    #[test]
    fn all_invalid_impressions_refuse_with_bounded_diagnostics_and_no_retained_calls() {
        for (imp, diagnostic) in [
            (
                json!({"id":"bad", "banner":{}, "ext":{"bidder":{"pkey":4_i32}}}),
                ConstructionDiagnostic::InvalidImpressionParameters,
            ),
            (
                json!({"id":"audio", "audio":{"mimes":["audio/mp4"]},
                "ext":{"bidder":{"pkey":"placement"}}}),
                ConstructionDiagnostic::UnsupportedMediaType,
            ),
        ] {
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &SharethroughBidder,
                &request(json!([imp])),
                &info(),
                SinkLimits::new(4096, 1),
                1,
                &EndpointPolicy(true),
            );
            assert!(semantic.is_err());
            assert!(outcome.accepted.is_empty());
            assert_eq!(remaining, 1);
            assert_eq!(
                outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
                [diagnostic]
            );
        }
    }

    #[test]
    fn zero_slots_refuse_every_split_without_a_semantic_failure() {
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &SharethroughBidder,
            &mixed_request(),
            &info(),
            SinkLimits::new(4096, 3),
            0,
            &EndpointPolicy(true),
        );
        semantic.unwrap();
        assert!(outcome.accepted.is_empty());
        assert_eq!(remaining, 0);
        assert_eq!(outcome.rejected_call_count, 3);
    }
}
