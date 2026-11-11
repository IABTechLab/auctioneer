use super::parameters::{
    Impression, ImpressionExtension, boolean, integer, object, prepare, unique,
};
use crate::bidders::url::{json_utf8_headers, write_endpoint};
use crate::{
    BidderError, BidderRequestInfo, EndpointBuilder, EntryPoint, OutboundRequestMetaBuilder,
    OutboundRequestSink, SinkAbort,
};
use crate::{BidderSettings, bidder_settings::msft};
use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, BidRequestExt, Imp};
use auctioneer_runtime::bounded_json::JsonFieldShape;
use serde::{
    Deserialize, Serialize, Serializer,
    ser::{Error as _, SerializeMap as _, SerializeSeq as _},
};
use serde_json::Value;
use std::{borrow::Cow, cell::OnceCell, fmt, ops::Deref};

pub(super) fn make_requests(
    request: &BidRequest,
    info: &BidderRequestInfo<'_>,
    sink: &mut OutboundRequestSink<'_>,
) -> Result<(), Vec<BidderError>> {
    let mut count = 0_usize;
    let mut member = 0_i64;
    for imp in &request.imp {
        match prepare(imp) {
            Ok(prepared) => {
                if prepared.member != 0 {
                    if member != 0 && member != prepared.member {
                        return Err(vec![member_mismatch()]);
                    }
                    member = prepared.member;
                }
                count = count.saturating_add(1);
            }
            Err(reason) => sink.diagnostic(reason),
        }
    }
    if count == 0 {
        return Ok(());
    }
    match info.original_request_ext_shape {
        JsonFieldShape::Absent | JsonFieldShape::Object => {}
        JsonFieldShape::Unavailable if request.ext.is_some() => {}
        JsonFieldShape::Unavailable
        | JsonFieldShape::Null
        | JsonFieldShape::NonObject
        | JsonFieldShape::Duplicate => return Err(vec![request_ext_error()]),
    }
    let namespace = request_namespace(request, info).map_err(|error| vec![error])?;
    let sdk_cache = OnceCell::new();
    let mut start = 0_usize;
    while start < request.imp.len() {
        let mut end = start;
        let mut chunk_count = 0_usize;
        while end < request.imp.len() && chunk_count < 10 {
            let imp = request
                .imp
                .get(end)
                .ok_or_else(|| vec![BidderError::unknown("invalid msft impression chunk")])?;
            if prepare(imp).is_ok() {
                chunk_count = chunk_count.saturating_add(1);
            }
            end = end.saturating_add(1);
        }
        if chunk_count == 0 {
            break;
        }
        let imps = request
            .imp
            .get(start..end)
            .ok_or_else(|| vec![BidderError::unknown("invalid msft impression chunk")])?;
        let view = Request {
            input: request,
            imps,
            count: chunk_count,
            namespace,
            sdk_cache: &sdk_cache,
        };
        let _recorded = sink.write_json(
            info,
            |meta| {
                meta.set_method("POST")?;
                request_endpoint(meta, info, member)?;
                json_utf8_headers(meta)?;
                meta.header("x-openrtb-version", "2.6")?;
                for imp in survivors(view.imps) {
                    meta.imp_id(&imp.imp.id)?;
                }
                Ok(())
            },
            &view,
        );
        start = end;
    }
    Ok(())
}

pub(super) fn member_mismatch() -> BidderError {
    BidderError::unknown("member id mismatch: all impressions must use the same member id")
}
pub(super) fn request_ext_error() -> BidderError {
    BidderError::unknown("malformed request ext.appnexus")
}

fn survivors(imps: &[Imp]) -> impl Iterator<Item = Impression<'_>> {
    imps.iter().filter_map(|imp| prepare(imp).ok())
}

#[derive(Clone, Copy, Serialize)]
struct RequestNamespace {
    #[serde(skip_serializing_if = "Option::is_none")]
    include_brand_category: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    brand_category_uniqueness: Option<bool>,
    #[serde(skip_serializing_if = "is_zero")]
    is_amp: i64,
    hb_source: i64,
}
#[cfg_attr(
    target_pointer_width = "64",
    expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "serde requires fn(&T) -> bool for skip_serializing_if"
    )
)]
fn is_zero(value: &i64) -> bool {
    *value == 0
}
fn request_namespace(
    request: &BidRequest,
    info: &BidderRequestInfo<'_>,
) -> Result<RequestNamespace, BidderError> {
    let inner = || -> Result<RequestNamespace, ()> {
        let fields = object(
            request
                .ext
                .as_ref()
                .and_then(|ext| ext.other.get("appnexus")),
        )?;
        let get = |name| {
            fields
                .map(|namespace| unique(namespace, name))
                .transpose()
                .map(Option::flatten)
        };
        let mut output = RequestNamespace {
            include_brand_category: boolean(get("include_brand_category")?)?,
            brand_category_uniqueness: boolean(get("brand_category_uniqueness")?)?,
            is_amp: integer(get("is_amp")?)?,
            hb_source: integer(get("hb_source")?)?,
        };
        if request
            .ext
            .as_ref()
            .and_then(|ext| ext.prebid.as_ref())
            .and_then(|prebid| prebid.targeting.as_ref())
            .and_then(|target| target.other.get("includebrandcategory"))
            .is_some_and(Value::is_object)
        {
            output.include_brand_category = Some(true);
            output.brand_category_uniqueness = Some(true);
        }
        let settings = msft::resolve(
            info.settings().and_then(BidderSettings::as_msft),
            info.base_settings().and_then(BidderSettings::as_msft),
        );
        let configured_source = if info.entry_point == EntryPoint::Video {
            settings.hb_source_video()
        } else {
            settings.hb_source()
        };
        let default = if info.entry_point == EntryPoint::Video {
            6
        } else {
            5
        };
        output.hb_source = configured_source
            .filter(|value| *value != 0)
            .unwrap_or(default);
        if info.entry_point == EntryPoint::Amp {
            output.is_amp = 1;
        }
        Ok(output)
    };
    inner().map_err(|()| request_ext_error())
}

