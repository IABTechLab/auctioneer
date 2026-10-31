use crate::bidders::url::{json_utf8_headers, write_endpoint};
use crate::correlation::CorrelationMedia;
use crate::{BidderError, BidderRequestInfo, ConstructionDiagnostic, OutboundRequestSink};
use auctioneer_openrtb::{
    ext::misc::RegsExt,
    types::{
        bid_request::{Banner, BidRequest, Imp},
        field::JsonField,
        inventory::{App, Site},
        regs::Regs,
    },
};
use serde::{Serialize, Serializer, ser::SerializeMap as _};
use serde_json::{Map, Value};

pub(super) fn make_requests(
    request: &BidRequest,
    info: &BidderRequestInfo<'_>,
    sink: &mut OutboundRequestSink<'_>,
) -> Result<(), Vec<BidderError>> {
    let mut has_valid_call = false;
    for imp in &request.imp {
        let params = match parameters(imp) {
            Ok(params) => params,
            Err(reason) => {
                sink.diagnostic(reason);
                continue;
            }
        };
        if imp.banner.is_none() && imp.video.is_none() {
            sink.diagnostic(ConstructionDiagnostic::UnsupportedMediaType);
            continue;
        }
        let (media, dimensions) = if let Some(banner) = imp.banner.as_ref() {
            match banner_dimensions(banner) {
                Ok(dimensions) => (CorrelationMedia::Banner, Some(dimensions)),
                Err(reason) => {
                    sink.diagnostic(reason);
                    continue;
                }
            }
        } else {
            (CorrelationMedia::Video, None)
        };
        has_valid_call = true;
        let impression = Impression {
            imp,
            params,
            dimensions,
        };
        let _recorded = sink.write_json(
            info,
            |meta| {
                meta.set_method("POST")?;
                write_endpoint(meta, info)?;
                json_utf8_headers(meta)?;
                meta.header("x-openrtb-version", "2.5")?;
                if let Some(agent) = request
                    .device
                    .as_ref()
                    .and_then(|device| device.ua.as_deref())
                    .filter(|agent| !agent.is_empty())
                {
                    meta.header("User-Agent", agent)?;
                }
                meta.imp_id(&imp.id)?;
                // Response precedence only; other offered media stay on the wire.
                meta.correlate_imp(0, media, 0)?;
                Ok(())
            },
            &Request {
                request,
                impression,
            },
        );
    }
    // A transport or budget refusal is already sink-owned and must not
    // turn a valid earlier call into a bidder-fatal semantic failure.
    if has_valid_call || request.imp.is_empty() {
        Ok(())
    } else {
        Err(vec![BidderError::bad_input(
            "no valid yahoo ads impressions",
        )])
    }
}

#[derive(Clone, Copy)]
struct Parameters<'a> {
    dcn: &'a str,
    pos: &'a str,
}

fn unique_field<'a>(
    fields: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a Value>, ConstructionDiagnostic> {
    let mut matching = fields
        .iter()
        .filter(|(key, _value)| key.eq_ignore_ascii_case(name));
    let value = matching.next().map(|(_key, value)| value);
    if matching.next().is_some() {
        return Err(ConstructionDiagnostic::InvalidImpressionParameters);
    }
    Ok(value)
}

fn parameters(imp: &Imp) -> Result<Parameters<'_>, ConstructionDiagnostic> {
    let invalid = ConstructionDiagnostic::InvalidImpressionParameters;
    let ext = imp.ext.as_ref().ok_or(invalid)?;
    let alias = unique_field(&ext.other, "bidder")?;
    let value = match (&ext.bidder, alias) {
        (JsonField::Value(value), None) | (JsonField::Missing, Some(value)) => value,
        (JsonField::Null, None) => return Ok(Parameters { dcn: "", pos: "" }),
        _ => return Err(invalid),
    };
    if value.is_null() {
        return Ok(Parameters { dcn: "", pos: "" });
    }
    let fields = value.as_object().ok_or(invalid)?;
    let string = |name| match unique_field(fields, name)? {
        None | Some(Value::Null) => Ok(""),
        Some(field) => field.as_str().ok_or(invalid),
    };
    Ok(Parameters {
        dcn: string("dcn")?,
        pos: string("pos")?,
    })
}

