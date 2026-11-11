use crate::bidders::url::json_utf8_headers;
use crate::{BidderError, BidderRequestInfo, EndpointBuilder, OutboundRequestSink};
use crate::{BidderSettings, bidder_endpoint, bidder_settings::thetradedesk};
use auctioneer_openrtb::types::{
    bid_request::{Banner, BidRequest, Imp},
    field::JsonField,
    inventory::{App, Publisher, Site},
};
use serde::{
    Serialize, Serializer,
    ser::{SerializeMap as _, SerializeSeq as _},
};
use serde_json::{Map, Value};

pub(super) fn make_requests(
    request: &BidRequest,
    info: &BidderRequestInfo<'_>,
    sink: &mut OutboundRequestSink<'_>,
) -> Result<(), Vec<BidderError>> {
    let selected = select_parameters(&request.imp).map_err(|error| vec![error])?;
    let settings = thetradedesk::resolve(
        info.settings().and_then(BidderSettings::as_thetradedesk),
        info.base_settings()
            .and_then(BidderSettings::as_thetradedesk),
    );
    let supply = if selected.supply.is_empty() {
        settings.supply_id().unwrap_or_default()
    } else {
        selected.supply
    };
    if supply.is_empty() {
        return Err(vec![missing_supply()]);
    }
    if !info.endpoint_macros.contains(&"SupplyId")
        || !info.endpoint_template.contains("{{.SupplyId}}")
    {
        return Err(vec![route_refused()]);
    }
    let mut refused_route = false;
    let _recorded = sink.write_json(
        info,
        |meta| {
            meta.set_method("POST")?;
            let bindings = bidder_endpoint::thetradedesk::Values { supply_id: supply };
            let values = bindings.as_pairs();
            if let Err(error) = EndpointBuilder::from_request(info, &values).expand_into(meta) {
                refused_route = true;
                return Err(error);
            }
            json_utf8_headers(meta)?;
            for imp in &request.imp {
                meta.imp_id(&imp.id)?;
            }
            Ok(())
        },
        &Request {
            request,
            publisher: selected.publisher,
        },
    );
    if refused_route {
        Err(vec![route_refused()])
    } else {
        Ok(())
    }
}

fn missing_supply() -> BidderError {
    BidderError::failed_to_request_bids("thetradedesk requires nonempty SupplyId")
}

fn route_refused() -> BidderError {
    BidderError::failed_to_request_bids("thetradedesk SupplyId route refused")
}

pub(super) fn invalid_parameters() -> BidderError {
    BidderError::failed_to_unmarshal("invalid thetradedesk parameters")
}

#[derive(Default)]
struct Parameters<'a> {
    publisher: &'a str,
    supply: &'a str,
}

fn parameter_string<'a>(fields: &'a Map<String, Value>, key: &str) -> Result<&'a str, BidderError> {
    match fields.get(key) {
        None | Some(Value::Null) => Ok(""),
        Some(Value::String(value)) => Ok(value),
        Some(_) => Err(invalid_parameters()),
    }
}

fn select_parameters(imps: &[Imp]) -> Result<Parameters<'_>, BidderError> {
    let mut selected = Parameters::default();
    for imp in imps {
        let ext = imp.ext.as_ref().ok_or_else(invalid_parameters)?;
        let fields = match &ext.bidder {
            JsonField::Null | JsonField::Value(Value::Null) => continue,
            JsonField::Value(Value::Object(fields)) => fields,
            JsonField::Missing | JsonField::Value(_) => return Err(invalid_parameters()),
        };
        let publisher = parameter_string(fields, "publisherId")?;
        let supply = parameter_string(fields, "supplySourceId")?;
        if selected.publisher.is_empty() {
            selected.publisher = publisher;
        }
        if selected.supply.is_empty() {
            selected.supply = supply;
        }
        if !selected.publisher.is_empty() && !selected.supply.is_empty() {
            break;
        }
    }
    Ok(selected)
}