fn request_endpoint(
    meta: &mut OutboundRequestMetaBuilder<'_>,
    info: &BidderRequestInfo<'_>,
    member: i64,
) -> Result<(), SinkAbort> {
    if member == 0 {
        return write_endpoint(meta, info);
    }
    let values = [
        ("AccountID", info.account_id),
        ("PublisherID", info.publisher_id.unwrap_or("")),
    ];
    let count = if info.publisher_id.is_some() { 2 } else { 1 };
    let member_id = member.to_string();
    let provided_values = values
        .get(..count)
        .ok_or(SinkAbort::Body(crate::BodyWriteError::Serialize))?;
    EndpointBuilder::from_request(info, provided_values).expand_with_query_set_into(
        meta,
        "member_id",
        &member_id,
    )
}

#[derive(Deserialize)]
struct AppExtension<'a> {
    #[serde(borrow)]
    prebid: Option<Sdk<'a>>,
}
#[derive(Deserialize)]
struct Sdk<'a> {
    #[serde(borrow)]
    source: Option<SdkString<'a>>,
    #[serde(borrow)]
    version: Option<SdkString<'a>>,
}

#[derive(Deserialize)]
#[serde(transparent)]
struct SdkString<'a>(#[serde(borrow)] Cow<'a, str>);

impl Deref for SdkString<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}
fn sdk(request: &BidRequest) -> Option<Sdk<'_>> {
    let raw = request.app.as_ref()?.ext.as_ref()?;
    let parsed: AppExtension<'_> = serde_json::from_str(raw.get()).ok()?;
    parsed
        .prebid
        .filter(|sdk| sdk.source.is_some() && sdk.version.is_some())
}
struct Joined<'a>(&'a str, &'a str);
impl fmt::Display for Joined<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.0, self.1)
    }
}
impl Serialize for Joined<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

