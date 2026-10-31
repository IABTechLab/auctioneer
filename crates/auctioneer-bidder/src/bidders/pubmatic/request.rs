use super::parameters::{ImpressionWalk, PreparedImpression, RequestExtension, Wrapper};
use crate::bidders::url::{json_utf8_headers, write_endpoint};
use crate::{BidderError, BidderRequestInfo, OutboundRequestSink};
use auctioneer_openrtb::types::{
    bid_request::{Banner, BidRequest, Imp},
    inventory::{App, Publisher, Site},
};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{Error as _, MapAccess, Visitor},
    ser::SerializeMap as _,
};
use serde_json::{Value, value::RawValue};
use std::fmt;

pub(super) fn make_requests(
    request: &BidRequest,
    info: &BidderRequestInfo<'_>,
    sink: &mut OutboundRequestSink<'_>,
) -> Result<(), Vec<BidderError>> {
    let initial = RequestExtension::parse(request, info)
        .map_err(|()| vec![BidderError::bad_input("invalid pubmatic request extension")])?;
    let mut walk = ImpressionWalk::new(&request.imp, initial.wrapper);
    let mut publisher = "";
    let mut has_survivor = false;
    for candidate in walk.by_ref() {
        match candidate {
            Ok(prepared) => {
                has_survivor = true;
                if publisher.is_empty() {
                    publisher = prepared.params.publisher.trim();
                }
            }
            Err(reason) => sink.diagnostic(reason),
        }
    }
    if !has_survivor {
        return Err(vec![BidderError::unknown(
            "No valid impressions for pubmatic",
        )]);
    }
    let view = RequestView {
        request,
        publisher,
        initial_wrapper: initial.wrapper,
        extension: initial.with_wrapper(walk.wrapper),
    };
    let _recorded = sink.write_json(
        info,
        |meta| {
            meta.set_method("POST")?;
            write_endpoint(meta, info)?;
            json_utf8_headers(meta)?;
            for prepared in ImpressionWalk::new(&request.imp, initial.wrapper).flatten() {
                meta.imp_id(&prepared.imp.id)?;
            }
            Ok(())
        },
        &view,
    );
    Ok(())
}

#[derive(Default)]
struct SdkFields<'a> {
    source: Option<&'a RawValue>,
    version: Option<&'a RawValue>,
    prebid: Option<&'a RawValue>,
}

#[derive(Clone, Copy)]
struct Sdk<'a> {
    source: &'a RawValue,
    version: &'a RawValue,
}

impl<'a> SdkFields<'a> {
    fn parse(raw: &'a RawValue) -> Option<Self> {
        serde_json::from_str(raw.get()).ok()
    }

    fn pair(&self) -> Option<Sdk<'a>> {
        let source = self
            .source
            .filter(|value| value.get().starts_with('"') && value.get().len() > 2)?;
        let version = self
            .version
            .filter(|value| value.get().starts_with('"') && value.get().len() > 2)?;
        Some(Sdk { source, version })
    }
}

impl<'de> Deserialize<'de> for SdkFields<'de> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(SdkFieldsVisitor)
    }

    fn deserialize_in_place<D: Deserializer<'de>>(
        deserializer: D,
        place: &mut Self,
    ) -> Result<(), D::Error> {
        *place = Self::deserialize(deserializer)?;
        Ok(())
    }
}

struct SdkFieldsVisitor;

#[expect(
    clippy::missing_trait_methods,
    reason = "deserialize_map only visits maps; other shapes must retain Serde's type refusal"
)]
impl<'de> Visitor<'de> for SdkFieldsVisitor {
    type Value = SdkFields<'de>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an SDK metadata object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut fields = SdkFields::default();
        // Raw keys avoid decoding arbitrary escaped names. Keeping one Serde
        // decoder also reuses its structural scratch across all unknown values.
        while let Some(key) = map.next_key::<&RawValue>()? {
            let value = map.next_value::<&RawValue>()?;
            let slot = if raw_sdk_key(key, "source") {
                Some(&mut fields.source)
            } else if raw_sdk_key(key, "version") {
                Some(&mut fields.version)
            } else if raw_sdk_key(key, "prebid") {
                Some(&mut fields.prebid)
            } else {
                None
            };
            if let Some(target) = slot
                && target.replace(value).is_some()
            {
                return Err(A::Error::custom("ambiguous SDK field"));
            }
        }
        Ok(fields)
    }
}

