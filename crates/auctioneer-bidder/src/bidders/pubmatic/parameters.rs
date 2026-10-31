use crate::bidders::wire_fields::signed_integer;
use crate::{BidderRequestInfo, ConstructionDiagnostic};
use auctioneer_openrtb::{
    ext::request_prebid::ExtRequestPrebid,
    types::{
        bid_request::{BidRequest, Imp},
        field::JsonField,
    },
};
use auctioneer_runtime::bounded_json::JsonFieldShape;
use serde::{Serialize, Serializer, ser::SerializeMap as _};
use serde_json::{Map, Value};
use std::{fmt, iter::once, slice::Iter};

pub(super) fn unique_field<'a>(
    fields: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a Value>, ()> {
    let mut matches = fields
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case(name));
    let value = matches.next().map(|(_, value)| value);
    if matches.next().is_some() {
        return Err(());
    }
    Ok(value)
}

pub(super) fn object(value: Option<&Value>) -> Result<Option<&Map<String, Value>>, ()> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(fields)) => Ok(Some(fields)),
        Some(_) => Err(()),
    }
}

pub(super) fn text(raw: Option<&Value>) -> Result<&str, ()> {
    match raw {
        None | Some(Value::Null) => Ok(""),
        Some(Value::String(value)) => Ok(value),
        Some(_) => Err(()),
    }
}

fn strings(raw: Option<&Value>) -> Result<&[Value], ()> {
    let values = match raw {
        None | Some(Value::Null) => return Ok(&[]),
        Some(Value::Array(values)) => values,
        Some(_) => return Err(()),
    };
    for value in values {
        text(Some(value))?;
    }
    Ok(values)
}

pub(super) fn boolean(raw: Option<&Value>) -> Result<bool, ()> {
    match raw {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(()),
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct Wrapper<'a> {
    profile: i64,
    version: i64,
    biddercode: &'a str,
}

impl Wrapper<'_> {
    fn parse(value: Option<&Value>) -> Result<Option<Self>, ()> {
        let Some(fields) = object(value)? else {
            return Ok(None);
        };
        // Even the overridden source field must have the source's string type.
        text(unique_field(fields, "biddercode")?)?;
        Ok(Some(Self {
            profile: signed_integer(fields, "profile").map_err(|_error| ())?,
            version: signed_integer(fields, "version").map_err(|_error| ())?,
            biddercode: "",
        }))
    }

    fn complete(self) -> bool {
        self.profile != 0 && self.version != 0
    }
}

#[derive(Clone, Copy)]
pub(super) struct RequestExtension<'a> {
    pub wrapper: Option<Wrapper<'a>>,
    pub acat: &'a [Value],
    pub allowed: Option<AllowedBidders<'a>>,
}

type RequestValues<'a> = (Option<&'a Value>, Option<&'a Value>, Option<&'a Value>);

fn prebid_values<'a>(
    typed_prebid: Option<&'a ExtRequestPrebid>,
    raw: Option<&'a Value>,
) -> Result<RequestValues<'a>, ()> {
    if let Some(typed) = typed_prebid {
        if raw.is_some() {
            return Err(());
        }
        let alternate = unique_field(&typed.other, "alternatebiddercodes")?;
        let raw_params = unique_field(&typed.other, "bidderparams")?;
        if raw_params.is_some() && !typed.bidderparams.is_empty() {
            return Err(());
        }
        if let Some(aliases) = unique_field(&typed.other, "aliases")? {
            if !typed.aliases.is_empty() {
                return Err(());
            }
            validate_aliases(Some(aliases))?;
        }
        if let Some(parameter_value) = raw_params {
            let params = object(Some(parameter_value))?;
            return Ok((
                params.and_then(|fields| fields.get("wrapper")),
                params.and_then(|fields| fields.get("acat")),
                alternate,
            ));
        }
        return Ok((
            typed.bidderparams.get("wrapper"),
            typed.bidderparams.get("acat"),
            alternate,
        ));
    }
    let Some(fields) = object(raw)? else {
        return Ok((None, None, None));
    };
    validate_aliases(unique_field(fields, "aliases")?)?;
    let params = object(unique_field(fields, "bidderparams")?)?;
    Ok((
        params.and_then(|parameters| parameters.get("wrapper")),
        params.and_then(|parameters| parameters.get("acat")),
        unique_field(fields, "alternatebiddercodes")?,
    ))
}

fn validate_aliases(raw: Option<&Value>) -> Result<(), ()> {
    if let Some(fields) = object(raw)? {
        for value in fields.values() {
            text(Some(value))?;
        }
    }
    Ok(())
}

impl<'a> RequestExtension<'a> {
    pub(super) fn with_wrapper(self, wrapper: Option<Wrapper<'a>>) -> Self {
        Self { wrapper, ..self }
    }

    pub(super) fn parse(
        request: &'a BidRequest,
        info: &'a BidderRequestInfo<'_>,
    ) -> Result<Self, ()> {
        if matches!(
            info.original_request_ext_shape,
            JsonFieldShape::NonObject | JsonFieldShape::Duplicate
        ) {
            return Err(());
        }
        let Some(extension) = request.ext.as_ref() else {
            return Ok(Self {
                wrapper: None,
                acat: &[],
                allowed: None,
            });
        };
        let raw = unique_field(&extension.other, "prebid")?;
        let (raw_wrapper, acat, alternate) = prebid_values(extension.prebid.as_ref(), raw)?;
        let mut wrapper = Wrapper::parse(raw_wrapper)?.unwrap_or_default();
        wrapper.biddercode = info.bidder.as_str();
        Ok(Self {
            wrapper: Some(wrapper),
            acat: strings(acat)?,
            allowed: AllowedBidders::parse(alternate)?,
        })
    }
}