struct Request<'a> {
    request: &'a BidRequest,
    publisher: &'a str,
}

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
        } = self.request;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", id)?;
        if !imp.is_empty() {
            map.serialize_entry("imp", &Impressions(imp))?;
        }
        if let Some(inventory) = site {
            map.serialize_entry(
                "site",
                &SiteView {
                    site: inventory,
                    publisher: self.publisher,
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
                        publisher: self.publisher,
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
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for imp in self.0 {
            seq.serialize_element(&Impression(imp))?;
        }
        seq.end()
    }
}

struct Impression<'a>(&'a Imp);

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
        } = self.0;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", id)?;
        if let Some(value) = banner {
            map.serialize_entry("banner", &BannerView(value))?;
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
        if let Some(value) = ext {
            map.serialize_entry("ext", value)?;
        }
        for (key, value) in other {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

struct BannerView<'a>(&'a Banner);

impl Serialize for BannerView<'_> {
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
        if let Some(first) = format.first() {
            map.serialize_entry("w", &first.w)?;
            map.serialize_entry("h", &first.h)?;
        } else {
            if let Some(value) = w {
                map.serialize_entry("w", value)?;
            }
            if let Some(value) = h {
                map.serialize_entry("h", value)?;
            }
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

struct SiteView<'a> {
    site: &'a Site,
    publisher: &'a str,
}

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
        } = self.site;
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
        map.serialize_entry(
            "publisher",
            &PublisherView {
                publisher: publisher.as_ref(),
                selected: self.publisher,
            },
        )?;
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
    publisher: &'a str,
}

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
        } = self.app;
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
        map.serialize_entry(
            "publisher",
            &PublisherView {
                publisher: publisher.as_ref(),
                selected: self.publisher,
            },
        )?;
        if let Some(value) = ext {
            map.serialize_entry("ext", value)?;
        }
        for (key, value) in other {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

struct PublisherView<'a> {
    publisher: Option<&'a Publisher>,
    selected: &'a str,
}

