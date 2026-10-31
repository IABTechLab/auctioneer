use crate::bidder::BidderError;
use crate::bidders::url::json_headers;
use crate::diagnostics::ConstructionDiagnostic;
use crate::endpoint_builder::EndpointBuilder;
use crate::meta::{META_BIDDER_REGION_BYTES, OutboundRequestMetaBuilder, SinkAbort};
use crate::request_info::BidderRequestInfo;
use crate::sink::OutboundRequestSink;
use auctioneer_openrtb::types::{
    bid_request::{Banner, BidRequest, BidRequestExt, Imp},
    device::{Device, Geo},
    field::JsonField,
    user::User,
};
use auctioneer_runtime::bounded_json::JsonFieldShape;
use serde::{
    Serialize, Serializer,
    ser::{SerializeMap, SerializeSeq as _},
};

pub(super) fn make_requests(
    request: &BidRequest,
    info: &BidderRequestInfo<'_>,
    sink: &mut OutboundRequestSink<'_>,
) -> Result<(), Vec<BidderError>> {
    match info.original_request_ext_shape {
        JsonFieldShape::Absent | JsonFieldShape::Object => {}
        // A typed Some proves an object can be safely extended. None
        // cannot distinguish absent wire input from normalized JSON null.
        JsonFieldShape::Unavailable if request.ext.is_some() => {}
        JsonFieldShape::Unavailable
        | JsonFieldShape::Null
        | JsonFieldShape::NonObject
        | JsonFieldShape::Duplicate => {
            return Err(vec![BidderError::bad_input(
                "aps: original request extension must be absent or an object",
            )]);
        }
    }

    let mut selected: Option<Parameters<'_>> = None;
    for imp in &request.imp {
        let Some(params) = parse_imp_params(imp) else {
            sink.diagnostic(ConstructionDiagnostic::InvalidImpressionParameters);
            continue;
        };
        if selected
            .as_ref()
            .is_some_and(|previous| previous.account != params.account)
        {
            return Err(vec![BidderError::bad_input(
                "aps: all valid impressions must use the same accountID",
            )]);
        }
        // The last VALID impression wins, including a missing/blank
        // region resetting a preceding non-default region to na.
        selected = Some(params);
    }
    let Some(params) = selected else {
        return Ok(());
    };

    let _outcome = sink.write_json(
        info,
        |meta| request_meta(meta, request, info, params.region),
        &ApsRequest::new(request, params.account),
    );
    Ok(())
}

#[derive(Clone, Copy)]
enum Region {
    Na,
    Eu,
    Fe,
}

impl Region {
    const fn host(self) -> &'static str {
        match self {
            Self::Na => "s2s.prebid.bid-na.ads.aps.amazon-adsystem.com",
            Self::Eu => "s2s.prebid.bid-eu.ads.aps.amazon-adsystem.com",
            Self::Fe => "s2s.prebid.bid-fe.ads.aps.amazon-adsystem.com",
        }
    }
}

struct Parameters<'a> {
    account: &'a str,
    region: Region,
}

fn parse_imp_params(imp: &Imp) -> Option<Parameters<'_>> {
    let fields = imp.ext.as_ref()?.bidder.as_ref()?.as_object()?;
    let account = fields.get("accountID")?.as_str()?.trim();
    if account.is_empty() {
        return None;
    }
    let region = match fields.get("region") {
        None => Region::Na,
        Some(value) => match value.as_str()?.trim() {
            "" | "na" => Region::Na,
            "eu" => Region::Eu,
            "fe" => Region::Fe,
            _ => return None,
        },
    };
    Some(Parameters { account, region })
}

fn valid_impressions(impressions: &[Imp]) -> impl Iterator<Item = &Imp> {
    impressions
        .iter()
        .filter(|imp| parse_imp_params(imp).is_some())
}