#[derive(Clone, Copy)]
pub(super) enum AllowedBidders<'a> {
    All,
    Explicit(&'a [Value]),
}

impl<'a> AllowedBidders<'a> {
    fn parse(raw: Option<&'a Value>) -> Result<Option<Self>, ()> {
        let Some(fields) = object(raw)? else {
            return Ok(None);
        };
        let enabled = boolean(unique_field(fields, "enabled")?)?;
        let optional_bidders = object(unique_field(fields, "bidders")?)?;
        let mut selected = Self::Explicit(&[]);
        if let Some(bidders) = optional_bidders {
            for (name, value) in bidders {
                let Some(bidder_fields) = object(Some(value))? else {
                    continue;
                };
                let bidder_enabled = boolean(unique_field(bidder_fields, "enabled")?)?;
                let allowed = unique_field(bidder_fields, "allowedbiddercodes")?;
                let codes = strings(allowed)?;
                if name == "pubmatic" && enabled && bidder_enabled {
                    selected = if allowed.is_none_or(Value::is_null)
                        || (codes.len() == 1 && codes.first().and_then(Value::as_str) == Some("*"))
                    {
                        Self::All
                    } else {
                        Self::Explicit(codes)
                    };
                }
            }
        }
        Ok(Some(selected))
    }
}

#[derive(Clone, Copy)]
pub(super) struct Params<'a> {
    pub publisher: &'a str,
    pub adslot: &'a str,
    pub floor: &'a str,
    pub dctr: &'a str,
    pub zone: &'a str,
    pub wrapper: Option<&'a Value>,
    pub keywords: &'a [Value],
}

impl<'a> Params<'a> {
    fn parse(imp: &'a Imp) -> Result<Self, ()> {
        let ext = imp.ext.as_ref().ok_or(())?;
        let raw = unique_field(&ext.other, "bidder")?;
        let value = match &ext.bidder {
            JsonField::Missing => raw.ok_or(())?,
            JsonField::Null if raw.is_none() => &Value::Null,
            JsonField::Value(value) if raw.is_none() => value,
            JsonField::Null | JsonField::Value(_) => return Err(()),
        };
        let fields = object(Some(value))?;
        let field = |name| {
            fields
                .map(|parameters| unique_field(parameters, name))
                .transpose()
                .map(Option::flatten)
        };
        let keywords = match field("keywords")? {
            None | Some(Value::Null) => &[][..],
            Some(Value::Array(values)) => values.as_slice(),
            Some(_) => return Err(()),
        };
        for keyword in keywords {
            Keyword::parse(keyword)?;
        }
        Ok(Self {
            publisher: text(field("publisherId")?)?,
            adslot: text(field("adSlot")?)?,
            floor: text(field("kadfloor")?)?,
            dctr: text(field("dctr")?)?,
            zone: text(field("pmzoneid")?)?,
            wrapper: field("wrapper")?,
            keywords,
        })
    }
}

#[derive(Clone, Copy)]
struct Keyword<'a> {
    key: &'a str,
    values: &'a [Value],
}

fn next_keyword<'a>(
    keywords: impl IntoIterator<Item = &'a Value>,
    after: Option<&str>,
) -> Option<Keyword<'a>> {
    let mut selected: Option<Keyword<'a>> = None;
    for raw in keywords {
        let Ok(keyword) = Keyword::parse(raw) else {
            continue;
        };
        if keyword.values.is_empty() || after.is_some_and(|lower| keyword.key <= lower) {
            continue;
        }
        if selected
            .as_ref()
            .is_none_or(|previous| keyword.key <= previous.key)
        {
            selected = Some(keyword);
        }
    }
    selected
}

impl<'a> Keyword<'a> {
    fn parse(value: &'a Value) -> Result<Self, ()> {
        let fields = value.as_object().ok_or(())?;
        let key = text(unique_field(fields, "key")?)?;
        Ok(Self {
            key: if key == "pmZoneID" { "pmZoneId" } else { key },
            values: strings(unique_field(fields, "value")?)?,
        })
    }
}

pub(super) struct PreparedImpression<'a> {
    pub imp: &'a Imp,
    pub params: Params<'a>,
    pub tag: Option<&'a str>,
    pub dimensions: Option<(i32, i32)>,
    pub floor: Option<f64>,
    pub extension: ImpressionExtension<'a>,
}

pub(super) struct ImpressionWalk<'a> {
    pub impressions: Iter<'a, Imp>,
    pub wrapper: Option<Wrapper<'a>>,
}

impl<'a> ImpressionWalk<'a> {
    pub(super) fn new(impressions: &'a [Imp], wrapper: Option<Wrapper<'a>>) -> Self {
        Self {
            impressions: impressions.iter(),
            wrapper,
        }
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Iterator's provided combinators must consume the same stateful next implementation"
)]
impl<'a> Iterator for ImpressionWalk<'a> {
    type Item = Result<PreparedImpression<'a>, ConstructionDiagnostic>;

    fn next(&mut self) -> Option<Self::Item> {
        self.impressions.next().map(|imp| {
            let invalid = ConstructionDiagnostic::InvalidImpressionParameters;
            if imp.banner.is_none() && imp.video.is_none() && imp.native.is_none() {
                return Err(ConstructionDiagnostic::UnsupportedMediaType);
            }
            let params = Params::parse(imp).map_err(|()| invalid)?;
            let wrapper = if self.wrapper.is_none_or(|wrapper| !wrapper.complete()) {
                Wrapper::parse(params.wrapper).map_err(|()| invalid)?
            } else {
                None
            };
            let (tag, mut dimensions) = adslot(params.adslot).map_err(|()| invalid)?;
            if let Some(banner) = &imp.banner {
                if dimensions.is_none() {
                    dimensions = match (banner.w, banner.h) {
                        (Some(w), Some(h)) => Some((w, h)),
                        _ => banner.format.first().map(|format| (format.w, format.h)),
                    };
                }
                if dimensions.is_none() {
                    return Err(invalid);
                }
            }
            let extension = ImpressionExtension::parse(imp, params).map_err(|()| invalid)?;
            let floor = params
                .floor
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .map(|value| imp.bidfloor.unwrap_or(0.0_f64).max(value))
                .or(imp.bidfloor);
            if let Some(incoming) = wrapper {
                let target = self.wrapper.get_or_insert_with(Wrapper::default);
                if target.profile == 0 {
                    target.profile = incoming.profile;
                }
                if target.version == 0 {
                    target.version = incoming.version;
                }
            }
            Ok(PreparedImpression {
                imp,
                params,
                tag,
                dimensions,
                floor,
                extension,
            })
        })
    }
}