struct Request<'a, 'cache> {
    input: &'a BidRequest,
    imps: &'a [Imp],
    count: usize,
    namespace: RequestNamespace,
    sdk_cache: &'cache OnceCell<Option<Sdk<'a>>>,
}
impl Serialize for Request<'_, '_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let BidRequest {
            id,
            imp: _scoped_impressions,
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
        } = self.input;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", id)?;
        // Forward the containing App before lazily decoding its SDK strings.
        // An over-cap App therefore cannot allocate a derived string first.
        if let Some(value) = app {
            map.serialize_entry("app", value)?;
        }
        map.serialize_entry(
            "imp",
            &Chunk {
                input: self.input,
                imps: self.imps,
                count: self.count,
                sdk_cache: self.sdk_cache,
            },
        )?;
        if let Some(value) = site {
            map.serialize_entry("site", value)?;
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
        map.serialize_entry(
            "ext",
            &RequestExtension {
                existing: ext.as_ref(),
                namespace: self.namespace,
            },
        )?;
        for (key, value) in other {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}
struct RequestExtension<'a> {
    existing: Option<&'a BidRequestExt>,
    namespace: RequestNamespace,
}
impl Serialize for RequestExtension<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        if let Some(ext) = self.existing {
            if let Some(prebid) = &ext.prebid {
                map.serialize_entry("prebid", prebid)?;
            }
            for (key, value) in &ext.other {
                if key != "appnexus" {
                    map.serialize_entry(key, value)?;
                }
            }
        }
        map.serialize_entry("appnexus", &self.namespace)?;
        map.end()
    }
}
struct Chunk<'a, 'cache> {
    input: &'a BidRequest,
    imps: &'a [Imp],
    count: usize,
    sdk_cache: &'cache OnceCell<Option<Sdk<'a>>>,
}
impl Serialize for Chunk<'_, '_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.count))?;
        for impression in survivors(self.imps) {
            let sdk = if impression.version.is_empty() {
                self.sdk_cache.get_or_init(|| sdk(self.input)).as_ref()
            } else {
                None
            };
            sequence.serialize_element(&ImpressionView { impression, sdk })?;
        }
        sequence.end()
    }
}
struct ImpressionView<'a, 'sdk> {
    impression: Impression<'a>,
    sdk: Option<&'sdk Sdk<'a>>,
}
impl Serialize for ImpressionView<'_, '_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let imp = self.impression.imp;
        let Imp {
            id,
            banner,
            video,
            native,
            audio,
            bidfloor,
            bidfloorcur,
            secure,
            ext: _original_ext,
            other,
        } = imp;
        let params = self.impression.params;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", id)?;
        if let Some(banner_input) = banner {
            map.serialize_entry(
                "banner",
                &BannerView {
                    banner: banner_input,
                    api: self.impression.api,
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
        if let Some(value) = bidfloor {
            map.serialize_entry("bidfloor", value)?;
        }
        if let Some(value) = bidfloorcur {
            map.serialize_entry("bidfloorcur", value)?;
        }
        if let Some(value) = secure {
            map.serialize_entry("secure", value)?;
        }
        if !params.inventory.is_empty() {
            map.serialize_entry("tagid", params.inventory)?;
        }
        if !self.impression.version.is_empty() {
            map.serialize_entry("displaymanagerver", self.impression.version)?;
        } else if let Some(sdk) = self.sdk {
            map.serialize_entry(
                "displaymanagerver",
                &Joined(
                    sdk.source.as_deref().unwrap_or(""),
                    sdk.version.as_deref().unwrap_or(""),
                ),
            )?;
        } else {
            // No explicit version or complete SDK projection is available.
        }
        map.serialize_entry("ext", &ImpressionExtension(params))?;
        for (key, value) in other {
            if key != "displaymanagerver" && (key != "tagid" || params.inventory.is_empty()) {
                map.serialize_entry(key, value)?;
            }
        }
        map.end()
    }
}
struct BannerView<'a> {
    banner: &'a Banner,
    api: Option<&'a [Value]>,
}
impl Serialize for BannerView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Banner {
            format,
            w,
            h,
            pos,
            ext,
            other,
        } = self.banner;
        let derived = if w.is_none() && h.is_none() {
            format.first()
        } else {
            None
        };
        let mut map = serializer.serialize_map(None)?;
        if !format.is_empty() {
            map.serialize_entry("format", format)?;
        }
        if let Some(width) = w.or(derived.map(|dimensions| dimensions.w)) {
            map.serialize_entry("w", &width)?;
        }
        if let Some(height) = h.or(derived.map(|dimensions| dimensions.h)) {
            map.serialize_entry("h", &height)?;
        }
        if let Some(value) = pos {
            map.serialize_entry("pos", value)?;
        }
        if let Some(value) = ext {
            map.serialize_entry("ext", value)?;
        }
        if let Some(api) = self.api.filter(|api| !api.is_empty()) {
            map.serialize_entry("api", &Apis(api))?;
        }
        for (key, value) in other {
            if key != "api" {
                map.serialize_entry(key, value)?;
            }
        }
        map.end()
    }
}
struct Apis<'a>(&'a [Value]);
impl Serialize for Apis<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for value in self.0 {
            let number = integer(Some(value)).map_err(|()| S::Error::custom("invalid msft api"))?;
            sequence.serialize_element(&number)?;
        }
        sequence.end()
    }
}

#[cfg(test)]
mod tests {
    use super::super::MsftBidder;
    use super::super::tests::{AllowEndpoints, banner, body, info, request};
    use super::{Request, member_mismatch, request_ext_error, request_namespace, sdk};
    use crate::bidders::harness;
    use crate::{
        BidderRequestInfo, ConstructionDiagnostic, EndpointRejectReason, EndpointValidator,
        EntryPoint, SinkLimits,
    };
    use crate::{BidderSettings, bidder_settings::msft};
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_runtime::bounded_json::JsonFieldShape;
    use serde_json::{Value, json};
    use std::{borrow::Cow, cell::OnceCell};
    use url::Url;

    #[test]
    fn ordinary_sdk_strings_borrow_the_scoped_app_extension() {
        let input: BidRequest = serde_json::from_value(json!({
            "id":"r", "app":{"ext":{"prebid":{"source":"s".repeat(1024), "version":"v".repeat(1024)}}},
        })).unwrap();
        let raw = input
            .app
            .as_ref()
            .unwrap()
            .ext
            .as_ref()
            .unwrap()
            .get()
            .as_bytes();
        let parsed = sdk(&input).unwrap();
        for text in [
            parsed.source.as_deref().unwrap(),
            parsed.version.as_deref().unwrap(),
        ] {
            assert_eq!(text.len(), 1024);
            assert!(raw.as_ptr_range().contains(&text.as_ptr()));
        }
    }