fn sdk(app: Option<&App>) -> Option<Sdk<'_>> {
    let fields = SdkFields::parse(app?.ext.as_ref()?)?;
    fields
        .prebid
        .and_then(SdkFields::parse)
        .and_then(|prebid| prebid.pair())
        .or_else(|| fields.pair())
}

fn raw_sdk_key(raw: &RawValue, name: &str) -> bool {
    let Some(mut encoded) = raw
        .get()
        .strip_prefix('"')
        .and_then(|quoted| quoted.strip_suffix('"'))
    else {
        return false;
    };
    for expected in name.bytes() {
        if let Some(escaped) = encoded.strip_prefix("\\u") {
            let Some(hex) = escaped.get(..4) else {
                return false;
            };
            if u16::from_str_radix(hex, 16).ok() != Some(u16::from(expected)) {
                return false;
            }
            let Some(tail) = escaped.get(4..) else {
                return false;
            };
            encoded = tail;
        } else {
            if encoded.as_bytes().first() != Some(&expected) {
                return false;
            }
            let Some(tail) = encoded.get(1..) else {
                return false;
            };
            encoded = tail;
        }
    }
    encoded.is_empty()
}

struct RequestView<'a> {
    request: &'a BidRequest,
    publisher: &'a str,
    initial_wrapper: Option<Wrapper<'a>>,
    extension: RequestExtension<'a>,
}