type AdslotParts<'a> = (Option<&'a str>, Option<(i32, i32)>);

fn adslot(raw_slot: &str) -> Result<AdslotParts<'_>, ()> {
    let slot = raw_slot.trim();
    if slot.is_empty() {
        return Ok((None, None));
    }
    let Some((tag, size)) = slot.split_once('@') else {
        return Ok((Some(slot), None));
    };
    if tag.is_empty() || size.is_empty() || size.contains('@') {
        return Err(());
    }
    let mut sizes = size.split(['x', 'X']);
    let width = sizes
        .next()
        .ok_or(())?
        .trim()
        .parse::<i32>()
        .map_err(|_error| ())?;
    let height = sizes
        .next()
        .ok_or(())?
        .split(':')
        .next()
        .ok_or(())?
        .trim()
        .parse::<i32>()
        .map_err(|_error| ())?;
    if sizes.next().is_some() {
        return Err(());
    }
    Ok((Some(tag.trim()), Some((width, height))))
}

#[derive(Clone, Copy)]
pub(super) struct ImpressionExtension<'a> {
    params: Params<'a>,
    data: Option<&'a Map<String, Value>>,
    adunit: Option<&'a str>,
    ae: i64,
    gpid: &'a str,
    skadn: Option<&'a Value>,
}

impl<'a> ImpressionExtension<'a> {
    fn parse(imp: &'a Imp, params: Params<'a>) -> Result<Self, ()> {
        let ext_fields = &imp.ext.as_ref().ok_or(())?.other;
        let data = unique_field(ext_fields, "data")?.and_then(Value::as_object);
        let gam = data
            .and_then(|fields| fields.get("adserver"))
            .and_then(Value::as_object)
            .filter(|fields| fields.get("name").and_then(Value::as_str) == Some("gam"))
            .and_then(|fields| fields.get("adslot"))
            .and_then(Value::as_str)
            .filter(|slot| !slot.is_empty());
        let has_keyword_adunit = params
            .keywords
            .iter()
            .filter_map(|value| Keyword::parse(value).ok())
            .any(|keyword| keyword.key == "dfp_ad_unit_code" && !keyword.values.is_empty());
        let adunit = if gam.is_some() {
            gam
        } else if has_keyword_adunit {
            None
        } else {
            match data.and_then(|fields| fields.get("pbadslot")) {
                None | Some(Value::Null) => None,
                Some(Value::String(slot)) => Some(slot.as_str()),
                Some(_) => return Err(()),
            }
        };
        Ok(Self {
            params,
            data,
            adunit,
            ae: signed_integer(ext_fields, "ae").map_err(|_error| ())?,
            gpid: text(unique_field(ext_fields, "gpid")?)?,
            skadn: unique_field(ext_fields, "skadn")?,
        })
    }

    fn keyword(self, name: &str) -> Option<Keyword<'a>> {
        self.params
            .keywords
            .iter()
            .rev()
            .filter_map(|value| Keyword::parse(value).ok())
            .find(|keyword| keyword.key == name && !keyword.values.is_empty())
    }

    fn key_val(self) -> KeyVal<'a> {
        KeyVal {
            initial: if self.params.dctr.is_empty() {
                self.keyword("key_val")
                    .map(|keyword| Joined::Values(keyword.values))
            } else {
                Some(Joined::Text(self.params.dctr))
            },
            data: self.data,
        }
    }

    pub(super) fn is_empty(self) -> bool {
        self.params
            .keywords
            .iter()
            .filter_map(|value| Keyword::parse(value).ok())
            .all(|keyword| keyword.values.is_empty())
            && self.params.dctr.is_empty()
            && self.params.zone.is_empty()
            && self.adunit.is_none()
            && self.ae == 0
            && self.gpid.is_empty()
            && self.skadn.is_none()
            && !self.key_val().has_values()
    }
}

#[derive(Clone, Copy)]
enum Joined<'a> {
    Text(&'a str),
    Values(&'a [Value]),
}

impl fmt::Display for Joined<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(value) => f.write_str(value),
            Self::Values(values) => {
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        f.write_str(",")?;
                    }
                    f.write_str(value.as_str().unwrap_or(""))?;
                }
                Ok(())
            }
        }
    }
}

impl Serialize for Joined<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[derive(Clone, Copy)]
struct KeyVal<'a> {
    initial: Option<Joined<'a>>,
    data: Option<&'a Map<String, Value>>,
}

fn targeting(value: &Value) -> bool {
    match value {
        Value::String(_) | Value::Bool(_) | Value::Number(_) => true,
        Value::Array(values) => !values.is_empty() && values.iter().all(Value::is_string),
        Value::Null | Value::Object(_) => false,
    }
}

impl KeyVal<'_> {
    fn has_values(self) -> bool {
        self.initial.is_some()
            || self.data.is_some_and(|data| {
                data.iter()
                    .any(|(key, value)| key != "adserver" && key != "pbadslot" && targeting(value))
            })
    }
}