fn request_meta(
    meta: &mut OutboundRequestMetaBuilder<'_>,
    request: &BidRequest,
    info: &BidderRequestInfo<'_>,
    region: Region,
) -> Result<(), SinkAbort> {
    meta.set_method("POST")?;
    // Only enum-owned values reach authority substitution; never accountID
    // or a raw impression parameter. The standard validator reviews origins.
    let mut endpoint = EndpointBuilder::from_request(info, &[]);
    endpoint
        .select_host(region.host())
        .map_err(|_error| endpoint_error())?;
    if request.test == Some(1_i32) {
        // The current endpoint/meta API has no query append operation. Bound
        // its convenience expansion before using Url's structured query API.
        // Only a fixed reviewed host is substituted, not request-sized values.
        // All of this runs after the sink reserves a call.
        if info.endpoint_template.len() > META_BIDDER_REGION_BYTES {
            return Err(endpoint_error());
        }
        let expanded = endpoint.build().map_err(|_error| endpoint_error())?;
        let mut url = url::Url::parse(&expanded).map_err(|_error| endpoint_error())?;
        url.query_pairs_mut().append_pair("amzn_debug_mode", "1");
        meta.set_url(url.as_str())?;
    } else {
        endpoint.expand_into(meta)?;
    }
    json_headers(meta)?;
    for imp in valid_impressions(&request.imp) {
        meta.imp_id(&imp.id)?;
    }
    Ok(())
}

// Match EndpointBuilder::expand_into's existing template-error vocabulary.
fn endpoint_error() -> SinkAbort {
    SinkAbort::MetaOverCap {
        limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap_or(u64::MAX),
    }
}

struct ApsRequest<'a> {
    request: &'a BidRequest,
    account: &'a str,
}

impl<'a> ApsRequest<'a> {
    const fn new(request: &'a BidRequest, account: &'a str) -> Self {
        Self { request, account }
    }
}

impl Serialize for ApsRequest<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Every transformed typed object is exhaustively destructured. A new
        // typed field must be reviewed here instead of silently disappearing.
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
        } = self.request;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", id)?;
        map.serialize_entry("imp", &Impressions(imp))?;
        optional(&mut map, "site", site.as_ref())?;
        optional(&mut map, "app", app.as_ref())?;
        optional(&mut map, "dooh", dooh.as_ref())?;
        if let Some(present_device) = device {
            map.serialize_entry("device", &ApsDevice(present_device))?;
        }
        if let Some(present_user) = user {
            map.serialize_entry("user", &ApsUser(present_user))?;
        }
        optional(&mut map, "regs", regs.as_ref())?;
        optional(&mut map, "source", source.as_ref())?;
        optional(&mut map, "at", at.as_ref())?;
        optional(&mut map, "tmax", tmax.as_ref())?;
        if cur.is_empty() {
            map.serialize_entry("cur", &["USD"])?;
        } else {
            map.serialize_entry("cur", cur)?;
        }
        optional(&mut map, "test", test.as_ref())?;
        map.serialize_entry(
            "ext",
            &ApsExt {
                ext: ext.as_ref(),
                account: self.account,
            },
        )?;
        for (key, value) in other {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

struct Impressions<'a>(&'a [Imp]);

impl Serialize for Impressions<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(None)?;
        for imp in valid_impressions(self.0) {
            sequence.serialize_element(&ApsImp(imp))?;
        }
        sequence.end()
    }
}

struct ApsImp<'a>(&'a Imp);

impl Serialize for ApsImp<'_> {
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
        } = self.0;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", id)?;
        if let Some(present_banner) = banner {
            map.serialize_entry("banner", &ApsBanner(present_banner))?;
        }
        optional(&mut map, "video", video.as_ref())?;
        optional(&mut map, "native", native.as_ref())?;
        optional(&mut map, "audio", audio.as_ref())?;
        optional(&mut map, "bidfloor", bidfloor.as_ref())?;
        optional(&mut map, "bidfloorcur", bidfloorcur.as_ref())?;
        optional(&mut map, "secure", secure.as_ref())?;
        optional(&mut map, "ext", ext.as_ref())?;
        for (key, value) in other {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

struct ApsBanner<'a>(&'a Banner);