fn banner_dimensions(banner: &Banner) -> Result<(i32, i32), ConstructionDiagnostic> {
    if let (Some(width), Some(height)) = (banner.w, banner.h) {
        return if width == 0 || height == 0 {
            Err(ConstructionDiagnostic::InvalidImpression)
        } else {
            Ok((width, height))
        };
    }
    // Deliberately do not add a second validation of the first format's sizes.
    banner
        .format
        .first()
        .map(|format| (format.w, format.h))
        .ok_or(ConstructionDiagnostic::InvalidImpression)
}

#[derive(Clone, Copy)]
struct Impression<'a> {
    imp: &'a Imp,
    params: Parameters<'a>,
    dimensions: Option<(i32, i32)>,
}

struct Request<'a> {
    request: &'a BidRequest,
    impression: Impression<'a>,
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
        } = self.request;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", id)?;
        map.serialize_entry("imp", &[self.impression])?;
        if let Some(inventory) = site {
            map.serialize_entry(
                "site",
                &SiteView {
                    site: inventory,
                    id: self.impression.params.dcn,
                },
            )?;
        }
        if let Some(inventory) = app {
            if site.is_some() {
                map.serialize_entry("app", inventory)?;
            } else {
                map.serialize_entry(
                    "app",
                    &AppView {
                        app: inventory,
                        id: self.impression.params.dcn,
                    },
                )?;
            }
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
            if let Some(gpp) = value.gpp.as_deref().filter(|gpp| !gpp.is_empty()) {
                map.serialize_entry("regs", &RegsView { regs: value, gpp })?;
            } else {
                map.serialize_entry("regs", value)?;
            }
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
        if let (Some(display), Some((width, height))) = (banner, self.dimensions) {
            map.serialize_entry(
                "banner",
                &BannerView {
                    banner: display,
                    width,
                    height,
                },
            )?;
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
        if !self.params.pos.is_empty() {
            map.serialize_entry("tagid", self.params.pos)?;
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

struct BannerView<'a> {
    banner: &'a Banner,
    width: i32,
    height: i32,
}

impl Serialize for BannerView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Banner {
            format,
            w: _original_width,
            h: _original_height,
            pos,
            ext,
            other,
        } = self.banner;
        let mut map = serializer.serialize_map(None)?;
        if !format.is_empty() {
            map.serialize_entry("format", format)?;
        }
        map.serialize_entry("w", &self.width)?;
        map.serialize_entry("h", &self.height)?;
        if let Some(value) = pos {
            map.serialize_entry("pos", value)?;
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

struct SiteView<'a> {
    site: &'a Site,
    id: &'a str,
}

impl Serialize for SiteView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Site {
            id: _original_id,
            name,
            domain,
            page,
            publisher,
            ext,
            other,
        } = self.site;
        let mut map = serializer.serialize_map(None)?;
        if !self.id.is_empty() {
            map.serialize_entry("id", self.id)?;
        }
        if let Some(value) = name {
            map.serialize_entry("name", value)?;
        }
        if let Some(value) = domain {
            map.serialize_entry("domain", value)?;
        }
        if let Some(value) = page {
            map.serialize_entry("page", value)?;
        }
        if let Some(value) = publisher {
            map.serialize_entry("publisher", value)?;
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

struct AppView<'a> {
    app: &'a App,
    id: &'a str,
}

impl Serialize for AppView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let App {
            id: _original_id,
            name,
            bundle,
            domain,
            publisher,
            ext,
            other,
        } = self.app;
        let mut map = serializer.serialize_map(None)?;
        if !self.id.is_empty() {
            map.serialize_entry("id", self.id)?;
        }
        if let Some(value) = name {
            map.serialize_entry("name", value)?;
        }
        if let Some(value) = bundle {
            map.serialize_entry("bundle", value)?;
        }
        if let Some(value) = domain {
            map.serialize_entry("domain", value)?;
        }
        if let Some(value) = publisher {
            map.serialize_entry("publisher", value)?;
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

struct RegsView<'a> {
    regs: &'a Regs,
    gpp: &'a str,
}

impl Serialize for RegsView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Regs {
            coppa,
            gdpr,
            us_privacy,
            gpp: _moved_gpp,
            gpp_sid,
            ext,
            other,
        } = self.regs;
        let mut map = serializer.serialize_map(None)?;
        if let Some(value) = coppa {
            map.serialize_entry("coppa", value)?;
        }
        if let Some(value) = gdpr {
            map.serialize_entry("gdpr", value)?;
        }
        if let Some(value) = us_privacy {
            map.serialize_entry("us_privacy", value)?;
        }
        map.serialize_entry(
            "ext",
            &RegsExtension {
                ext: ext.as_ref(),
                gpp: self.gpp,
                sid: gpp_sid.as_deref(),
            },
        )?;
        for (key, value) in other {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

struct RegsExtension<'a> {
    ext: Option<&'a RegsExt>,
    gpp: &'a str,
    sid: Option<&'a [i32]>,
}

impl Serialize for RegsExtension<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        if let Some(RegsExt { dsa, gpc, other }) = self.ext {
            if let Some(value) = dsa {
                map.serialize_entry("dsa", value)?;
            }
            if let Some(value) = gpc {
                map.serialize_entry("gpc", value)?;
            }
            for (key, value) in other {
                if key != "gpp" && (key != "gpp_sid" || self.sid.is_none()) {
                    map.serialize_entry(key, value)?;
                }
            }
        }
        map.serialize_entry("gpp", self.gpp)?;
        if let Some(sid) = self.sid {
            map.serialize_entry("gpp_sid", sid)?;
        }
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use super::super::YahooAdsBidder;
    use super::super::tests::{ENDPOINT, banner, info, mixed_request, request};
    use crate::bidders::harness;
    use crate::correlation::CorrelationMedia;
    use crate::{
        BidderError, BidderRequestInfo, ConstructionDiagnostic, EndpointRejectReason,
        EndpointValidator, OutboundBuildError, SinkLimits,
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

    #[test]
    fn one_mixed_source_impression_generates_one_banner_annotated_call_preserving_all_media() {
        let mut request = mixed_request();
        request.imp[0].native =
            serde_json::from_value(json!({"request":"native payload"})).unwrap();
        request.imp[0].audio = serde_json::from_value(json!({"mimes":["audio/mp3"]})).unwrap();
        let before = serde_json::to_value(&request).unwrap();
        let calls = harness::drive_ok(&YahooAdsBidder, &request, &info());
        assert_eq!(calls.len(), 1);
        let call = &calls[0];
        assert_eq!(call.method(), "POST");
        assert_eq!(call.url(), ENDPOINT);
        assert_eq!(call.covered_imp_ids().collect::<Vec<_>>(), ["i"]);
        let context = call.meta().imp_correlation(0).unwrap();
        assert_eq!(context.media(), CorrelationMedia::Banner);
        assert_eq!(context.token(), 0);
        assert!(call.meta().imp_correlation(1).is_none());
        let body: Value = serde_json::from_slice(call.body()).unwrap();
        assert_eq!(body["imp"].as_array().unwrap().len(), 1);
        assert_eq!(
            body["imp"][0]["banner"],
            json!({"format":[{"w":728_i32,"h":90_i32}],"w":728_i32,"h":90_i32})
        );
        assert_eq!(body["imp"][0]["video"], before["imp"][0]["video"]);
        assert_eq!(body["imp"][0]["native"], before["imp"][0]["native"]);
        assert_eq!(body["imp"][0]["audio"], before["imp"][0]["audio"]);
        assert_eq!(body["imp"][0]["tagid"], "position");
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn split_rewrite_preserves_scoped_fields_and_all_unmodified_extensions() {
        let request: BidRequest = serde_json::from_value(json!({
            "id":"r", "tmax":37_i32, "cur":["EUR"], "test":1_i32,
            "site":{"id":"old", "domain":"publisher.example", "publisher":{"id":"publisher"}},
            "device":{"ua":"scoped agent", "ip":"192.0.2.1"},
            "user":{"keywords":"scoped keywords"}, "source":{"tid":"scoped-tid"},
            "ext":{"opaque":{"keep":[true,null]}}, "bcat":["IAB1"],
            "imp":[{"id":"i", "tagid":"old", "banner":{"w":300_i32,"h":250_i32,"ext":{"keep":true}},
                "bidfloor":0.75_f64, "bidfloorcur":"EUR", "secure":1_i32,
                "ext":{"bidder":{"dcn":"site","pos":"position","opaque":true},
                    "prebid":{"adunitcode":"slot"}, "keep":[1_i32,null]}, "instl":1_i32}]
        }))
        .unwrap();
        let before = serde_json::to_value(&request).unwrap();
        let calls = harness::drive_ok(&YahooAdsBidder, &request, &info());
        assert_eq!(calls.len(), 1);
        assert_eq!(
            serde_json::from_slice::<Value>(calls[0].body()).unwrap(),
            json!({
                "id":"r", "tmax":37_i32, "cur":["EUR"], "test":1_i32,
                "site":{"id":"site", "domain":"publisher.example", "publisher":{"id":"publisher"}},
                "device":{"ua":"scoped agent", "ip":"192.0.2.1"},
                "user":{"keywords":"scoped keywords"}, "source":{"tid":"scoped-tid"},
                "ext":{"opaque":{"keep":[true,null]}}, "bcat":["IAB1"],
                "imp":[{"id":"i", "tagid":"position", "banner":{"w":300_i32,"h":250_i32,"ext":{"keep":true}},
                    "bidfloor":0.75_f64, "bidfloorcur":"EUR", "secure":1_i32,
                    "ext":{"bidder":{"dcn":"site","pos":"position","opaque":true},
                        "prebid":{"adunitcode":"slot"}, "keep":[1_i32,null]}, "instl":1_i32}]
            })
        );
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn inventory_rewrite_prefers_site_then_app_and_never_synthesizes_either() {
        for (inventory, expected_site, expected_app) in [
            (
                json!({"site":{"id":"old-site"},"app":{"id":"old-app","bundle":"bundle"}}),
                Some(json!({"id":"site"})),
                Some(json!({"id":"old-app","bundle":"bundle"})),
            ),
            (
                json!({"app":{"id":"old-app","bundle":"bundle"}}),
                None,
                Some(json!({"id":"site","bundle":"bundle"})),
            ),
            (json!({}), None, None),
        ] {
            let mut document = json!({"id":"r", "imp":[banner("i")]});
            document
                .as_object_mut()
                .unwrap()
                .extend(inventory.as_object().unwrap().clone());
            let request = serde_json::from_value(document).unwrap();
            let calls = harness::drive_ok(&YahooAdsBidder, &request, &info());
            assert_eq!(calls.len(), 1);
            let body: Value = serde_json::from_slice(calls[0].body()).unwrap();
            assert_eq!(body.get("site"), expected_site.as_ref());
            assert_eq!(body.get("app"), expected_app.as_ref());
        }
    }

    #[test]
    fn dimensions_keep_complete_pair_or_replace_both_from_first_format() {
        for (dimensions, expected) in [
            (
                json!({"w":300_i32,"h":250_i32,"format":[{"w":728_i32,"h":90_i32}]}),
                [300_i32, 250_i32],
            ),
            (
                json!({"w":300_i32,"format":[{"w":728_i32,"h":90_i32},{"w":320_i32,"h":50_i32}]}),
                [728_i32, 90_i32],
            ),
            (
                json!({"h":250_i32,"format":[{"w":728_i32,"h":90_i32}]}),
                [728_i32, 90_i32],
            ),
            (json!({"format":[{"w":0_i32,"h":0_i32}]}), [0_i32, 0_i32]),
        ] {
            let mut imp = banner("i");
            imp["banner"] = dimensions;
            let calls = harness::drive_ok(&YahooAdsBidder, &request(json!([imp])), &info());
            assert_eq!(calls.len(), 1);
            let body: Value = serde_json::from_slice(calls[0].body()).unwrap();
            assert_eq!(body["imp"][0]["banner"]["w"], expected[0]);
            assert_eq!(body["imp"][0]["banner"]["h"], expected[1]);
        }
    }

    #[test]
    fn invalid_banner_refuses_its_entire_impression_but_preserves_later_good_impression() {
        for bad in [
            json!({"w":0_i32,"h":90_i32}),
            json!({"w":728_i32,"h":0_i32}),
            json!({"format":[]}),
        ] {
            let mut mixed = banner("mixed");
            mixed["banner"] = bad;
            mixed["video"] = json!({"mimes":["video/mp4"]});
            let request = request(json!([mixed, banner("later")]));
            let before = serde_json::to_value(&request).unwrap();
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &YahooAdsBidder,
                &request,
                &info(),
                SinkLimits::new(4096, 2),
                2,
                &EndpointPolicy(true),
            );
            semantic.unwrap();
            assert!(outcome.errors.is_empty());
            assert_eq!(
                outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
                [ConstructionDiagnostic::InvalidImpression]
            );
            assert_eq!(outcome.accepted.len(), 1);
            assert_eq!(remaining, 1);
            assert_eq!(
                outcome.accepted[0].covered_imp_ids().collect::<Vec<_>>(),
                ["later"]
            );
            assert_eq!(
                outcome.accepted[0]
                    .meta()
                    .imp_correlation(0)
                    .unwrap()
                    .media(),
                CorrelationMedia::Banner
            );
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
    }

    #[test]
    fn malformed_parameter_types_only_refuse_their_impression() {
        for params in [
            json!(false),
            json!([]),
            json!({"dcn":7_i32,"pos":"position"}),
            json!({"dcn":"site","pos":false}),
        ] {
            let mut bad = banner("private-bad-id");
            bad["ext"]["bidder"] = params;
            let request = request(json!([bad, banner("good")]));
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &YahooAdsBidder,
                &request,
                &info(),
                SinkLimits::new(4096, 2),
                1,
                &EndpointPolicy(true),
            );
            semantic.unwrap();
            assert!(outcome.errors.is_empty());
            assert_eq!(
                outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
                [ConstructionDiagnostic::InvalidImpressionParameters]
            );
            assert_eq!(outcome.accepted.len(), 1);
            assert_eq!(
                outcome.accepted[0].covered_imp_ids().collect::<Vec<_>>(),
                ["good"]
            );
            assert_eq!(remaining, 0);
        }
    }

    #[test]
    fn direct_adapter_does_not_impose_schema_required_or_min_length_rules() {
        for params in [
            Value::Null,
            json!({}),
            json!({"dcn":null,"pos":null}),
            json!({"dcn":"","pos":""}),
            json!({"dcn":"site"}),
        ] {
            let mut imp = banner("i");
            imp["ext"]["bidder"] = params;
            let calls = harness::drive_ok(&YahooAdsBidder, &request(json!([imp])), &info());
            assert_eq!(calls.len(), 1);
            let body: Value = serde_json::from_slice(calls[0].body()).unwrap();
            assert!(body["imp"][0].get("tagid").is_none());
            assert_eq!(body["imp"][0]["banner"]["w"], 728_i32);
        }
    }

    #[test]
    fn single_equivalent_parameter_spelling_is_accepted_without_rewriting_extension_keys() {
        let mut imp = banner("i");
        imp["ext"] = json!({"BIDDER":{"DCN":"case-site","Pos":"case-position"}, "keep":true});
        let calls = harness::drive_ok(&YahooAdsBidder, &request(json!([imp])), &info());
        assert_eq!(calls.len(), 1);
        let body: Value = serde_json::from_slice(calls[0].body()).unwrap();
        assert_eq!(body["imp"][0]["tagid"], "case-position");
        assert_eq!(
            body["imp"][0]["ext"],
            json!({"BIDDER":{"DCN":"case-site","Pos":"case-position"}, "keep":true})
        );
    }

    #[test]
    fn competing_parameter_or_container_spellings_refuse_only_their_impression() {
        for ext in [
            json!({"bidder":{"dcn":"site","DCN":"site","pos":"position"}}),
            json!({"bidder":{"dcn":"site","Pos":null,"POS":null}}),
            json!({"bidder":{"dcn":"site","pos":"position"},"BIDDER":null}),
            json!({"Bidder":null,"BIDDER":null}),
        ] {
            let mut bad = banner("bad");
            bad["ext"] = ext;
            let (semantic, outcome, _) = harness::drive_with_limits(
                &YahooAdsBidder,
                &request(json!([bad, banner("good")])),
                &info(),
                SinkLimits::new(4096, 2),
                1,
                &EndpointPolicy(true),
            );
            semantic.unwrap();
            assert_eq!(outcome.accepted.len(), 1);
            assert_eq!(
                outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
                [ConstructionDiagnostic::InvalidImpressionParameters]
            );
        }
    }

    #[test]
    fn no_valid_impressions_has_fixed_semantic_error_and_spends_no_call() {
        let request = request(json!([{"id":"private-id","banner":{"w":300_i32,"h":250_i32}}]));
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &YahooAdsBidder,
            &request,
            &info(),
            SinkLimits::new(4096, 1),
            1,
            &EndpointPolicy(true),
        );
        assert_eq!(
            semantic.unwrap_err(),
            [BidderError::bad_input("no valid yahoo ads impressions")]
        );
        assert!(outcome.accepted.is_empty());
        assert!(outcome.errors.is_empty());
        assert_eq!(remaining, 1);
    }

    #[test]
    fn empty_impression_list_is_a_successful_no_call() {
        let (semantic, calls) = harness::drive(&YahooAdsBidder, &request(json!([])), &info());
        semantic.unwrap();
        assert!(calls.is_empty());
    }

    #[test]
    fn unsupported_only_impressions_are_locally_refused_without_a_transport_call() {
        let request = request(json!([{
            "id":"i", "audio":{"mimes":["audio/mp3"]},
            "ext":{"bidder":{"dcn":"site","pos":"position"}}
        }, banner("good")]));
        let (semantic, outcome, _) = harness::drive_with_limits(
            &YahooAdsBidder,
            &request,
            &info(),
            SinkLimits::new(4096, 2),
            1,
            &EndpointPolicy(true),
        );
        semantic.unwrap();
        assert_eq!(outcome.accepted.len(), 1);
        assert_eq!(
            outcome.accepted[0].covered_imp_ids().collect::<Vec<_>>(),
            ["good"]
        );
        assert_eq!(
            outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
            [ConstructionDiagnostic::UnsupportedMediaType]
        );
    }

    #[test]
    fn nonempty_gpp_moves_into_extension_and_preserves_map_key_case() {
        for (sid, expected_sid) in [
            (Some(json!([6_i32])), json!([6_i32])),
            (Some(json!([])), json!([])),
            (None, json!([7_i32])),
        ] {
            let mut document = json!({"id":"r", "imp":[banner("i")], "regs":{
                "coppa":1_i32,"gpp":"consent", "ext":{
                    "gpp":"old", "gpp_sid":[7_i32], "GPP":"case-sensitive map key", "keep":{"null":null}
                }
            }});
            if let Some(value) = sid {
                document["regs"]["gpp_sid"] = value;
            }
            let request = serde_json::from_value(document).unwrap();
            let before = serde_json::to_value(&request).unwrap();
            let calls = harness::drive_ok(&YahooAdsBidder, &request, &info());
            let body: Value = serde_json::from_slice(calls[0].body()).unwrap();
            assert_eq!(
                body["regs"],
                json!({"coppa":1_i32,"ext":{
                    "gpp":"consent", "gpp_sid":expected_sid,
                    "GPP":"case-sensitive map key", "keep":{"null":null}
                }})
            );
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
    }

    #[test]
    fn absent_or_typed_null_regs_extension_safely_gains_gpp() {
        for regs in [
            json!({"gpp":"consent","gpp_sid":[]}),
            json!({"gpp":"consent","gpp_sid":[],"ext":null}),
        ] {
            let request =
                serde_json::from_value(json!({"id":"r", "imp":[banner("i")],"regs":regs})).unwrap();
            let calls = harness::drive_ok(&YahooAdsBidder, &request, &info());
            let body: Value = serde_json::from_slice(calls[0].body()).unwrap();
            assert_eq!(body["regs"], json!({"ext":{"gpp":"consent","gpp_sid":[]}}));
        }
    }

    #[test]
    fn empty_top_level_gpp_does_not_move_or_clear_any_privacy_field() {
        let request = serde_json::from_value(json!({"id":"r", "imp":[banner("i")],"regs":{
            "gpp":"","gpp_sid":[6_i32],"ext":{"gpp":"legacy","gpp_sid":[7_i32],"gdpr":1_i32}
        }}))
        .unwrap();
        let calls = harness::drive_ok(&YahooAdsBidder, &request, &info());
        let body: Value = serde_json::from_slice(calls[0].body()).unwrap();
        assert_eq!(
            body["regs"],
            json!({
                "gpp":"","gpp_sid":[6_i32],"ext":{"gpp":"legacy","gpp_sid":[7_i32],"gdpr":1_i32}
            })
        );
    }

    #[test]
    fn scoped_user_agent_is_optional_and_only_source_declared_headers_are_added() {
        for agent in [None, Some(""), Some("scoped agent")] {
            let mut document = json!({"id":"r", "imp":[banner("i")]});
            if let Some(value) = agent {
                document["device"] = json!({"ua":value});
            }
            let request = serde_json::from_value(document).unwrap();
            let calls = harness::drive_ok(&YahooAdsBidder, &request, &info());
            assert_eq!(calls.len(), 1);
            let mut expected = vec![
                ("Content-Type", "application/json;charset=utf-8"),
                ("Accept", "application/json"),
                ("x-openrtb-version", "2.5"),
            ];
            if let Some(value) = agent.filter(|value| !value.is_empty()) {
                expected.push(("User-Agent", value));
            }
            assert_eq!(calls[0].headers().collect::<Vec<_>>(), expected);
        }
    }

    #[test]
    fn multiple_impressions_keep_order_and_independent_inventory_parameters_and_media() {
        let request: BidRequest = serde_json::from_value(json!({
            "id":"r", "site":{"id":"original"}, "imp":[
                {"id":"first", "banner":{"w":300_i32,"h":250_i32},
                 "ext":{"bidder":{"dcn":"first-site","pos":"first-slot"}}},
                {"id":"second", "video":{"mimes":["video/mp4"]},
                 "ext":{"bidder":{"dcn":"second-site","pos":"second-slot"}}}
            ]
        }))
        .unwrap();
        let before = serde_json::to_value(&request).unwrap();
        let calls = harness::drive_ok(&YahooAdsBidder, &request, &info());
        assert_eq!(calls.len(), 2);
        let expected = [
            (
                "first",
                CorrelationMedia::Banner,
                json!({
                    "id":"r", "site":{"id":"first-site"}, "imp":[
                        {"id":"first", "tagid":"first-slot", "banner":{"w":300_i32,"h":250_i32},
                         "ext":{"bidder":{"dcn":"first-site","pos":"first-slot"}}}
                    ]
                }),
            ),
            (
                "second",
                CorrelationMedia::Video,
                json!({
                    "id":"r", "site":{"id":"second-site"}, "imp":[
                        {"id":"second", "tagid":"second-slot", "video":{"mimes":["video/mp4"]},
                         "ext":{"bidder":{"dcn":"second-site","pos":"second-slot"}}}
                    ]
                }),
            ),
        ];
        for (call, (id, media, body)) in calls.iter().zip(expected) {
            assert_eq!(call.covered_imp_ids().collect::<Vec<_>>(), [id]);
            assert_eq!(call.meta().imp_correlation(0).unwrap().media(), media);
            assert_eq!(call.meta().imp_correlation(1), None);
            assert_eq!(serde_json::from_slice::<Value>(call.body()).unwrap(), body);
        }
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn impression_sibling_refusal_does_not_revoke_an_accepted_call() {
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &YahooAdsBidder,
            &request(json!([banner("first"), banner("second")])),
            &info(),
            SinkLimits::new(4096, 1),
            1,
            &EndpointPolicy(true),
        );
        semantic.unwrap();
        assert_eq!(outcome.accepted.len(), 1);
        assert_eq!(
            outcome.accepted[0].covered_imp_ids().collect::<Vec<_>>(),
            ["first"]
        );
        assert_eq!(
            outcome.accepted[0]
                .meta()
                .imp_correlation(0)
                .unwrap()
                .media(),
            CorrelationMedia::Banner
        );
        assert_eq!(
            outcome.errors,
            [OutboundBuildError::CallBudgetExhausted { max_calls: 1 }]
        );
        assert_eq!(outcome.rejected_call_count, 1);
        assert_eq!(remaining, 0);
    }

    #[test]
    fn exact_body_cap_accepts_and_one_byte_under_refunds_the_call() {
        let expected = json!({"id":"r","imp":[{
            "id":"i","tagid":"position","banner":{"format":[{"w":728_i32,"h":90_i32}],"w":728_i32,"h":90_i32},
            "ext":{"bidder":{"dcn":"site","pos":"position"}}
        }]});
        let length = serde_json::to_vec(&expected).unwrap().len();
        for (cap, accepted, remaining) in [(length, 1, 0), (length - 1, 0, 1)] {
            let (semantic, outcome, actual_remaining) = harness::drive_with_limits(
                &YahooAdsBidder,
                &request(json!([banner("i")])),
                &info(),
                SinkLimits::new(cap, 1),
                1,
                &EndpointPolicy(true),
            );
            semantic.unwrap();
            assert_eq!(outcome.accepted.len(), accepted);
            assert_eq!(actual_remaining, remaining);
            if accepted == 1 {
                assert!(outcome.errors.is_empty());
                assert_eq!(outcome.accepted[0].body().len(), length);
                assert_eq!(
                    serde_json::from_slice::<Value>(outcome.accepted[0].body()).unwrap(),
                    expected
                );
            } else {
                assert_eq!(
                    outcome.errors,
                    [OutboundBuildError::OversizeRaw {
                        limit: u64::try_from(cap).unwrap()
                    }]
                );
            }
        }
    }

    #[test]
    fn endpoint_policy_refusal_refunds_each_impression_call_without_a_semantic_abort() {
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &YahooAdsBidder,
            &request(json!([banner("first"), banner("second")])),
            &info(),
            SinkLimits::new(4096, 1),
            1,
            &EndpointPolicy(false),
        );
        semantic.unwrap();
        assert!(outcome.accepted.is_empty());
        assert_eq!(
            outcome.errors,
            vec![
                OutboundBuildError::EndpointRejected {
                    reason: EndpointRejectReason::HostNotAllowed
                };
                2
            ]
        );
        assert_eq!(remaining, 1);
    }

    #[test]
    fn empty_endpoint_is_not_replaced_with_a_dummy_or_source_default_route() {
        let mut info = info();
        info.endpoint_template = "";
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &YahooAdsBidder,
            &request(json!([banner("i")])),
            &info,
            SinkLimits::new(4096, 1),
            1,
            &EndpointPolicy(true),
        );
        semantic.unwrap();
        assert!(outcome.accepted.is_empty());
        assert_eq!(outcome.errors, [OutboundBuildError::MissingUrl]);
        assert_eq!(remaining, 1);
    }
}