    #[test]
    fn sdk_projection_is_lazy_cached_and_owned_by_one_request_construction() {
        let mut input = request(json!([
            {"id":"one","displaymanagerver":"explicit","banner":{},"ext":{"bidder":{}}},
            {"id":"two","banner":{},"ext":{"bidder":{}}},
        ]));
        input.app = Some(
            serde_json::from_value(json!({"ext":{"prebid":{
                "source":"escaped\nsource", "version":"version\tvalue",
            }}}))
            .unwrap(),
        );
        let cache = OnceCell::new();
        let namespace = request_namespace(&input, &info(EntryPoint::Auction)).unwrap();
        let explicit = Request {
            input: &input,
            imps: &input.imp[..1],
            count: 1,
            namespace,
            sdk_cache: &cache,
        };
        let wire = serde_json::to_value(&explicit).unwrap();
        assert_eq!(wire["imp"][0]["displaymanagerver"], "explicit");
        assert!(
            cache.get().is_none(),
            "explicit versions must not decode SDK"
        );
        let missing = Request {
            input: &input,
            imps: &input.imp[1..],
            count: 1,
            namespace,
            sdk_cache: &cache,
        };
        let first = serde_json::to_value(&missing).unwrap();
        assert_eq!(
            first["imp"][0]["displaymanagerver"],
            "escaped\nsource-version\tvalue"
        );
        let decoded = cache.get().unwrap().as_ref().unwrap();
        assert!(matches!(decoded.source.as_ref().unwrap().0, Cow::Owned(_)));
        let source = decoded.source.as_deref().unwrap().as_ptr();
        let version = decoded.version.as_deref().unwrap().as_ptr();
        assert_eq!(serde_json::to_value(&missing).unwrap(), first);
        let repeated = cache.get().unwrap().as_ref().unwrap();
        assert_eq!(repeated.source.as_deref().unwrap().as_ptr(), source);
        assert_eq!(repeated.version.as_deref().unwrap().as_ptr(), version);
        let later_cache = OnceCell::new();
        assert!(later_cache.get().is_none());
        let later = Request {
            sdk_cache: &later_cache,
            ..missing
        };
        assert_eq!(serde_json::to_value(later).unwrap(), first);
        assert!(later_cache.get().unwrap().is_some());
    }

    #[test]
    fn unsuccessful_sdk_lookup_is_cached_without_creating_a_version() {
        let mut input = request(json!([banner("i", json!({"placement_id":1_i64}))]));
        input.app =
            Some(serde_json::from_value(json!({"ext":{"prebid":{"source":1_i64}}})).unwrap());
        let cache = OnceCell::new();
        let view = Request {
            input: &input,
            imps: &input.imp,
            count: 1,
            namespace: request_namespace(&input, &info(EntryPoint::Auction)).unwrap(),
            sdk_cache: &cache,
        };
        let first = serde_json::to_value(&view).unwrap();
        assert!(first["imp"][0].get("displaymanagerver").is_none());
        assert!(cache.get().unwrap().is_none());
        assert_eq!(serde_json::to_value(&view).unwrap(), first);
    }