impl Serialize for ApsBanner<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Banner {
            format,
            w,
            h,
            pos,
            ext,
            other,
        } = self.0;
        let mut map = serializer.serialize_map(None)?;
        if !format.is_empty() {
            map.serialize_entry("format", format)?;
        }
        if let Some(first) = format.first().filter(|_first| w.is_none() || h.is_none()) {
            map.serialize_entry("w", &first.w)?;
            map.serialize_entry("h", &first.h)?;
        } else {
            optional(&mut map, "w", w.as_ref())?;
            optional(&mut map, "h", h.as_ref())?;
        }
        optional(&mut map, "pos", pos.as_ref())?;
        optional(&mut map, "ext", ext.as_ref())?;
        for (key, value) in other {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

struct ApsExt<'a> {
    ext: Option<&'a BidRequestExt>,
    account: &'a str,
}

#[derive(Serialize)]
struct Sdk {
    source: &'static str,
    version: &'static str,
}

impl Serialize for ApsExt<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        if let Some(BidRequestExt { prebid, other }) = self.ext {
            optional(&mut map, "prebid", prebid.as_ref())?;
            for (key, value) in other {
                if key != "account" && key != "sdk" {
                    map.serialize_entry(key, value)?;
                }
            }
        }
        map.serialize_entry("account", self.account)?;
        map.serialize_entry(
            "sdk",
            &Sdk {
                source: "auctioneer",
                version: env!("CARGO_PKG_VERSION"),
            },
        )?;
        map.end()
    }
}

struct ApsUser<'a>(&'a User);

impl Serialize for ApsUser<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let User {
            id,
            buyeruid,
            yob: _yob,
            gender: _gender,
            keywords,
            consent,
            eids,
            geo: _geo,
            data,
            ext,
            other,
        } = self.0;
        let mut map = serializer.serialize_map(None)?;
        optional(&mut map, "id", id.as_ref())?;
        optional(&mut map, "buyeruid", buyeruid.as_ref())?;
        optional(&mut map, "keywords", keywords.as_ref())?;
        optional(&mut map, "consent", consent.as_ref())?;
        if !eids.is_empty() {
            map.serialize_entry("eids", eids)?;
        }
        optional(&mut map, "data", data.as_ref())?;
        optional(&mut map, "ext", ext.as_ref())?;
        for (key, value) in other {
            // customdata is currently flattened; filter the entire removal
            // set so a typed-only caller cannot reintroduce a removed key.
            if !matches!(key.as_str(), "gender" | "yob" | "customdata" | "geo") {
                map.serialize_entry(key, value)?;
            }
        }
        map.end()
    }
}

struct ApsDevice<'a>(&'a Device);

impl Serialize for ApsDevice<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Device {
            ua,
            sua,
            ip,
            ipv6,
            geo,
            dnt,
            lmt,
            os,
            devicetype,
            make,
            model,
            osv,
            hwv,
            h,
            w,
            ppi,
            pxratio,
            js,
            geofetch,
            flashver,
            language,
            langb,
            carrier,
            mccmnc,
            connectiontype,
            ifa,
            ext,
            other,
        } = self.0;
        let mut map = serializer.serialize_map(None)?;
        optional(&mut map, "ua", ua.as_ref())?;
        json_field(&mut map, "sua", sua)?;
        optional(&mut map, "ip", ip.as_ref())?;
        optional(&mut map, "ipv6", ipv6.as_ref())?;
        if let Some(present_geo) = geo {
            map.serialize_entry("geo", &ApsGeo(present_geo))?;
        }
        optional(&mut map, "dnt", dnt.as_ref())?;
        optional(&mut map, "lmt", lmt.as_ref())?;
        optional(&mut map, "os", os.as_ref())?;
        json_field(&mut map, "devicetype", devicetype)?;
        json_field(&mut map, "make", make)?;
        json_field(&mut map, "model", model)?;
        json_field(&mut map, "osv", osv)?;
        json_field(&mut map, "hwv", hwv)?;
        json_field(&mut map, "h", h)?;
        json_field(&mut map, "w", w)?;
        json_field(&mut map, "ppi", ppi)?;
        json_field(&mut map, "pxratio", pxratio)?;
        json_field(&mut map, "js", js)?;
        json_field(&mut map, "geofetch", geofetch)?;
        json_field(&mut map, "flashver", flashver)?;
        json_field(&mut map, "language", language)?;
        json_field(&mut map, "langb", langb)?;
        json_field(&mut map, "carrier", carrier)?;
        json_field(&mut map, "mccmnc", mccmnc)?;
        json_field(&mut map, "connectiontype", connectiontype)?;
        optional(&mut map, "ifa", ifa.as_ref())?;
        optional(&mut map, "ext", ext.as_ref())?;
        for (key, value) in other {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

struct ApsGeo<'a>(&'a Geo);