impl Serialize for RequestView<'_> {
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
            ext: _original_extension,
            other,
        } = self.request;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", id)?;
        map.serialize_entry(
            "imp",
            &Impressions {
                items: imp,
                wrapper: self.initial_wrapper,
                app: app.as_ref(),
            },
        )?;
        if let Some(site_value) = site {
            map.serialize_entry("site", &SiteView(site_value, self.publisher))?;
        }
        if let Some(app_value) = app {
            if site.is_none() {
                map.serialize_entry("app", &AppView(app_value, self.publisher))?;
            } else {
                map.serialize_entry("app", app_value)?;
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
        map.serialize_entry("ext", &self.extension)?;
        for (key, value) in other {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

struct SiteView<'a>(&'a Site, &'a str);

impl Serialize for SiteView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Site {
            id,
            name,
            domain,
            page,
            publisher,
            ext,
            other,
        } = self.0;
        let mut map = serializer.serialize_map(None)?;
        if let Some(value) = id {
            map.serialize_entry("id", value)?;
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
        map.serialize_entry("publisher", &PublisherView(publisher.as_ref(), self.1))?;
        if let Some(value) = ext {
            map.serialize_entry("ext", value)?;
        }
        for (key, value) in other {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

struct AppView<'a>(&'a App, &'a str);

impl Serialize for AppView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let App {
            id,
            name,
            bundle,
            domain,
            publisher,
            ext,
            other,
        } = self.0;
        let mut map = serializer.serialize_map(None)?;
        if let Some(value) = id {
            map.serialize_entry("id", value)?;
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
        map.serialize_entry("publisher", &PublisherView(publisher.as_ref(), self.1))?;
        if let Some(value) = ext {
            map.serialize_entry("ext", value)?;
        }
        for (key, value) in other {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

struct PublisherView<'a>(Option<&'a Publisher>, &'a str);

impl Serialize for PublisherView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        if !self.1.is_empty() {
            map.serialize_entry("id", self.1)?;
        }
        if let Some(Publisher {
            id: _original_id,
            name,
            domain,
            ext,
            other,
        }) = self.0
        {
            if let Some(value) = name {
                map.serialize_entry("name", value)?;
            }
            if let Some(value) = domain {
                map.serialize_entry("domain", value)?;
            }
            if let Some(value) = ext {
                map.serialize_entry("ext", value)?;
            }
            for (key, value) in other {
                map.serialize_entry(key, value)?;
            }
        }
        map.end()
    }
}

struct Impressions<'a> {
    items: &'a [Imp],
    wrapper: Option<Wrapper<'a>>,
    app: Option<&'a App>,
}

impl Serialize for Impressions<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Raw SDK fields are inspected only after reservation and endpoint validation.
        let sdk = sdk(self.app);
        serializer.collect_seq(
            ImpressionWalk::new(self.items, self.wrapper)
                .flatten()
                .map(|prepared| ImpressionView { prepared, sdk }),
        )
    }
}

struct ImpressionView<'a> {
    prepared: PreparedImpression<'a>,
    sdk: Option<Sdk<'a>>,
}

impl Serialize for ImpressionView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Imp {
            id,
            banner,
            video,
            native,
            audio: _stripped_audio,
            bidfloor: _original_floor,
            bidfloorcur,
            secure,
            ext: _original_extension,
            other,
        } = self.prepared.imp;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", id)?;
        if let Some(value) = banner {
            map.serialize_entry(
                "banner",
                &BannerView {
                    banner: value,
                    dimensions: self.prepared.dimensions,
                },
            )?;
        }
        if let Some(value) = video {
            map.serialize_entry("video", value)?;
        }
        if let Some(value) = native {
            map.serialize_entry("native", value)?;
        }
        if let Some(value) = self.prepared.floor.filter(|value| *value != 0.0_f64) {
            map.serialize_entry("bidfloor", &value)?;
        }
        if let Some(value) = bidfloorcur {
            map.serialize_entry("bidfloorcur", value)?;
        }
        if let Some(value) = secure {
            map.serialize_entry("secure", value)?;
        }
        if !self.prepared.extension.is_empty() {
            map.serialize_entry("ext", &self.prepared.extension)?;
        }
        if let Some(tag) = self.prepared.tag.filter(|tag| !tag.is_empty()) {
            map.serialize_entry("tagid", tag)?;
        }
        let sdk = self.sdk.filter(|_| {
            ["displaymanager", "displaymanagerver"].iter().all(|key| {
                other
                    .get(*key)
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty)
            })
        });
        if let Some(pair) = sdk {
            map.serialize_entry("displaymanager", pair.source)?;
            map.serialize_entry("displaymanagerver", pair.version)?;
        }
        for (key, value) in other {
            if (key == "tagid" && self.prepared.tag.is_some())
                || (sdk.is_some() && (key == "displaymanager" || key == "displaymanagerver"))
            {
                continue;
            }
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

struct BannerView<'a> {
    banner: &'a Banner,
    dimensions: Option<(i32, i32)>,
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
        if let Some((w, h)) = self.dimensions {
            map.serialize_entry("w", &w)?;
            map.serialize_entry("h", &h)?;
        }
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

#[cfg(test)]
mod tests {
    use super::super::tests::{banner, body, drive, info, input};
    use crate::ConstructionDiagnostic;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};

    #[test]
    fn request_aggregates_survivors_strips_audio_and_uses_source_headers() {
        let request = input(&json!([
            {"id":"audio-only", "audio":{}, "ext":{"bidder":{"publisherId":"bad"}}},
            {"id":"banner", "banner":{"w":300_i32,"h":250_i32}, "audio":{},
                "ext":{"bidder":{"publisherId":"p"}}},
            {"id":"video", "video":{"w":640_i32,"h":480_i32}, "audio":{},
                "ext":{"bidder":{"publisherId":"p"}}},
            {"id":"native", "native":{"request":"{\"native\":{}}"}, "audio":{},
                "ext":{"bidder":{"publisherId":"p"}}},
        ]));
        let before = json!(&request);
        let (semantic, outcome, remaining) = drive(&request);
        semantic.unwrap();
        assert_eq!(remaining, 0);
        assert!(outcome.errors.is_empty());
        assert_eq!(outcome.accepted.len(), 1);
        assert_eq!(
            outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
            [ConstructionDiagnostic::UnsupportedMediaType]
        );
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
            ["banner", "video", "native"]
        );
        let body: Value = serde_json::from_slice(call.body()).unwrap();
        let impressions = body["imp"].as_array().unwrap();
        assert_eq!(impressions.len(), 3);
        for imp in impressions {
            assert!(imp.get("audio").is_none());
            assert!(imp.get("ext").is_none());
        }
        assert_eq!(body["imp"][0]["banner"], before["imp"][1]["banner"]);
        assert_eq!(body["imp"][1]["video"], before["imp"][2]["video"]);
        assert_eq!(body["imp"][2]["native"], before["imp"][3]["native"]);
        assert_eq!(json!(&request), before);
    }

    #[test]
    fn request_does_not_chunk_at_the_unused_thirty_impression_constant() {
        let impressions: Vec<_> = (0_u32..31)
            .map(|index| banner(&format!("i-{index}"), &json!({"publisherId":"p"})))
            .collect();
        let request = input(&json!(impressions));
        let (semantic, outcome, remaining) = drive(&request);
        semantic.unwrap();
        assert_eq!(remaining, 0);
        assert!(outcome.errors.is_empty());
        assert_eq!(outcome.accepted.len(), 1);
        assert_eq!(
            outcome.accepted[0].covered_imp_ids().collect::<Vec<_>>(),
            request
                .imp
                .iter()
                .map(|imp| imp.id.as_str())
                .collect::<Vec<_>>()
        );
        let body: Value = serde_json::from_slice(outcome.accepted[0].body()).unwrap();
        assert_eq!(body["imp"].as_array().unwrap().len(), 31);
        assert_eq!(body["imp"][30]["banner"]["w"], json!(300_i32));
    }

    #[test]
    fn request_all_invalid_impressions_leave_the_call_budget_untouched() {
        for impressions in [
            json!([]),
            json!([{"id":"audio", "audio":{}, "ext":{"bidder":{}}}]),
            json!([banner("bad", &json!({"adSlot":"slot@badx250"}))]),
        ] {
            let request = input(&impressions);
            let (semantic, outcome, remaining) = drive(&request);
            if !request.imp.is_empty() {
                assert!(semantic.is_err());
            }
            assert!(outcome.accepted.is_empty());
            assert!(outcome.errors.is_empty());
            assert_eq!(remaining, 1);
        }
    }

    #[test]
    fn request_uses_first_nonempty_trimmed_survivor_publisher_on_site_before_app() {
        let mut rejected = banner(
            "invalid",
            &json!({"publisherId":"never-use","wrapper":{"profile":99_i32}}),
        );
        rejected["ext"]["bidder"]["adSlot"] = json!("broken@");
        let request: BidRequest = serde_json::from_value(json!({
            "id":"r", "imp":[
                rejected,
                banner("empty", &json!({"publisherId":" \t "})),
                banner("first", &json!({"publisherId":"  first-publisher \n"})),
                banner("later", &json!({"publisherId":"later-publisher"})),
            ],
            "site":{"id":"site","publisher":{"id":"old-site","name":"retained","domain":"example.test","ext":{"keep":true}}},
            "app":{"id":"app","publisher":{"id":"old-app","name":"app-retained"}},
        })).unwrap();
        let before = json!(&request);
        let body = body(&request);
        assert_eq!(
            body["site"]["publisher"],
            json!({"id":"first-publisher","name":"retained","domain":"example.test","ext":{"keep":true}})
        );
        assert_eq!(body["app"], before["app"]);
        assert_eq!(body["imp"].as_array().unwrap().len(), 3);
        assert_eq!(json!(&request), before);
    }

    #[test]
    fn request_adds_publisher_to_existing_inventory_only() {
        for inventory in ["site", "app"] {
            let mut value = json!({"id":"r","imp":[banner("i",&json!({"publisherId":" p "}))]});
            value[inventory] = json!({"id":"inventory"});
            let body = body(&serde_json::from_value(value).unwrap());
            assert_eq!(body[inventory]["publisher"]["id"], json!("p"));
        }
        let body = body(&input(&json!([banner("i", &json!({"publisherId":"p"}))])));
        assert!(body.get("site").is_none());
        assert!(body.get("app").is_none());
    }

    #[test]
    fn request_sdk_uses_complete_prebid_pair_then_complete_flat_pair() {
        for (extension, expected) in [
            (
                json!({"prebid":{"source":"prebid-mobile","version":"1.0"},"source":"flat","version":"2.0"}),
                Some(("prebid-mobile", "1.0")),
            ),
            (
                json!({"prebid":{"source":"incomplete"},"source":"flat","version":"2.0"}),
                Some(("flat", "2.0")),
            ),
            (
                json!({"source":"flat","version":"2.0"}),
                Some(("flat", "2.0")),
            ),
            (
                json!({"prebid":{"source":"nested"},"version":"do-not-mix"}),
                None,
            ),
            (json!({"source":"","version":"2.0"}), None),
        ] {
            let request: BidRequest = serde_json::from_value(json!({
                "id":"r","app":{"id":"app","ext":extension},"imp":[
                    banner("empty",&json!({"publisherId":"p"})),
                    {"id":"existing","video":{},"displaymanager":"existing","displaymanagerver":"9.0","ext":{"bidder":{}}},
                    {"id":"partial","native":{},"displaymanagerver":"7.0","ext":{"bidder":{}}},
                ],
            })).unwrap();
            let body = body(&request);
            if let Some((source, version)) = expected {
                assert_eq!(body["imp"][0]["displaymanager"], json!(source));
                assert_eq!(body["imp"][0]["displaymanagerver"], json!(version));
            } else {
                assert!(body["imp"][0].get("displaymanager").is_none());
                assert!(body["imp"][0].get("displaymanagerver").is_none());
            }
            assert_eq!(body["imp"][1]["displaymanager"], json!("existing"));
            assert_eq!(body["imp"][1]["displaymanagerver"], json!("9.0"));
            assert!(body["imp"][2].get("displaymanager").is_none());
            assert_eq!(body["imp"][2]["displaymanagerver"], json!("7.0"));
        }
    }

    #[test]
    fn request_borrowed_view_preserves_unmodified_openrtb_fields_and_input() {
        let request: BidRequest = serde_json::from_value(json!({
            "id":"r", "imp":[{
                "id":"i","banner":{"w":300_i32,"h":250_i32,"pos":1_i32,"custom":[true,null]},
                "bidfloor":0.75_f64,"bidfloorcur":"EUR","secure":1_i32,"tagid":"old","custom":{"keep":true},
                "ext":{"bidder":{"publisherId":" p ","adSlot":"new"}},
            }],
            "site":{"id":"site","publisher":{"id":"old","name":"kept"}},
            "app":{"id":"app"},"dooh":{"id":"dooh"},"device":{"make":"already-scoped"},
            "user":{"id":"already-scoped"},"regs":{"coppa":1_i32},"source":{"tid":"already-scoped"},
            "at":2_i32,"tmax":150_i64,"cur":["EUR"],"test":1_i32,"ext":{"drop":"source-replaces-root-ext"},"custom":[false,null],
        })).unwrap();
        let before = json!(&request);
        let mut expected = before.clone();
        expected["ext"] = json!({"wrapper":{"biddercode":"pubmatic-requested"}});
        expected["site"]["publisher"]["id"] = json!("p");
        expected["imp"][0]["tagid"] = json!("new");
        expected["imp"][0].as_object_mut().unwrap().remove("ext");
        assert_eq!(body(&request), expected);
        assert_eq!(json!(&request), before);
    }
}