impl fmt::Display for KeyVal<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut written = false;
        if let Some(initial) = self.initial {
            write!(f, "{initial}")?;
            written = match initial {
                Joined::Text(value) => !value.is_empty(),
                Joined::Values(values) => {
                    values.len() > 1
                        || values.first().is_some_and(|value| {
                            value.as_str().is_some_and(|string| !string.is_empty())
                        })
                }
            };
        }
        if let Some(data) = self.data {
            for (key, raw) in data {
                if key == "adserver" || key == "pbadslot" || !targeting(raw) {
                    continue;
                }
                if written {
                    f.write_str("|")?;
                }
                write!(f, "{}=", key.trim())?;
                match raw {
                    Value::String(value) => f.write_str(value.trim())?,
                    Value::Bool(value) => write!(f, "{value}")?,
                    Value::Number(value) => write!(f, "{}", value.as_f64().ok_or(fmt::Error)?)?,
                    Value::Array(values) => {
                        for (index, value) in values.iter().enumerate() {
                            if index != 0 {
                                f.write_str(",")?;
                            }
                            f.write_str(value.as_str().unwrap_or("").trim())?;
                        }
                    }
                    Value::Null | Value::Object(_) => {}
                }
                written = true;
            }
        }
        Ok(())
    }
}

impl Serialize for KeyVal<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl Serialize for Wrapper<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        if self.profile != 0 {
            map.serialize_entry("profile", &self.profile)?;
        }
        if self.version != 0 {
            map.serialize_entry("version", &self.version)?;
        }
        if !self.biddercode.is_empty() {
            map.serialize_entry("biddercode", self.biddercode)?;
        }
        map.end()
    }
}

struct TrimmedStrings<'a>(&'a [Value]);

impl Serialize for TrimmedStrings<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(
            self.0
                .iter()
                .map(|value| value.as_str().unwrap_or("").trim()),
        )
    }
}

impl Serialize for AllowedBidders<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::All => serializer.collect_seq(["all"]),
            Self::Explicit(values) => serializer.collect_seq(
                once("pubmatic").chain(values.iter().map(|value| value.as_str().unwrap_or(""))),
            ),
        }
    }
}

struct Marketplace<'a>(AllowedBidders<'a>);

impl Serialize for Marketplace<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("allowedbidders", &self.0)?;
        map.end()
    }
}

impl Serialize for RequestExtension<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        if let Some(wrapper) = self.wrapper {
            map.serialize_entry("wrapper", &wrapper)?;
        }
        if !self.acat.is_empty() {
            map.serialize_entry("acat", &TrimmedStrings(self.acat))?;
        }
        if let Some(allowed) = self.allowed {
            map.serialize_entry("marketplace", &Marketplace(allowed))?;
        }
        map.end()
    }
}