impl Serialize for PublisherView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        if !self.selected.is_empty() {
            map.serialize_entry("id", self.selected)?;
        }
        if let Some(Publisher {
            id,
            name,
            domain,
            ext,
            other,
        }) = self.publisher
        {
            if self.selected.is_empty()
                && let Some(value) = id
            {
                map.serialize_entry("id", value)?;
            }
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

#[cfg(test)]
mod tests {
    use super::super::TheTradeDeskBidder;
    use super::super::tests::{ENDPOINT, calls, info, minimal, request};
    use super::{invalid_parameters, missing_supply, route_refused};
    use crate::BidderSettings;
    use crate::bidders::harness;
    use crate::{
        BidderError, BidderHttpRequest, BidderRequestInfo, EndpointRejectReason, EndpointValidator,
        OutboundBuildError, SinkLimits,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_openrtb::types::field::JsonField;
    use serde_json::{Value, json};

    struct Policy(bool);

    impl EndpointValidator for Policy {
        fn validate(
            &self,
            url: &url::Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            if self.0 && url.host_str() == Some("bidder.example.test") {
                Ok(())
            } else {
                Err(EndpointRejectReason::HostNotAllowed)
            }
        }
    }

    fn settings(supply: Option<&str>) -> BidderSettings {
        serde_json::from_value(json!({
            "implementation":"thetradedesk", "settings":{"supply_id":supply},
        }))
        .unwrap()
    }

    fn body(call: &BidderHttpRequest) -> Value {
        serde_json::from_slice(call.body()).unwrap()
    }

    #[test]
    fn identifiers_are_selected_independently_and_later_rows_are_not_decoded() {
        for imps in [
            json!([
                {"id":"a", "banner":{}, "ext":{"bidder":{"publisherId":"first"}}},
                {"id":"b", "video":{}, "ext":{"bidder":{
                    "publisherId":"ignored", "supplySourceId":"chosen"}}},
                {"id":"c", "audio":{}, "ext":{"bidder":false}},
            ]),
            json!([
                {"id":"a", "banner":{}, "ext":{"bidder":{"supplySourceId":"chosen"}}},
                {"id":"b", "video":{}, "ext":{"bidder":{
                    "publisherId":"first", "supplySourceId":"ignored"}}},
                {"id":"c", "audio":{}},
            ]),
        ] {
            let request = request(imps);
            let before = serde_json::to_value(&request).unwrap();
            let calls = calls(&request);
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].url(), "https://bidder.example.test/bid/chosen");
            assert_eq!(body(&calls[0])["site"]["publisher"]["id"], "first");
            assert_eq!(body(&calls[0])["imp"], before["imp"]);
            assert_eq!(
                calls[0].covered_imp_ids().collect::<Vec<_>>(),
                ["a", "b", "c"]
            );
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
    }

    #[test]
    fn selected_row_decodes_both_fields_before_early_exit() {
        for (key, bad) in [
            ("publisherId", json!(123_i32)),
            ("supplySourceId", json!(false)),
            ("supplySourceId", json!([])),
        ] {
            let mut request = minimal();
            request.imp[0]
                .ext
                .as_mut()
                .unwrap()
                .bidder
                .as_mut()
                .unwrap()[key] = bad;
            let (semantic, calls) =
                harness::drive(&TheTradeDeskBidder, &request, &info(None, None));
            assert_eq!(semantic.unwrap_err(), [invalid_parameters()]);
            assert!(calls.is_empty());
        }
    }

    #[test]
    fn malformed_rows_before_both_identifiers_are_found_are_fatal() {
        for first in [
            json!({"id":"bad", "banner":{}}),
            json!({"id":"bad", "banner":{}, "ext":{}}),
            json!({"id":"bad", "banner":{}, "ext":{"bidder":false}}),
            json!({"id":"bad", "banner":{}, "ext":{"bidder":[]}}),
        ] {
            let request = request(json!([first, {"id":"good", "banner":{}, "ext":{"bidder":{
                "publisherId":"p", "supplySourceId":"s"}}}]));
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &TheTradeDeskBidder,
                &request,
                &info(None, None),
                SinkLimits::new(4096, 2),
                1,
                &Policy(true),
            );
            assert_eq!(semantic.unwrap_err(), [invalid_parameters()]);
            assert!(outcome.accepted.is_empty());
            assert!(outcome.errors.is_empty());
            assert_eq!(remaining, 1);
        }
    }

    #[test]
    fn null_and_empty_fields_continue_scanning_without_trimming_values() {
        let request = request(json!([
            {"id":"a", "banner":{}, "ext":{"bidder":null}},
            {"id":"b", "banner":{}, "ext":{"bidder":{
                "publisherId":null, "supplySourceId":""}}},
            {"id":"c", "banner":{}, "ext":{"bidder":{
                "publisherId":" ", "supplySourceId":" route "}}},
        ]));
        let calls = calls(&request);
        assert_eq!(
            calls[0].url(),
            "https://bidder.example.test/bid/%20route%20"
        );
        assert_eq!(body(&calls[0])["site"]["publisher"]["id"], " ");
    }

    #[test]
    fn site_precedes_app_and_empty_publisher_preserves_existing_identity() {
        for publisher in ["", "selected"] {
            let mut request = minimal();
            request.app = Some(
                serde_json::from_value(json!({
                    "bundle":"app", "publisher":{"id":"app-existing", "name":"app-name"},
                }))
                .unwrap(),
            );
            request.imp[0].ext.as_mut().unwrap().bidder = JsonField::Value(json!({
                "publisherId":publisher, "supplySourceId":"route",
            }));
            let before = serde_json::to_value(&request).unwrap();
            let actual = body(&calls(&request)[0]);
            assert_eq!(actual["app"], before["app"]);
            assert_eq!(
                actual["site"]["publisher"]["id"],
                if publisher.is_empty() {
                    "existing"
                } else {
                    publisher
                }
            );
            request.site = None;
            let app_body = body(&calls(&request)[0]);
            assert_eq!(
                app_body["app"]["publisher"]["id"],
                if publisher.is_empty() {
                    "app-existing"
                } else {
                    publisher
                }
            );
            assert_eq!(app_body["app"]["publisher"]["name"], "app-name");
        }
    }

    #[test]
    fn missing_publisher_is_created_without_inventing_an_id() {
        for app in [false, true] {
            let mut request = minimal();
            request.imp[0].ext.as_mut().unwrap().bidder = JsonField::Value(json!({
                "supplySourceId":"route",
            }));
            request.site.as_mut().unwrap().publisher = None;
            let branch = if app {
                request.site = None;
                request.app = Some(serde_json::from_value(json!({"bundle":"app"})).unwrap());
                "app"
            } else {
                "site"
            };
            assert_eq!(body(&calls(&request)[0])[branch]["publisher"], json!({}));
        }
    }

    #[test]
    fn first_banner_format_replaces_dimensions_and_preserves_all_other_fields() {
        let request: BidRequest = serde_json::from_value(json!({
            "id":"r", "at":2_i32, "tmax":99_i64, "test":1_i32, "cur":["EUR"],
            "dooh":{"id":"d"}, "device":{"ua":"scoped"}, "user":{"keywords":"scoped"},
            "regs":{"ext":{"gdpr":1_i32}}, "source":{"tid":"t"},
            "ext":{"preserved":{"x":true}}, "bcat":["IAB1"],
            "site":{"id":"site", "name":"name", "domain":"domain", "page":"page",
                "publisher":{"id":"p", "name":"pub", "domain":"pub-domain",
                    "ext":{"prebid":{"parentAccount":"parent"}}, "cat":["IAB2"]},
                "ext":{"amp":1_i32}, "ref":"ref"},
            "imp":[{"id":"i", "bidfloor":1.25_f64, "bidfloorcur":"EUR", "secure":1_i32,
                "banner":{"w":1_i32,"h":2_i32,"pos":3_i32,"ext":{"x":1_i32},
                    "format":[{"w":300_i32,"h":250_i32},{"w":728_i32,"h":90_i32}],
                    "battr":[1_i32]}, "video":{"mimes":null}, "native":{"request":"{}"},
                "audio":{}, "tagid":"tag", "ext":{"bidder":{
                    "publisherId":"p", "supplySourceId":"route"}}}],
        }))
        .unwrap();
        let before = serde_json::to_value(&request).unwrap();
        let mut expected = before.clone();
        expected["imp"][0]["banner"]["w"] = json!(300_i32);
        expected["imp"][0]["banner"]["h"] = json!(250_i32);
        let calls = calls(&request);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].method(), "POST");
        assert_eq!(
            calls[0].headers().collect::<Vec<_>>(),
            [
                ("Content-Type", "application/json;charset=utf-8"),
                ("Accept", "application/json"),
            ]
        );
        assert_eq!(body(&calls[0]), expected);
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn absent_formats_leave_original_dimensions_and_no_inventory_stays_absent() {
        let mut request = minimal();
        request.site = None;
        request.imp[0].banner.as_mut().unwrap().w = Some(320_i32);
        request.imp[0].banner.as_mut().unwrap().h = Some(50_i32);
        assert_eq!(
            body(&calls(&request)[0]),
            serde_json::to_value(&request).unwrap()
        );
    }

    #[test]
    fn typed_alias_settings_take_field_precedence_including_empty() {
        let request = request(json!([{"id":"i", "banner":{},
            "ext":{"bidder":{"publisherId":"p"}}}]));
        for (requested, base, expected) in [
            (None, Some("base"), Some("base")),
            (Some("alias"), Some("base"), Some("alias")),
            (Some(""), Some("base"), None),
            (None, None, None),
        ] {
            let requested_settings = settings(requested);
            let base_settings = settings(base);
            let info = info(Some(&requested_settings), Some(&base_settings));
            let (semantic, calls) = harness::drive(&TheTradeDeskBidder, &request, &info);
            if let Some(route) = expected {
                semantic.unwrap();
                assert_eq!(calls.len(), 1);
                assert_eq!(
                    calls[0].url(),
                    format!("https://bidder.example.test/bid/{route}")
                );
            } else {
                assert_eq!(semantic.unwrap_err(), [missing_supply()]);
                assert!(calls.is_empty());
            }
            assert_eq!(info.bidder.as_str(), "ttd");
            assert_eq!(info.core_name.as_str(), "thetradedesk");
        }
    }

    #[test]
    fn impression_supply_overrides_even_empty_or_unsafe_host_settings() {
        for host in [None, Some(""), Some("../private"), Some("host")] {
            let base = settings(host);
            let calls =
                harness::drive_ok(&TheTradeDeskBidder, &minimal(), &info(None, Some(&base)));
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].url(), "https://bidder.example.test/bid/route");
        }
    }

    #[test]
    fn approved_policy_requires_nonempty_supply_and_never_invents_a_default() {
        for params in [json!({}), json!({"publisherId":"p", "supplySourceId":""})] {
            let request = request(json!([{"id":"i", "banner":{}, "ext":{"bidder":params}}]));
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &TheTradeDeskBidder,
                &request,
                &info(None, None),
                SinkLimits::new(4096, 1),
                1,
                &Policy(true),
            );
            assert_eq!(
                semantic.unwrap_err(),
                [BidderError::failed_to_request_bids(
                    "thetradedesk requires nonempty SupplyId"
                )]
            );
            assert!(outcome.accepted.is_empty());
            assert!(outcome.errors.is_empty());
            assert_eq!(remaining, 1);
        }
    }

    #[test]
    fn route_must_consume_an_explicit_declared_supply_macro() {
        for (template, macros) in [
            ("https://bidder.example.test/bid/static", &["SupplyId"][..]),
            (ENDPOINT, &[][..]),
        ] {
            let mut info = info(None, None);
            info.endpoint_template = template;
            info.endpoint_macros = macros;
            let (semantic, calls) = harness::drive(&TheTradeDeskBidder, &minimal(), &info);
            assert_eq!(semantic.unwrap_err(), [route_refused()]);
            assert!(calls.is_empty());
        }
    }

    #[test]
    fn shared_endpoint_builder_encodes_data_and_rejects_traversal_with_refunds() {
        for (supply, suffix) in [
            ("a/b ?#%&+", Some("a%2Fb%20%3F%23%25%26%2B")),
            ("../x", None),
            ("%252e%252e%252fx", None),
            ("a\\..\\b", None),
            ("%00", None),
        ] {
            let mut request = minimal();
            request.imp[0].ext.as_mut().unwrap().bidder = JsonField::Value(json!({
                "publisherId":"p", "supplySourceId":supply,
            }));
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &TheTradeDeskBidder,
                &request,
                &info(None, None),
                SinkLimits::new(4096, 1),
                1,
                &Policy(true),
            );
            if let Some(encoded) = suffix {
                semantic.unwrap();
                assert_eq!(outcome.accepted.len(), 1);
                assert_eq!(
                    outcome.accepted[0].url(),
                    format!("https://bidder.example.test/bid/{encoded}")
                );
                assert_eq!(remaining, 0);
            } else {
                assert_eq!(semantic.unwrap_err(), [route_refused()]);
                assert!(outcome.accepted.is_empty());
                assert_eq!(outcome.errors.len(), 1);
                assert_eq!(remaining, 1);
            }
        }
    }

    #[test]
    fn construction_limits_are_independent_and_refused_calls_refund_slots() {
        let request = minimal();
        let exact = calls(&request)[0].body().len();
        for (cap, slots, allowed, success, remaining) in [
            (exact, 1, true, true, 0),
            (exact - 1, 1, true, false, 1),
            (exact, 0, true, false, 0),
            (exact, 1, false, false, 1),
        ] {
            let (semantic, outcome, actual_remaining) = harness::drive_with_limits(
                &TheTradeDeskBidder,
                &request,
                &info(None, None),
                SinkLimits::new(cap, 1),
                slots,
                &Policy(allowed),
            );
            semantic.unwrap();
            assert_eq!(outcome.accepted.len(), usize::from(success));
            assert_eq!(actual_remaining, remaining);
            assert_eq!(outcome.errors.len(), usize::from(!success));
            if slots == 0 {
                assert!(matches!(
                    outcome.errors[0],
                    OutboundBuildError::CallBudgetExhausted { .. }
                ));
            }
        }
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &TheTradeDeskBidder,
            &request,
            &info(None, None),
            SinkLimits::new(exact, 0),
            1,
            &Policy(true),
        );
        semantic.unwrap();
        assert!(outcome.accepted.is_empty());
        assert_eq!(outcome.errors.len(), 1);
        assert_eq!(remaining, 1);
    }
}