    #[test]
    fn relocation_uses_only_the_scoped_input_and_preserves_false_and_original_request() {
        let request: BidRequest = serde_json::from_value(json!({
            "id":"request","user":{"keywords":"scoped"},"tmax":81_i32,
            "imp":[{"id":"i","tagid":"old","banner":{"format":[{"w":300_i32,"h":250_i32}]},
                "ext":{"gpid":"g","opaque":true,"bidder":{
                    "placement_id":7_i32,"member":42_i32,"inv_code":"inventory",
                    "allow_smaller_sizes":false,"use_pmt_rule":false,"keywords":"k=v",
                    "traffic_source_code":"source","pubclick":"https://click.test/",
                    "ext_inv_code":"external","ext_imp_id":"auction","banner_frameworks":[1_i32,2_i32]}}}]
        }))
        .unwrap();
        let before = serde_json::to_value(&request).unwrap();
        let calls = harness::drive_ok(&MsftBidder, &request, &info(EntryPoint::Auction));
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].url(),
            "https://bidder.example.test/openrtb2?member_id=42"
        );
        assert_eq!(calls[0].covered_imp_ids().collect::<Vec<_>>(), ["i"]);
        assert_eq!(
            calls[0].headers().collect::<Vec<_>>(),
            [
                ("Content-Type", "application/json;charset=utf-8"),
                ("Accept", "application/json"),
                ("x-openrtb-version", "2.6")
            ]
        );
        let actual: Value = serde_json::from_slice(calls[0].body()).unwrap();
        assert_eq!(
            actual["imp"][0]["ext"],
            json!({"gpid":"g","appnexus":{
            "placement_id":7_i32,"allow_smaller_sizes":false,"use_pmt_rule":false,
            "keywords":"k=v","traffic_source_code":"source","pub_click":"https://click.test/",
            "ext_inv_code":"external","ext_imp_id":"auction"}})
        );
        assert_eq!(actual["imp"][0]["tagid"], "inventory");
        assert_eq!(actual["imp"][0]["banner"]["api"], json!([1_i32, 2_i32]));
        assert_eq!(actual["user"], before["user"]);
        assert_eq!(actual["tmax"], 81_i32);
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn dimensions_and_api_distinguish_nil_zero_partial_and_explicit_empty_array() {
        for (extra, expected_w, expected_h, expected_api) in [
            (json!({}), Some(300), Some(250), json!([2_i32])),
            (json!({"w":0_i32}), Some(0), None, json!([2_i32])),
            (json!({"h":10_i32}), None, Some(10), json!([2_i32])),
            (
                json!({"w":0_i32,"h":0_i32,"api":[]}),
                Some(0),
                Some(0),
                Value::Null,
            ),
            (json!({"api":null}), Some(300), Some(250), json!([2_i32])),
            (json!({"api":[1_i32]}), Some(300), Some(250), json!([1_i32])),
        ] {
            let mut imp = banner(
                "i",
                json!({"placement_id":1_i32,"banner_frameworks":[2_i32]}),
            );
            imp["banner"]
                .as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            let actual = body(&request(json!([imp])), &info(EntryPoint::Auction));
            let banner = &actual["imp"][0]["banner"];
            assert_eq!(banner["w"].as_i64(), expected_w);
            assert_eq!(banner["h"].as_i64(), expected_h);
            assert_eq!(banner["api"], expected_api);
        }
    }

    #[test]
    fn valid_siblings_survive_bad_parameters_and_member_conflict_precedes_any_call() {
        let request = request(json!([
            banner("bad", json!({"placement_id":"bad","member":999_i32})),
            banner("one", json!({"member":42_i32,"inv_code":"a"})),
            banner("two", json!({"placement_id":2_i32}))
        ]));
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &MsftBidder,
            &request,
            &info(EntryPoint::Auction),
            SinkLimits::new(0x4000, 3),
            2,
            &AllowEndpoints,
        );
        semantic.unwrap();
        assert_eq!(remaining, 1);
        assert_eq!(
            outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
            [ConstructionDiagnostic::InvalidImpressionParameters]
        );
        assert_eq!(
            outcome.accepted[0].covered_imp_ids().collect::<Vec<_>>(),
            ["one", "two"]
        );
        let conflict = super::tests::request(json!([
            banner("one", json!({"member":42_i32,"inv_code":"a"})),
            banner("two", json!({"member":43_i32,"inv_code":"b"}))
        ]));
        let (conflict_semantic, conflict_outcome, conflict_remaining) = harness::drive_with_limits(
            &MsftBidder,
            &conflict,
            &info(EntryPoint::Auction),
            SinkLimits::new(0x4000, 2),
            2,
            &AllowEndpoints,
        );
        assert_eq!(conflict_semantic.unwrap_err(), [member_mismatch()]);
        assert!(conflict_outcome.accepted.is_empty());
        assert_eq!(conflict_remaining, 2);
    }

    #[test]
    fn chunks_have_exact_coverage_and_budget_refusal_does_not_become_semantic_failure() {
        for (count, expected) in [(0_i32, 0), (10_i32, 1), (11_i32, 2), (21_i32, 3)] {
            let request = request(Value::Array(
                (0_i32..count)
                    .map(|i| banner(&format!("i{i}"), json!({"placement_id":1_i32})))
                    .collect(),
            ));
            let calls = harness::drive_ok(&MsftBidder, &request, &info(EntryPoint::Auction));
            assert_eq!(calls.len(), expected);
            let ids: Vec<_> = calls
                .iter()
                .flat_map(crate::BidderHttpRequest::covered_imp_ids)
                .collect();
            assert_eq!(
                ids,
                request
                    .imp
                    .iter()
                    .map(|imp| imp.id.as_str())
                    .collect::<Vec<_>>()
            );
            for call in &calls {
                let wire: Value = serde_json::from_slice(call.body()).unwrap();
                assert_eq!(wire["id"], "request");
                assert!(wire["imp"].as_array().unwrap().len() <= 10);
            }
            if count == 21_i32 {
                let (semantic, outcome, remaining) = harness::drive_with_limits(
                    &MsftBidder,
                    &request,
                    &info(EntryPoint::Auction),
                    SinkLimits::new(0x8000, 21),
                    1,
                    &AllowEndpoints,
                );
                semantic.unwrap();
                assert_eq!(outcome.accepted.len(), 1);
                assert_eq!(outcome.rejected_call_count, 2);
                assert_eq!(remaining, 0);
            }
        }
    }

    #[test]
    fn settings_precedence_is_resolved_before_zero_defaults_and_entrypoint_selection() {
        let requested: BidderSettings = serde_json::from_value(json!({"implementation":"msft",
            "settings":{"hb_source":0_i32,"hb_source_video":null}}))
        .unwrap();
        let base: BidderSettings = serde_json::from_value(json!({"implementation":"msft",
            "settings":{"hb_source":50_i32,"hb_source_video":60_i32}}))
        .unwrap();
        let request = request(json!([banner("i", json!({"placement_id":1_i32}))]));
        // Exercise the generated borrowed view, not a bidder-local settings DTO.
        let resolved = msft::resolve(requested.as_msft(), base.as_msft());
        assert_eq!(resolved.hb_source(), Some(0));
        assert_eq!(resolved.hb_source_video(), Some(60));
        for (entry, expected, amp) in [
            (EntryPoint::Auction, 5_i32, Value::Null),
            (EntryPoint::Amp, 5_i32, json!(1_i32)),
            (EntryPoint::Video, 60_i32, Value::Null),
        ] {
            let mut info = info(entry);
            info.set_settings(Some("msft"), Some(&requested), Some(&base))
                .unwrap();
            let actual = body(&request, &info);
            assert_eq!(actual["ext"]["appnexus"]["hb_source"], expected);
            assert_eq!(actual["ext"]["appnexus"]["is_amp"], amp);
        }
        for (entry, expected) in [
            (EntryPoint::Auction, 5_i32),
            (EntryPoint::Amp, 5_i32),
            (EntryPoint::Video, 6_i32),
        ] {
            assert_eq!(
                body(&request, &info(entry))["ext"]["appnexus"]["hb_source"],
                expected
            );
            let mut info = info(entry);
            info.set_settings(Some("msft"), None, Some(&base)).unwrap();
            assert_eq!(
                body(&request, &info)["ext"]["appnexus"]["hb_source"],
                if entry == EntryPoint::Video {
                    60_i32
                } else {
                    50_i32
                }
            );
        }
        for (field, entry, default) in [
            ("hb_source", EntryPoint::Auction, 5),
            ("hb_source_video", EntryPoint::Video, 6),
        ] {
            for value in [i64::MIN, -1, 0, i64::MAX] {
                let settings: BidderSettings =
                    serde_json::from_value(json!({"implementation":"msft",
                    "settings":{(field):value}}))
                    .unwrap();
                let mut info = info(entry);
                info.set_settings(Some("msft"), Some(&settings), None)
                    .unwrap();
                assert_eq!(
                    body(&request, &info)["ext"]["appnexus"]["hb_source"],
                    if value == 0 { default } else { value }
                );
            }
        }
    }

    #[test]
    fn request_extension_flags_are_exact_typed_and_other_root_namespaces_survive() {
        let request: BidRequest = serde_json::from_value(json!({"id":"r","imp":[banner("i",
            json!({"placement_id":1_i32}))],"ext":{"opaque":{"keep":true},
            "APPNEXUS":{"opaque":"distinct map key"},"appnexus":{"include_brand_category":false,
                "brand_category_uniqueness":false,"is_amp":9_i32,"hb_source":99_i32,"discard":true},
            "prebid":{"targeting":{"includebrandcategory":{}}}}}))
        .unwrap();
        let actual = body(&request, &info(EntryPoint::Auction));
        assert_eq!(
            actual["ext"]["appnexus"],
            json!({"include_brand_category":true,
            "brand_category_uniqueness":true,"is_amp":9_i32,"hb_source":5_i32})
        );
        assert_eq!(
            actual["ext"]["APPNEXUS"],
            json!({"opaque":"distinct map key"})
        );
        assert_eq!(actual["ext"]["opaque"], json!({"keep":true}));
    }

    #[test]
    fn original_extension_null_is_not_mistaken_for_absence() {
        let request = request(json!([banner("i", json!({"placement_id":1_i32}))]));
        for shape in [
            JsonFieldShape::Null,
            JsonFieldShape::NonObject,
            JsonFieldShape::Duplicate,
            JsonFieldShape::Unavailable,
        ] {
            let mut info = info(EntryPoint::Auction);
            info.original_request_ext_shape = shape;
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &MsftBidder,
                &request,
                &info,
                SinkLimits::new(4096, 1),
                1,
                &AllowEndpoints,
            );
            assert_eq!(semantic.unwrap_err(), [request_ext_error()]);
            assert!(outcome.accepted.is_empty());
            assert_eq!(remaining, 1);
        }
    }

    #[test]
    fn sdk_version_uses_both_strings_even_empty_and_never_replaces_an_explicit_version() {
        for (sdk, expected) in [
            (json!({"source":"mobile","version":"1"}), "mobile-1"),
            (json!({"source":"","version":""}), "-"),
            (json!({"source":"mobile"}), ""),
            (json!({"source":false,"version":"1"}), ""),
        ] {
            let request: BidRequest = serde_json::from_value(json!({"id":"r","app":{"ext":{
                "prebid":sdk}},"imp":[banner("i",json!({"placement_id":1_i32})),
                {"id":"kept","banner":{},"displaymanagerver":"explicit","ext":{"bidder":{}}}]}))
            .unwrap();
            let actual = body(&request, &info(EntryPoint::Auction));
            assert_eq!(
                actual["imp"][0]["displaymanagerver"].as_str().unwrap_or(""),
                expected
            );
            assert_eq!(actual["imp"][1]["displaymanagerver"], "explicit");
        }
    }

    #[test]
    fn literal_member_query_is_replaced_through_the_shared_setter() {
        let request = request(json!([banner(
            "i",
            json!({"member":42_i32,"inv_code":"a"})
        )]));
        let mut info = info(EntryPoint::Auction);
        info.endpoint_template = "https://bidder.example.test/?member_id=old&x=1";
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &MsftBidder,
            &request,
            &info,
            SinkLimits::new(4096, 1),
            1,
            &AllowEndpoints,
        );
        semantic.unwrap();
        assert_eq!(outcome.accepted.len(), 1);
        assert_eq!(
            outcome.accepted[0].url(),
            "https://bidder.example.test/?x=1&member_id=42"
        );
        assert!(outcome.errors.is_empty());
        assert_eq!(remaining, 0);
    }

    #[test]
    fn encoded_member_query_keys_are_replaced_after_one_decoding_pass() {
        let request = request(json!([banner(
            "i",
            json!({"member":42_i32,"inv_code":"a"})
        )]));
        for endpoint in [
            "https://bidder.example.test/?m%65mber_id=old&x=1",
            "https://bidder.example.test/?member%5fid=old&x=1",
            "https://bidder.example.test/?member%5Fid=old&x=1",
        ] {
            let mut info = info(EntryPoint::Auction);
            info.endpoint_template = endpoint;
            let calls = harness::drive_ok(&MsftBidder, &request, &info);
            assert_eq!(calls.len(), 1);
            assert_eq!(
                calls[0].url(),
                "https://bidder.example.test/?x=1&member_id=42"
            );
        }
    }

    #[test]
    fn repeated_member_query_keys_become_one_effective_binding() {
        let request = request(json!([banner(
            "i",
            json!({"member":42_i32,"inv_code":"a"})
        )]));
        let mut info = info(EntryPoint::Auction);
        info.endpoint_template =
            "https://bidder.example.test/?member_id=old&x=1&m%65mber_id=other&member_id";
        let calls = harness::drive_ok(&MsftBidder, &request, &info);
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].url(),
            "https://bidder.example.test/?x=1&member_id=42"
        );
    }

    #[test]
    fn macro_produced_member_query_keys_are_replaced_after_expansion() {
        let request = request(json!([banner(
            "i",
            json!({"member":42_i32,"inv_code":"a"})
        )]));
        for (endpoint, account_id, publisher_id) in [
            (
                "https://bidder.example.test/?{{.AccountID}}=old&x=1",
                "member_id",
                None,
            ),
            (
                "https://bidder.example.test/?{{.PublisherID}}=old&x=1",
                "account",
                Some("member_id"),
            ),
            (
                "https://bidder.example.test/?%{{.AccountID}}=old&x=1",
                "6dember_id",
                None,
            ),
        ] {
            let mut info = info(EntryPoint::Auction);
            info.endpoint_template = endpoint;
            info.endpoint_macros = &["AccountID", "PublisherID"];
            info.account_id = account_id;
            info.publisher_id = publisher_id;
            let calls = harness::drive_ok(&MsftBidder, &request, &info);
            assert_eq!(calls.len(), 1);
            assert_eq!(
                calls[0].url(),
                "https://bidder.example.test/?x=1&member_id=42"
            );
        }
    }

    #[test]
    fn member_replacement_preserves_unrelated_query_pairs_and_their_encoding() {
        let request = request(json!([banner(
            "i",
            json!({"member":42_i32,"inv_code":"a"})
        )]));
        let mut info = info(EntryPoint::Auction);
        info.endpoint_template = "https://bidder.example.test/?z=a%2Fb&member_id=old&x=a+b&x=a%20b&flag&blank=&Member_id=case&member%255fid=twice";
        let calls = harness::drive_ok(&MsftBidder, &request, &info);
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].url(),
            "https://bidder.example.test/?z=a%2Fb&x=a+b&x=a%20b&flag&blank=&Member_id=case&member%255fid=twice&member_id=42"
        );
    }

    #[test]
    fn zero_or_absent_member_leaves_the_existing_query_untouched() {
        for params in [
            json!({"placement_id":1_i32}),
            json!({"member":0_i32,"inv_code":"a"}),
        ] {
            let request = request(json!([banner("i", params)]));
            for endpoint in [
                "https://bidder.example.test/",
                "https://bidder.example.test/?member_id=old&m%65mber_id=other&x=a%2Fb",
            ] {
                let mut info = info(EntryPoint::Auction);
                info.endpoint_template = endpoint;
                let calls = harness::drive_ok(&MsftBidder, &request, &info);
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].url(), endpoint);
            }
        }
    }

    #[test]
    fn nonzero_member_uses_the_full_source_signed_integer_domain() {
        for (member, expected) in [
            (
                i64::MIN,
                "https://bidder.example.test/?member_id=-9223372036854775808",
            ),
            (-42, "https://bidder.example.test/?member_id=-42"),
            (1, "https://bidder.example.test/?member_id=1"),
            (
                i64::MAX,
                "https://bidder.example.test/?member_id=9223372036854775807",
            ),
        ] {
            let request = request(json!([banner(
                "i",
                json!({"member":member,"inv_code":"a"})
            )]));
            let mut info = info(EntryPoint::Auction);
            info.endpoint_template = "https://bidder.example.test/?member_id=old";
            let calls = harness::drive_ok(&MsftBidder, &request, &info);
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].url(), expected);
        }
    }

    #[test]
    fn raw_call_and_endpoint_refusals_remain_sink_owned_and_refund_the_reserved_slot() {
        let request = request(json!([banner("i", json!({"placement_id":1_i32}))]));
        let expected = json!({"id":"request","tmax":81_i32,"ext":{"appnexus":{"hb_source":5_i32}},
            "imp":[{"id":"i","banner":{"format":[{"w":300_i32,"h":250_i32}],"w":300_i32,"h":250_i32},
                "ext":{"appnexus":{"placement_id":1_i32}}}]});
        let exact = serde_json::to_vec(&expected).unwrap().len();
        for limit in [exact.saturating_sub(1), exact] {
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &MsftBidder,
                &request,
                &info(EntryPoint::Auction),
                SinkLimits::new(limit, 1),
                1,
                &AllowEndpoints,
            );
            semantic.unwrap();
            if limit == exact {
                assert_eq!(outcome.accepted.len(), 1);
                assert!(outcome.errors.is_empty());
                assert_eq!(remaining, 0);
            } else {
                assert!(outcome.accepted.is_empty());
                assert_eq!(
                    outcome.errors,
                    [crate::OutboundBuildError::OversizeRaw {
                        limit: u64::try_from(limit).unwrap()
                    }]
                );
                assert_eq!(remaining, 1);
            }
        }
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &MsftBidder,
            &request,
            &info(EntryPoint::Auction),
            SinkLimits::new(0, 1),
            0,
            &AllowEndpoints,
        );
        semantic.unwrap();
        assert!(outcome.accepted.is_empty());
        assert_eq!(
            outcome.errors,
            [crate::OutboundBuildError::CallBudgetExhausted { max_calls: 0 }]
        );
        assert_eq!(remaining, 0);
        let (denied_semantic, denied_outcome, denied_remaining) = harness::drive_with_limits(
            &MsftBidder,
            &request,
            &info(EntryPoint::Auction),
            SinkLimits::new(0, 1),
            1,
            &DenyEndpoints,
        );
        denied_semantic.unwrap();
        assert!(denied_outcome.accepted.is_empty());
        assert_eq!(
            denied_outcome.errors,
            [crate::OutboundBuildError::EndpointRejected {
                reason: EndpointRejectReason::HostNotAllowed
            }]
        );
        assert_eq!(denied_remaining, 1);
    }

    #[test]
    fn malformed_overwritten_request_fields_are_fatal_before_any_chunk_is_reserved() {
        let request:BidRequest=serde_json::from_value(json!({"id":"r","ext":{"appnexus":{
            "hb_source":"invalid even though overwritten"}},"imp":[banner("i",json!({"placement_id":1_i32}))]})).unwrap();
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &MsftBidder,
            &request,
            &info(EntryPoint::Auction),
            SinkLimits::new(4096, 1),
            1,
            &AllowEndpoints,
        );
        assert_eq!(semantic.unwrap_err(), [request_ext_error()]);
        assert!(outcome.accepted.is_empty());
        assert!(outcome.errors.is_empty());
        assert_eq!(remaining, 1);
    }

    #[test]
    fn video_inventory_does_not_override_auction_context_and_other_query_values_survive() {
        let request = request(json!([{"id":"i","video":{"mimes":["video/mp4"]},
            "ext":{"bidder":{"member":-42_i32,"inv_code":"a"}}}]));
        let mut info = info(EntryPoint::Auction);
        info.endpoint_template = "https://bidder.example.test/path?x=a%2Fb";
        let calls = harness::drive_ok(&MsftBidder, &request, &info);
        assert_eq!(
            calls[0].url(),
            "https://bidder.example.test/path?x=a%2Fb&member_id=-42"
        );
        let wire: Value = serde_json::from_slice(calls[0].body()).unwrap();
        assert_eq!(wire["ext"]["appnexus"]["hb_source"], 5_i32);
        assert!(wire["imp"][0].get("video").is_some());
    }

    struct DenyEndpoints;
    impl EndpointValidator for DenyEndpoints {
        fn validate(
            &self,
            _url: &Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            Err(EndpointRejectReason::HostNotAllowed)
        }
    }
}