impl Serialize for ImpressionExtension<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        let key_val = self.key_val();
        // One scan selects the next key and its last nonempty value. Each
        // emitted key reaches the capped writer before another selection scan.
        let mut after = None;
        while let Some(keyword) = next_keyword(self.params.keywords.iter(), after) {
            after = Some(keyword.key);
            let overlaid = match keyword.key {
                "key_val" => key_val.has_values(),
                "pmZoneId" => !self.params.zone.is_empty(),
                "dfp_ad_unit_code" => self.adunit.is_some(),
                "ae" => self.ae != 0,
                "gpid" => !self.gpid.is_empty(),
                "skadn" => self.skadn.is_some(),
                _ => false,
            };
            if !overlaid {
                map.serialize_entry(keyword.key, &Joined::Values(keyword.values))?;
            }
        }
        if key_val.has_values() {
            map.serialize_entry("key_val", &key_val)?;
        }
        if !self.params.zone.is_empty() {
            map.serialize_entry("pmZoneId", self.params.zone)?;
        }
        if let Some(adunit) = self.adunit {
            map.serialize_entry("dfp_ad_unit_code", adunit)?;
        }
        if self.ae != 0 {
            map.serialize_entry("ae", &self.ae)?;
        }
        if !self.gpid.is_empty() {
            map.serialize_entry("gpid", self.gpid)?;
        }
        if let Some(skadn) = self.skadn {
            map.serialize_entry("skadn", skadn)?;
        }
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use super::super::PubmaticBidder;
    use super::super::tests::{AllowEndpoint, banner, body, drive, info, input};
    use crate::SinkLimits;
    use crate::bidders::harness;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_runtime::bounded_json::JsonFieldShape;
    use serde_json::{Value, json};

    fn assert_local_rejection(impression: &Value) {
        let request = input(&json!([
            impression,
            banner("valid", &json!({"publisherId":"good"})),
        ]));
        let (semantic, outcome, remaining) = drive(&request);
        semantic.unwrap();
        assert!(outcome.errors.is_empty());
        assert_eq!(remaining, 0);
        assert_eq!(outcome.accepted.len(), 1);
        assert_eq!(
            outcome.accepted[0].covered_imp_ids().collect::<Vec<_>>(),
            ["valid"]
        );
        assert_eq!(outcome.diagnostics.iter().count(), 1);
        let body: Value = serde_json::from_slice(outcome.accepted[0].body()).unwrap();
        assert_eq!(body["imp"].as_array().unwrap().len(), 1);
        assert_eq!(body["imp"][0]["id"], json!("valid"));
    }

    #[test]
    fn request_adslot_tag_and_banner_dimensions_follow_separate_source_paths() {
        for (slot, original, expected_tag, expected_size) in [
            (
                "",
                json!({"format":[{"w":300_i32,"h":250_i32}]}),
                "old",
                (300_u32, 250_u32),
            ),
            (
                " \t ",
                json!({"w":728_i32,"h":90_i32}),
                "old",
                (728_u32, 90_u32),
            ),
            (
                " bare-slot \t",
                json!({"format":[{"w":320_i32,"h":50_i32}]}),
                "bare-slot",
                (320_u32, 50_u32),
            ),
            (
                " bare-slot ",
                json!({"w":728_i32,"h":90_i32,"format":[{"w":300_i32,"h":250_i32}]}),
                "bare-slot",
                (728_u32, 90_u32),
            ),
            (
                " sized @ 640 X 480 :1 ",
                json!({"w":728_i32,"h":90_i32}),
                "sized",
                (640_u32, 480_u32),
            ),
            ("sized@300x250", json!({}), "sized", (300_u32, 250_u32)),
            (
                "",
                json!({"w":728_i32,"format":[{"w":320_i32,"h":50_i32},{"w":300_i32,"h":250_i32}]}),
                "old",
                (320_u32, 50_u32),
            ),
            (
                "",
                json!({"h":90_i32,"format":[{"w":320_i32,"h":50_i32}]}),
                "old",
                (320_u32, 50_u32),
            ),
        ] {
            let request = input(&json!([{
                "id":"i", "tagid":"old", "banner":original,
                "ext":{"bidder":{"publisherId":"p","adSlot":slot}},
            }]));
            let body = body(&request);
            assert_eq!(body["imp"][0]["tagid"], json!(expected_tag), "{slot}");
            assert_eq!(
                body["imp"][0]["banner"]["w"],
                json!(expected_size.0),
                "{slot}"
            );
            assert_eq!(
                body["imp"][0]["banner"]["h"],
                json!(expected_size.1),
                "{slot}"
            );
            assert_eq!(
                body["imp"][0]["banner"].get("format"),
                original.get("format")
            );
        }
    }

    #[test]
    fn request_video_adslot_does_not_replace_player_dimensions() {
        let request = input(&json!([{
            "id":"i", "video":{"w":640_i32,"h":480_i32},
            "ext":{"bidder":{"publisherId":"p","adSlot":" video @300x250:1"}},
        }]));
        let body = body(&request);
        assert_eq!(body["imp"][0]["tagid"], json!("video"));
        assert_eq!(body["imp"][0]["video"], json!({"w":640_i32,"h":480_i32}));
        assert!(body["imp"][0].get("banner").is_none());
    }

    #[test]
    fn request_invalid_adslot_is_impression_local() {
        for slot in [
            "slot@",
            "@300x250",
            "slot@300x250@other",
            "slot@300",
            "slot@300x250x1",
            "slot@300x250:suffix",
            "slot@badx250",
            "slot@300xbad",
            "slot@300.5x250",
            "slot@300x250.5",
            "slot@999999999999999999999999x250",
        ] {
            assert_local_rejection(&banner("bad", &json!({"publisherId":"bad","adSlot":slot})));
        }
    }

    #[test]
    fn request_missing_banner_dimensions_without_fallback_refuse_without_panicking() {
        for dimensions in [
            json!({}),
            json!({"w":300_i32}),
            json!({"h":250_i32}),
            json!({"format":[]}),
        ] {
            assert_local_rejection(&json!({
                "id":"bad", "banner":dimensions, "ext":{"bidder":{"publisherId":"bad"}},
            }));
        }
    }

    #[test]
    fn request_wrapper_fills_each_missing_field_from_valid_impressions_only() {
        let request: BidRequest = serde_json::from_value(json!({
            "id":"r", "ext":{"prebid":{"bidderparams":{"wrapper":{"profile":17_i32,"biddercode":"untrusted"}},
                "aliases":{"unrelated-a":"other","unrelated-b":"pubmatic"}}},
            "imp":[
                banner("invalid", &json!({"adSlot":"bad@","wrapper":{"version":99_i32}})),
                banner("first", &json!({"wrapper":{"profile":22_i32,"version":0_i32}})),
                banner("second", &json!({"wrapper":{"profile":33_i32,"version":44_i32,"biddercode":"also-untrusted"}})),
                banner("ignored-wrapper", &json!({"wrapper":"not parsed after both fields are filled"})),
            ],
        })).unwrap();
        let body = body(&request);
        assert_eq!(
            body["ext"],
            json!({"wrapper":{"profile":17_i32,"version":44_i32,"biddercode":"pubmatic-requested"}})
        );
        assert_eq!(body["imp"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn request_complete_root_wrapper_prevents_impression_wrapper_decoding() {
        let request: BidRequest = serde_json::from_value(json!({
            "id":"r", "ext":{"prebid":{"bidderparams":{"wrapper":{"profile":7_i32,"version":8_i32}}}},
            "imp":[banner("i",&json!({"publisherId":"p","wrapper":{"profile":"invalid-but-unused"}}))],
        })).unwrap();
        let body = body(&request);
        assert_eq!(
            body["ext"]["wrapper"],
            json!({"profile":7_i32,"version":8_i32,"biddercode":"pubmatic-requested"})
        );
        assert_eq!(body["imp"][0]["id"], json!("i"));
    }

    #[test]
    fn request_invalid_impression_wrapper_keeps_valid_siblings() {
        for wrapper in [
            json!("secret-wrapper"),
            json!([]),
            json!({"profile":"secret-profile"}),
            json!({"version":1.5_f64}),
        ] {
            assert_local_rejection(&banner(
                "invalid",
                &json!({"publisherId":"bad","wrapper":wrapper}),
            ));
        }
    }

    #[test]
    fn request_absent_extension_remains_distinct_from_empty_extension() {
        let absent = body(&input(&json!([banner("i", &json!({"publisherId":"p"}))])));
        assert_eq!(absent["ext"], json!({}));
        let present: BidRequest = serde_json::from_value(json!({
            "id":"r", "imp":[banner("i",&json!({"publisherId":"p"}))], "ext":{},
        }))
        .unwrap();
        assert_eq!(
            body(&present)["ext"],
            json!({"wrapper":{"biddercode":"pubmatic-requested"}})
        );
    }

    #[test]
    fn request_root_wrapper_acat_and_alternate_codes_fail_before_impressions() {
        let mut first_error = None;
        for prebid in [
            json!({"bidderparams":{"wrapper":"secret-root-wrapper"}}),
            json!({"bidderparams":{"wrapper":{"profile":"secret-profile"}}}),
            json!({"bidderparams":{"wrapper":{"version":2.5_f64}}}),
            json!({"bidderparams":{"acat":"secret-acat"}}),
            json!({"bidderparams":{"acat":["good",{"secret":"value"}]}}),
            json!({"alternatebiddercodes":"secret-alternates"}),
            json!({"alternatebiddercodes":{"enabled":"secret-enabled"}}),
            json!({"alternatebiddercodes":{"bidders":{"pubmatic":{"allowedbiddercodes":[1_i32]}}}}),
        ] {
            let request: BidRequest = serde_json::from_value(json!({
                "id":"r", "ext":{"prebid":prebid},
                "imp":[
                    {"id":"invalid-media","audio":{},"ext":{"bidder":{}}},
                    banner("valid",&json!({"publisherId":"p"})),
                ],
            }))
            .unwrap();
            let (semantic, outcome, remaining) = drive(&request);
            let errors = semantic.unwrap_err();
            assert_eq!(errors.len(), 1);
            assert!(!errors[0].message.contains("secret"));
            if let Some(expected) = &first_error {
                assert_eq!(&errors[0], expected);
            } else {
                first_error = Some(errors[0].clone());
            }
            assert!(outcome.accepted.is_empty());
            assert!(outcome.errors.is_empty());
            assert_eq!(outcome.diagnostics.iter().count(), 0);
            assert_eq!(remaining, 1);
        }
    }

    #[test]
    fn request_invalid_original_root_extension_shape_is_bidder_fatal() {
        for shape in [JsonFieldShape::NonObject, JsonFieldShape::Duplicate] {
            let request = input(&json!([banner("valid", &json!({"publisherId":"p"}))]));
            let mut info = info();
            info.original_request_ext_shape = shape;
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &PubmaticBidder,
                &request,
                &info,
                SinkLimits::new(4096, 1),
                1,
                &AllowEndpoint,
            );
            assert!(semantic.is_err());
            assert!(outcome.accepted.is_empty());
            assert!(outcome.errors.is_empty());
            assert_eq!(remaining, 1);
        }
    }

    #[test]
    fn request_acat_trims_each_string_without_inventing_filtering() {
        let request: BidRequest = serde_json::from_value(json!({
            "id":"r", "imp":[banner("i",&json!({"publisherId":"p"}))],
            "ext":{"prebid":{"bidderparams":{"acat":[" drg \t","dlu","   ","drg"]}}},
        }))
        .unwrap();
        assert_eq!(
            body(&request)["ext"]["acat"],
            json!(["drg", "dlu", "", "drg"])
        );
    }

    #[test]
    fn request_alternate_bidder_codes_preserve_source_disabled_explicit_and_all_forms() {
        for (alternate, expected) in [
            (
                json!({"enabled":false,"bidders":{"pubmatic":{"enabled":true,"allowedbiddercodes":["groupm"]}}}),
                json!(["pubmatic"]),
            ),
            (
                json!({"enabled":true,"bidders":{"pubmatic":{"enabled":false,"allowedbiddercodes":["groupm"]}}}),
                json!(["pubmatic"]),
            ),
            (
                json!({"enabled":true,"bidders":{"pubmatic":{"enabled":true}}}),
                json!(["all"]),
            ),
            (
                json!({"enabled":true,"bidders":{"pubmatic":{"enabled":true,"allowedbiddercodes":null}}}),
                json!(["all"]),
            ),
            (
                json!({"enabled":true,"bidders":{"pubmatic":{"enabled":true,"allowedbiddercodes":["*"]}}}),
                json!(["all"]),
            ),
            (
                json!({"enabled":true,"bidders":{"pubmatic":{"enabled":true,"allowedbiddercodes":[]}}}),
                json!(["pubmatic"]),
            ),
            (
                json!({"enabled":true,"bidders":{"pubmatic":{"enabled":true,"allowedbiddercodes":["groupm","other"]}}}),
                json!(["pubmatic", "groupm", "other"]),
            ),
            (
                json!({"enabled":true,"bidders":{"pubmatic":{"enabled":true,"allowedbiddercodes":["*","other"]}}}),
                json!(["pubmatic", "*", "other"]),
            ),
        ] {
            let request: BidRequest = serde_json::from_value(json!({
                "id":"r", "imp":[banner("i",&json!({"publisherId":"p"}))],
                "ext":{"prebid":{"alternatebiddercodes":alternate}},
            }))
            .unwrap();
            assert_eq!(
                body(&request)["ext"]["marketplace"]["allowedbidders"],
                expected
            );
        }
    }

    #[test]
    fn request_kadfloor_uses_finite_decimal_max_and_ignores_invalid_strings() {
        for (floor, kadfloor, expected) in [
            (Some(0.12_f64), " 0.13 \t", 0.13_f64),
            (Some(0.12_f64), "0.11", 0.12_f64),
            (None, "0.15", 0.15_f64),
            (None, "-1", 0.0_f64),
            (Some(0.12_f64), "1e-1", 0.12_f64),
            (Some(0.12_f64), "not-a-number", 0.12_f64),
            (Some(0.12_f64), "NaN", 0.12_f64),
            (Some(0.12_f64), "Infinity", 0.12_f64),
            (Some(0.12_f64), "-inf", 0.12_f64),
            (Some(0.12_f64), "1e999", 0.12_f64),
        ] {
            let mut imp = banner("i", &json!({"publisherId":"p","kadfloor":kadfloor}));
            if let Some(bid_floor) = floor {
                imp["bidfloor"] = json!(bid_floor);
            }
            let body = body(&input(&json!([imp])));
            assert_eq!(
                body["imp"][0]["bidfloor"]
                    .as_f64()
                    .unwrap_or(0.0_f64)
                    .to_bits(),
                expected.to_bits(),
                "{kadfloor}"
            );
        }
    }

    #[test]
    fn request_keywords_normalize_zone_join_values_and_direct_values_win() {
        for (direct, expected_key_val, expected_zone) in [
            (false, "keyword=one,two", "zone1,zone2"),
            (true, "direct=retained", "direct-zone"),
        ] {
            let mut params = json!({"publisherId":"p","keywords":[
                {"key":"pmZoneID","value":["zone1","zone2"]},
                {"key":"key_val","value":["keyword=one","two"]},
                {"key":"empty","value":[]},
                {"key":"plain","value":[" untrimmed ","value"]},
            ]});
            if direct {
                params["dctr"] = json!("direct=retained");
                params["pmzoneid"] = json!("direct-zone");
            }
            let body = body(&input(&json!([banner("i", &params)])));
            assert_eq!(
                body["imp"][0]["ext"],
                json!({
                    "pmZoneId":expected_zone, "key_val":expected_key_val,"plain":" untrimmed ,value",
                })
            );
        }
    }

    #[test]
    fn request_fpd_prefers_gam_adslot_then_pbadslot_and_formats_typed_values() {
        for (adserver, expected_slot) in [
            (json!({"name":"gam","adslot":"/111/home"}), "/111/home"),
            (json!({"name":"gam","adslot":""}), "/222/fallback"),
            (json!({"name":"other","adslot":"/not-gam"}), "/222/fallback"),
            (json!("malformed-but-ignored"), "/222/fallback"),
        ] {
            let mut imp = banner("i", &json!({"publisherId":"p","dctr":"existing=one"}));
            imp["ext"]["data"] = json!({
                "adserver":adserver,"pbadslot":"/222/fallback", " age ":25_i32,
                "rate":0.15_f64,"flag":true," category ":" cinema ","sports":[" rugby "," cricket"],
            });
            let body = body(&input(&json!([imp])));
            assert_eq!(
                body["imp"][0]["ext"]["dfp_ad_unit_code"],
                json!(expected_slot)
            );
            let key_val = body["imp"][0]["ext"]["key_val"].as_str().unwrap();
            assert!(key_val.starts_with("existing=one|"));
            let mut pairs: Vec<_> = key_val.split('|').collect();
            pairs.sort_unstable();
            assert_eq!(
                pairs,
                [
                    "age=25",
                    "category=cinema",
                    "existing=one",
                    "flag=true",
                    "rate=0.15",
                    "sports=rugby,cricket"
                ]
            );
        }
    }

    #[test]
    fn request_fpd_nonobject_and_unsupported_targeting_do_not_panic_or_leak() {
        for data in [
            json!("secret-invalid-data"),
            json!(["secret-invalid-data"]),
            json!({"category":" movies ","mixed":[1_i32,"secret-mixed"],"object":{"secret":"value"},"empty":[],"null":null}),
        ] {
            let mut imp = banner("i", &json!({"publisherId":"p"}));
            imp["ext"]["data"] = data.clone();
            let body = body(&input(&json!([imp])));
            if data.is_object() {
                let pairs: Vec<_> = body["imp"][0]["ext"]["key_val"]
                    .as_str()
                    .unwrap()
                    .split('|')
                    .filter(|part| !part.is_empty())
                    .collect();
                assert_eq!(pairs, ["category=movies"]);
            } else {
                assert!(body["imp"][0].get("ext").is_none());
            }
            assert!(!serde_json::to_string(&body).unwrap().contains("secret"));
        }
    }

    #[test]
    fn request_preserves_scoped_ae_gpid_and_skadn_without_forwarding_bidder_params() {
        let mut imp = banner("i", &json!({"publisherId":"p","unrelated":"not-forwarded"}));
        imp["ext"]["ae"] = json!(1_i32);
        imp["ext"]["gpid"] = json!("scoped-gpid");
        imp["ext"]["skadn"] = json!({"version":"4.0","skadnetids":["allowed-id"]});
        imp["ext"]["unrelated"] = json!({"secret":"not-forwarded"});
        assert_eq!(
            body(&input(&json!([imp])))["imp"][0]["ext"],
            json!({
                "ae":1_i32,"gpid":"scoped-gpid","skadn":{"version":"4.0","skadnetids":["allowed-id"]},
            })
        );
        let scrubbed = body(&input(&json!([banner("i", &json!({"publisherId":"p"}))])));
        assert!(scrubbed["imp"][0].get("ext").is_none());
    }

    #[test]
    fn request_single_case_insensitive_source_fields_are_accepted() {
        for (publisher, adslot, floor, wrapper, profile, version, keywords, key, value) in [
            (
                "publisherId",
                "adSlot",
                "kadfloor",
                "wrapper",
                "profile",
                "version",
                "keywords",
                "key",
                "value",
            ),
            (
                "PublisherId",
                "AdSlot",
                "Kadfloor",
                "Wrapper",
                "Profile",
                "Version",
                "Keywords",
                "Key",
                "Value",
            ),
            (
                "PUBLISHERID",
                "ADSLOT",
                "KADFLOOR",
                "WRAPPER",
                "PROFILE",
                "VERSION",
                "KEYWORDS",
                "KEY",
                "VALUE",
            ),
        ] {
            let request: BidRequest = serde_json::from_value(json!({
                "id":"r","site":{},"imp":[banner("i",&json!({
                    publisher:" p ",adslot:" slot@640x480 ",floor:"0.25",
                    wrapper:{profile:3_i32,version:4_i32},keywords:[{key:"plain",value:["a","b"]}],
                }))],
            }))
            .unwrap();
            let body = body(&request);
            assert_eq!(body["site"]["publisher"]["id"], json!("p"));
            assert_eq!(body["imp"][0]["tagid"], json!("slot"));
            assert_eq!(body["imp"][0]["banner"]["w"], json!(640_i32));
            assert_eq!(body["imp"][0]["bidfloor"], json!(0.25_f64));
            assert_eq!(body["imp"][0]["ext"]["plain"], json!("a,b"));
            assert_eq!(body["ext"]["wrapper"]["profile"], json!(3_i32));
            assert_eq!(body["ext"]["wrapper"]["version"], json!(4_i32));
        }
    }

    #[test]
    fn request_competing_source_field_variants_reject_only_the_affected_impression() {
        for params in [
            json!({"publisherId":"same","PUBLISHERID":"same"}),
            json!({"adSlot":"slot","ADSLOT":"slot"}),
            json!({"kadfloor":"1","Kadfloor":null}),
            json!({"wrapper":{"profile":1_i32,"Profile":1_i32}}),
            json!({"wrapper":{"version":1_i32,"Version":null}}),
            json!({"dctr":"one","DCTR":"two"}),
            json!({"pmzoneid":"one","PMZONEID":"two"}),
            json!({"keywords":[{"key":"one","Key":"two","value":["v"]}]}),
            json!({"keywords":[{"key":"one","value":["v"],"Value":["v"]}]}),
        ] {
            assert_local_rejection(&banner("ambiguous", &params));
        }
    }

    #[test]
    fn request_root_wrapper_competing_variants_are_bidder_fatal() {
        for wrapper in [
            json!({"profile":1_i32,"Profile":1_i32}),
            json!({"version":null,"Version":2_i32}),
        ] {
            let request: BidRequest = serde_json::from_value(json!({
                "id":"r","ext":{"prebid":{"bidderparams":{"wrapper":wrapper}}},
                "imp":[banner("valid",&json!({"publisherId":"p"}))],
            }))
            .unwrap();
            let (semantic, outcome, remaining) = drive(&request);
            assert!(semantic.is_err());
            assert!(outcome.accepted.is_empty());
            assert!(outcome.errors.is_empty());
            assert_eq!(remaining, 1);
        }
    }

    #[test]
    fn request_invalid_parameter_types_are_impression_local() {
        for params in [
            json!(false),
            json!([]),
            json!("secret-params"),
            json!({"publisherId":1_i32}),
            json!({"adSlot":false}),
            json!({"kadfloor":0.25_f64}),
            json!({"dctr":[]}),
            json!({"pmzoneid":true}),
            json!({"keywords":"secret-keywords"}),
            json!({"keywords":["bad"]}),
            json!({"keywords":[{"key":5_i32,"value":["v"]}]}),
            json!({"keywords":[{"key":"k","value":[1_i32]}]}),
        ] {
            assert_local_rejection(&banner("invalid", &params));
        }
    }

    #[test]
    fn request_keyword_adunit_precedes_pbadslot_but_not_a_nonempty_gam_slot() {
        for (keywords, adserver, expected) in [
            (json!(["/keyword"]), json!(null), "/keyword"),
            (json!(["", "tail"]), json!(null), ",tail"),
            (json!([""]), json!(null), ""),
            (json!([]), json!(null), "/fallback"),
            (
                json!(["/keyword"]),
                json!({"name":"gam","adslot":"/gam"}),
                "/gam",
            ),
            (
                json!(["/keyword"]),
                json!({"name":"gam","adslot":""}),
                "/keyword",
            ),
        ] {
            let mut impression = banner(
                "i",
                &json!({"publisherId":"p","keywords":[
                    {"key":"dfp_ad_unit_code","value":keywords},
                ]}),
            );
            impression["ext"]["data"] = json!({"pbadslot":"/fallback","adserver":adserver});
            assert_eq!(
                body(&input(&json!([impression])))["imp"][0]["ext"]["dfp_ad_unit_code"],
                expected
            );
        }
    }

    #[test]
    fn request_fpd_numeric_scalars_do_not_invent_integral_fraction_suffixes() {
        for (value, expected) in [
            (json!(1_i64), "score=1"),
            (json!(1.0_f64), "score=1"),
            (json!(-1.0_f64), "score=-1"),
            (json!(0.5_f64), "score=0.5"),
        ] {
            let mut impression = banner("i", &json!({"publisherId":"p"}));
            impression["ext"]["data"] = json!({"score":value});
            assert_eq!(
                body(&input(&json!([impression])))["imp"][0]["ext"]["key_val"],
                expected
            );
        }
    }

    #[test]
    fn request_keyword_selection_takes_one_pass_before_the_next_emitted_key() {
        use super::next_keyword;
        use std::cell::Cell;

        for unique in [8_usize, 128, 8192] {
            let mut keywords = vec![json!({"key":"empty","value":[]}); unique];
            for pass in ["first", "last"] {
                keywords.extend((0..unique).map(|index| {
                    json!({
                        "key":format!("key-{index:05}"),"value":[pass],
                    })
                }));
            }
            let visited = Cell::new(0_usize);
            let first = next_keyword(
                keywords
                    .iter()
                    .inspect(|_item| visited.set(visited.get() + 1)),
                None,
            )
            .unwrap();
            assert_eq!(first.key, "key-00000");
            assert_eq!(first.values, [json!("last")]);
            assert_eq!(visited.get(), keywords.len());
            visited.set(0);
            let second = next_keyword(
                keywords
                    .iter()
                    .inspect(|_item| visited.set(visited.get() + 1)),
                Some(first.key),
            )
            .unwrap();
            assert_eq!(second.key, "key-00001");
            assert_eq!(second.values, [json!("last")]);
            assert_eq!(visited.get(), keywords.len());
        }
    }
}