impl Serialize for ApsGeo<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Geo {
            country,
            lat: _lat,
            lon: _lon,
            ext,
            other,
        } = self.0;
        let mut map = serializer.serialize_map(None)?;
        optional(&mut map, "country", country.as_ref())?;
        optional(&mut map, "ext", ext.as_ref())?;
        for (key, value) in other {
            if key != "lat" && key != "lon" {
                map.serialize_entry(key, value)?;
            }
        }
        map.end()
    }
}

fn optional<M: SerializeMap, T: Serialize>(
    map: &mut M,
    key: &str,
    field: Option<&T>,
) -> Result<(), M::Error> {
    if let Some(value) = field {
        map.serialize_entry(key, value)?;
    }
    Ok(())
}

fn json_field<M: SerializeMap, T: Serialize>(
    map: &mut M,
    key: &str,
    field: &JsonField<T>,
) -> Result<(), M::Error> {
    if !field.is_missing() {
        map.serialize_entry(key, field)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::tests::{
        EU, FE, HOSTS, LIMITS, NA, body, drive, drive_with, imp, info, minimal, request, shape,
        value,
    };
    use crate::diagnostics::ConstructionDiagnostic;
    use crate::endpoint_validator::EndpointRejectReason;
    use crate::meta::META_BYTES_CAPACITY;
    use crate::sink::SinkLimits;
    use crate::types::OutboundBuildError;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_runtime::bounded_json::JsonFieldShape;
    use serde_json::{Value, json};

    #[test]
    fn trims_accounts_and_routes_all_reviewed_regions_without_changing_params() {
        for (region, url) in [(" na ", NA), ("\teu\n", EU), (" fe ", FE), (" \t", NA)] {
            let request = request(json!([imp(
                "i",
                json!({"accountID": "  account\t", "region": region})
            )]));
            let before = value(&request);
            let outcome = drive(&request);
            assert_eq!(outcome.accepted[0].url(), url);
            assert_eq!(body(&outcome)["ext"]["account"], "account");
            assert_eq!(body(&outcome)["imp"], before["imp"]);
            assert_eq!(value(&request), before);
        }
    }

    #[test]
    fn last_valid_impression_wins_in_both_orders_and_omission_resets_na() {
        for (regions, expected) in [
            ([Some("eu"), Some("fe")], FE),
            ([Some("fe"), Some("eu")], EU),
            ([Some("eu"), None], NA),
            ([None, Some("eu")], EU),
            ([Some("eu"), Some("")], NA),
        ] {
            let imps: Vec<_> = regions
                .iter()
                .enumerate()
                .map(|(index, region)| {
                    let mut params = json!({"accountID": "account"});
                    if let Some(present_region) = region {
                        params["region"] = json!(present_region);
                    }
                    imp(&index.to_string(), params)
                })
                .collect();
            let outcome = drive(&request(json!(imps)));
            assert_eq!(outcome.accepted[0].url(), expected);
            assert_eq!(
                outcome.accepted[0].covered_imp_ids().collect::<Vec<_>>(),
                ["0", "1"]
            );
        }
    }

    #[test]
    fn invalid_parameters_are_skipped_without_changing_account_or_region() {
        for params in [
            json!({}),
            json!({"accountID": ""}),
            json!({"accountID": " \n"}),
            json!({"accountID": 42_i32}),
            json!({"accountID": null}),
            json!({"accountID": true}),
            json!({"accountID": []}),
            json!({"accountID": "other", "region": "xx"}),
            json!({"accountID": "other", "region": "EU"}),
            json!({"accountID": "other", "region": "na.evil.test/"}),
            json!({"accountID": "other", "region": "na%2Fevil"}),
            json!({"accountID": "other", "region": 1_i32}),
            json!({"accountID": "other", "region": null}),
            json!({"accountID": "other", "region": []}),
            json!([]),
            json!("aps"),
            json!(1_i32),
            json!(true),
            json!(null),
        ] {
            let request = request(json!([
                imp("bad-first", params.clone()),
                imp("good", json!({"accountID": "account", "region": "eu"})),
                imp("bad-last", params),
            ]));
            let outcome = drive(&request);
            assert_eq!(outcome.accepted[0].url(), EU);
            assert_eq!(
                outcome.accepted[0].covered_imp_ids().collect::<Vec<_>>(),
                ["good"]
            );
            assert_eq!(body(&outcome)["imp"].as_array().unwrap().len(), 1);
            assert_eq!(body(&outcome)["ext"]["account"], "account");
            assert!(
                outcome
                    .diagnostics
                    .iter()
                    .eq([ConstructionDiagnostic::InvalidImpressionParameters; 2].iter())
            );
        }
    }

    #[test]
    fn missing_extensions_and_all_invalid_impressions_emit_diagnostics_not_calls() {
        let input = request(json!([
            {"id": "no-ext", "banner": {}},
            {"id": "no-bidder", "banner": {}, "ext": {}},
            imp("empty", json!({"accountID": ""})),
        ]));
        let outcome = drive(&input);
        assert!(outcome.accepted.is_empty());
        assert!(
            outcome
                .diagnostics
                .iter()
                .eq([ConstructionDiagnostic::InvalidImpressionParameters; 3].iter())
        );
        assert!(drive(&request(json!([]))).accepted.is_empty());
    }

    #[test]
    fn conflicting_valid_accounts_are_fatal_before_any_call_and_do_not_echo_values() {
        let request = request(json!([
            imp("bad", json!({})),
            imp("one", json!({"accountID": "private-one", "region": "eu"})),
            imp("two", json!({"accountID": "private-two", "region": "na"})),
        ]));
        let (semantic, outcome, remaining) =
            drive_with(&request, &info(JsonFieldShape::Absent), LIMITS, 1);
        let errors = semantic.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 2);
        assert!(!errors[0].message.contains("private"));
        assert!(outcome.accepted.is_empty());
        assert!(outcome.errors.is_empty());
        assert_eq!(outcome.diagnostics.len(), 1);
        assert_eq!(remaining, 1);
    }

    #[test]
    fn raw_null_is_not_normalized_to_absence_before_the_guard() {
        let mut input = value(&minimal());
        input["ext"] = Value::Null;
        let wire = serde_json::to_vec(&input).unwrap();
        let request: BidRequest = serde_json::from_slice(&wire).unwrap();
        assert!(request.ext.is_none());
        assert_eq!(shape(&wire), JsonFieldShape::Null);
        let (semantic, outcome, remaining) = drive_with(&request, &info(shape(&wire)), LIMITS, 1);
        assert_eq!(semantic.unwrap_err()[0].code, 2);
        assert!(outcome.accepted.is_empty());
        assert!(outcome.errors.is_empty());
        assert_eq!(remaining, 1);
    }

    #[test]
    fn original_unsafe_shapes_refuse_even_if_a_transform_removed_the_extension() {
        for (wire, expected) in [
            (
                serde_json::to_vec(&json!({"ext":null})).unwrap(),
                JsonFieldShape::Null,
            ),
            (
                serde_json::to_vec(&json!({"ext":[]})).unwrap(),
                JsonFieldShape::NonObject,
            ),
            (
                serde_json::to_vec(&json!({"ext":true})).unwrap(),
                JsonFieldShape::NonObject,
            ),
            (
                serde_json::to_vec(&json!({"ext":7_i32})).unwrap(),
                JsonFieldShape::NonObject,
            ),
            (
                serde_json::to_vec(&json!({"ext":"object"})).unwrap(),
                JsonFieldShape::NonObject,
            ),
            (
                br#"{"ext":{},"ext":{}}"#.to_vec(),
                JsonFieldShape::Duplicate,
            ),
            (
                br#"{"ext":null,"e\u0078t":{}}"#.to_vec(),
                JsonFieldShape::Duplicate,
            ),
        ] {
            let provenance = shape(&wire);
            assert_eq!(provenance, expected);
            let (semantic, outcome, remaining) =
                drive_with(&minimal(), &info(provenance), SinkLimits::new(0, 16), 1);
            assert_eq!(semantic.unwrap_err()[0].code, 2);
            assert!(outcome.accepted.is_empty());
            assert!(outcome.errors.is_empty());
            assert_eq!(remaining, 1);
        }
    }

    #[test]
    fn typed_callers_cannot_claim_wire_absence_but_typed_objects_are_safe() {
        let (unavailable_semantic, unavailable_outcome, _) =
            drive_with(&minimal(), &info(JsonFieldShape::Unavailable), LIMITS, 1);
        assert_eq!(unavailable_semantic.unwrap_err()[0].code, 2);
        assert!(unavailable_outcome.accepted.is_empty());
        for provenance in [JsonFieldShape::Absent, JsonFieldShape::Object] {
            let (semantic, outcome, _) = drive_with(&minimal(), &info(provenance), LIMITS, 1);
            semantic.unwrap();
            assert_eq!(outcome.accepted.len(), 1);
        }
        let mut request = minimal();
        request.ext = Some(serde_json::from_value(json!({"keep": true})).unwrap());
        let (semantic, outcome, _) =
            drive_with(&request, &info(JsonFieldShape::Unavailable), LIMITS, 1);
        semantic.unwrap();
        assert_eq!(body(&outcome)["ext"]["keep"], true);
        // Wire evidence takes precedence over even a typed replacement object.
        let (null_semantic, null_outcome, _) =
            drive_with(&request, &info(JsonFieldShape::Null), LIMITS, 1);
        assert_eq!(null_semantic.unwrap_err()[0].code, 2);
        assert!(null_outcome.accepted.is_empty());
    }

    #[test]
    fn replaces_only_account_and_sdk_preserving_unrelated_extension_keys() {
        let request: BidRequest = serde_json::from_value(json!({
            "id": "r", "imp": [imp("i", json!({"accountID": "account"}))],
            "ext": {"account": {"old": 1_i32}, "sdk": {"source": "other", "version": "old", "extra": 1_i32},
                "keep": {"nested": [null, 1_i32]}, "prebid": {"debug": true}}
        }))
        .unwrap();
        let before = value(&request);
        assert_eq!(
            body(&drive(&request))["ext"],
            json!({
                "account": "account", "sdk": {"source": "auctioneer", "version": env!("CARGO_PKG_VERSION")},
                "keep": {"nested": [null, 1_i32]}, "prebid": {"debug": true}
            })
        );
        assert_eq!(value(&request), before);
    }

    #[test]
    fn empty_currency_defaults_but_nonempty_currencies_are_untouched() {
        for currencies in [json!([]), json!(["EUR", "USD"])] {
            let mut request = minimal();
            request.cur = serde_json::from_value(currencies.clone()).unwrap();
            let expected = if request.cur.is_empty() {
                json!(["USD"])
            } else {
                currencies
            };
            assert_eq!(body(&drive(&request))["cur"], expected);
        }
    }

    #[test]
    fn backfills_both_dimensions_only_from_the_first_format_if_either_is_missing() {
        for (banner, expected) in [
            (
                json!({"format": [{"w": 300_i32, "h": 250_i32}, {"w": 728_i32, "h": 90_i32}]}),
                (Some(300), Some(250)),
            ),
            (
                json!({"w": 970_i32, "format": [{"w": 300_i32, "h": 250_i32}]}),
                (Some(300), Some(250)),
            ),
            (
                json!({"h": 90_i32, "format": [{"w": 300_i32, "h": 250_i32}]}),
                (Some(300), Some(250)),
            ),
            (
                json!({"w": 0_i32, "h": 0_i32, "format": [{"w": 300_i32, "h": 250_i32}]}),
                (Some(0), Some(0)),
            ),
            (
                json!({"w": 970_i32, "h": 90_i32, "format": [{"w": 300_i32, "h": 250_i32}]}),
                (Some(970), Some(90)),
            ),
            (json!({"w": 970_i32}), (Some(970), None)),
            (json!({}), (None, None)),
        ] {
            let mut request = minimal();
            request.imp[0].banner = Some(serde_json::from_value(banner).unwrap());
            let before = value(&request);
            let output = body(&drive(&request));
            assert_eq!(
                output["imp"][0]["banner"].get("w").and_then(Value::as_i64),
                expected.0
            );
            assert_eq!(
                output["imp"][0]["banner"].get("h").and_then(Value::as_i64),
                expected.1
            );
            assert_eq!(value(&request), before);
        }
    }

    #[test]
    fn borrowed_views_preserve_all_unmodified_typed_and_flattened_fields() {
        let mut input = json!({
            "id": "r", "imp": [{"id": "i", "banner": {"w": 300_i32, "h": 250_i32,
                "pos": 1_i32, "ext": {"banner-key": 1_i32}, "format": [{"w": 300_i32, "h": 250_i32, "ratio": 2_i32}], "battr": [1_i32]},
                "video": {"mimes": ["video/mp4"]}, "audio": {}, "native": {},
                "bidfloor": 1.5_f64, "bidfloorcur": "EUR", "secure": 1_i32, "tagid": "tag",
                "ext": {"bidder": {"accountID": "account"}, "keep": [1_i32]}}],
            "site": {"id": "site"}, "app": {"id": "app"}, "dooh": {"id": "dooh"},
            "regs": {"coppa": 1_i32}, "source": {"tid": "tid"}, "at": 2_i32, "tmax": 31_i32,
            "cur": ["EUR"], "test": 0_i32, "wseat": ["seat"],
            "user": {"id": "user", "buyeruid": "buyer", "yob": 1980_i32, "gender": "F",
                "customdata": "secret", "geo": {"country": "USA"}, "keywords": "kw",
                "consent": "consent", "eids": [{"source": "example.test", "uids": [{"id": "uid"}]}],
                "data": [{"id": "data"}], "ext": {"keep": "user-ext"}, "keep": "user"}
        });
        input["device"] = json!({"ua": "UA", "sua": {"mobile": null}, "ip": "1.2.3.4", "ipv6": "2001:db8::1",
                "geo": {"country": "USA", "lat": 1.0_f64, "lon": 2.0_f64, "ext": {"keep": 1_i32}, "city": "city"},
                "dnt": 0_i32, "lmt": 1_i32, "os": "OS", "devicetype": 1_i32, "make": null, "model": "model",
                "osv": "1", "hwv": "2", "h": 1000_i32, "w": 500_i32, "ppi": 200_i32, "pxratio": 2_i32,
                "js": 1_i32, "geofetch": 0_i32, "flashver": "none", "language": "en", "langb": "en-US",
                "carrier": "carrier", "mccmnc": "123", "connectiontype": 2_i32, "ifa": "ifa",
                "ext": {"keep": "device-ext"}, "keep": "device"
        });
        let request: BidRequest = serde_json::from_value(input.clone()).unwrap();
        let before = serde_json::to_vec(&request).unwrap();
        let mut expected = input;
        for key in ["yob", "gender", "customdata", "geo"] {
            assert!(
                expected["user"]
                    .as_object_mut()
                    .unwrap()
                    .remove(key)
                    .is_some()
            );
        }
        for key in ["lat", "lon"] {
            assert!(
                expected["device"]["geo"]
                    .as_object_mut()
                    .unwrap()
                    .remove(key)
                    .is_some()
            );
        }
        expected["ext"] = json!({"account":"account", "sdk":{"source":"auctioneer", "version":env!("CARGO_PKG_VERSION")}});
        assert_eq!(body(&drive(&request)), expected);
        assert_eq!(serde_json::to_vec(&request).unwrap(), before);
    }

    #[test]
    fn never_reintroduces_user_or_device_data_removed_by_shared_privacy() {
        let request: BidRequest = serde_json::from_value(json!({
            "id": "r", "imp": [imp("i", json!({"accountID": "account"}))],
            "user": {"keywords": "allowed"}, "device": {"geo": {"country": "USA"}}
        }))
        .unwrap();
        let output = body(&drive(&request));
        assert_eq!(output["user"], json!({"keywords": "allowed"}));
        assert_eq!(output["device"], json!({"geo": {"country": "USA"}}));
        let absent_output = body(&drive(&minimal()));
        assert!(absent_output.get("user").is_none());
        assert!(absent_output.get("device").is_none());
    }

    #[test]
    fn appends_a_structured_query_pair_only_for_test_one_and_preserves_existing_query() {
        let mut request = minimal();
        request.test = Some(1_i32);
        let mut context = info(JsonFieldShape::Absent);
        context.endpoint_template = "https://{{.Host}}/e/pb/bid?keep=a%20b";
        let (semantic, outcome, _) = drive_with(&request, &context, LIMITS, 1);
        semantic.unwrap();
        assert!(outcome.errors.is_empty());
        let url = url::Url::parse(outcome.accepted[0].url()).unwrap();
        assert_eq!(
            url.query_pairs().into_owned().collect::<Vec<_>>(),
            [
                ("keep".to_owned(), "a b".to_owned()),
                ("amzn_debug_mode".to_owned(), "1".to_owned())
            ]
        );
        for test in [None, Some(0_i32), Some(2_i32)] {
            request.test = test;
            assert_eq!(drive(&request).accepted[0].url(), NA);
        }
    }

    #[test]
    fn endpoint_host_declarations_and_sink_validation_cannot_be_bypassed() {
        let mut context = info(JsonFieldShape::Absent);
        context.host_allowlist = None;
        let (semantic, outcome, remaining) = drive_with(&minimal(), &context, LIMITS, 1);
        semantic.unwrap();
        assert!(outcome.accepted.is_empty());
        assert_eq!(outcome.errors.len(), 1);
        assert_eq!(remaining, 1);
        context.host_allowlist = Some(HOSTS);
        context.endpoint_template = "https://evil.example/e/pb/bid";
        let (rejected_semantic, rejected_outcome, rejected_remaining) =
            drive_with(&minimal(), &context, SinkLimits::new(0, 16), 1);
        rejected_semantic.unwrap();
        assert_eq!(
            rejected_outcome.errors,
            [OutboundBuildError::EndpointRejected {
                reason: EndpointRejectReason::AuthorityMismatch
            }]
        );
        assert!(rejected_outcome.accepted.is_empty());
        assert_eq!(rejected_remaining, 1);
    }

    #[test]
    fn body_metadata_and_call_limits_are_enforced_with_reservation_refunds() {
        let context = info(JsonFieldShape::Absent);
        let (semantic, outcome, remaining) = drive_with(&minimal(), &context, LIMITS, 0);
        semantic.unwrap();
        assert!(outcome.accepted.is_empty());
        assert_eq!(outcome.rejected_call_count, 1);
        assert_eq!(
            outcome.errors,
            [OutboundBuildError::CallBudgetExhausted { max_calls: 0 }]
        );
        assert_eq!(remaining, 0);

        let (body_semantic, body_outcome, body_remaining) =
            drive_with(&minimal(), &context, SinkLimits::new(1, 16), 1);
        body_semantic.unwrap();
        assert!(body_outcome.accepted.is_empty());
        assert_eq!(
            body_outcome.errors,
            [OutboundBuildError::OversizeRaw { limit: 1 }]
        );
        assert_eq!(body_remaining, 1);

        let (ids_semantic, ids_outcome, ids_remaining) =
            drive_with(&minimal(), &context, SinkLimits::new(0, 0), 1);
        ids_semantic.unwrap();
        assert!(ids_outcome.accepted.is_empty());
        assert_eq!(
            ids_outcome.errors,
            [OutboundBuildError::TooManyImpIds { limit: 0 }]
        );
        assert_eq!(ids_remaining, 1);

        let mut request = minimal();
        request.imp[0].id = "x".repeat(META_BYTES_CAPACITY);
        let (meta_semantic, meta_outcome, meta_remaining) =
            drive_with(&request, &context, LIMITS, 1);
        meta_semantic.unwrap();
        assert!(meta_outcome.accepted.is_empty());
        assert!(matches!(
            meta_outcome.errors.as_slice(),
            [OutboundBuildError::MetaOverCap { .. }]
        ));
        assert_eq!(meta_remaining, 1);
    }
}
