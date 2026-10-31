use super::super::parameters::{known, object, optional_known, text};
use super::{Request, ser_error};
use serde::{
    Deserializer as _, Serialize, Serializer,
    de::{Error as DeError, MapAccess, SeqAccess, Visitor},
    ser::{SerializeMap, SerializeSeq},
};
use serde_json::{Map, Number, Value, value::RawValue};
use std::{borrow::Cow, fmt, iter};

#[derive(Clone, Copy)]
struct Input<'a>(&'a Value);
fn whole(number: &Number) -> Option<i64> {
    number.as_i64().or_else(|| {
        let float = number.as_f64().filter(|n| n.fract() == 0.0_f64)?;
        if float == 0.0_f64 {
            return Some(0);
        }
        // Recover the integral magnitude without a truncating float-to-int cast.
        let bits = float.abs().to_bits();
        let exponent = u32::try_from((bits >> 52_u32) & 0x7ff)
            .ok()?
            .checked_sub(1023)?;
        let significand = (bits & 0x000f_ffff_ffff_ffff) | 0x0010_0000_0000_0000;
        let magnitude = if exponent <= 52 {
            significand.checked_shr(52_u32.checked_sub(exponent)?)?
        } else {
            significand.checked_mul(1_u64.checked_shl(exponent.checked_sub(52)?)?)?
        };
        let signed = i128::from(magnitude);
        i64::try_from(if float.is_sign_negative() {
            signed.checked_neg()?
        } else {
            signed
        })
        .ok()
    })
}
fn normalized(value: &Value) -> bool {
    match value {
        Value::String(_) | Value::Bool(_) => true,
        Value::Number(number) => whole(number).is_some(),
        Value::Array(values) => {
            values.iter().all(Value::is_string) || values.iter().all(Value::is_boolean)
        }
        Value::Null | Value::Object(_) => false,
    }
}
struct Decimal(i64);
impl Serialize for Decimal {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}
impl Serialize for Input<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Value::String(value) => serializer.collect_seq(iter::once(value)),
            Value::Bool(value) => {
                serializer.collect_seq(iter::once(if *value { "true" } else { "false" }))
            }
            Value::Number(number) => serializer.collect_seq(iter::once(Decimal(
                whole(number).ok_or_else(ser_error::<S::Error>)?,
            ))),
            Value::Array(values) if values.is_empty() => serializer.serialize_none(),
            Value::Array(values) if values.iter().all(Value::is_string) => {
                values.serialize(serializer)
            }
            Value::Array(values) if values.iter().all(Value::is_boolean) => {
                serializer.collect_seq(values.iter().map(|v| {
                    if v.as_bool() == Some(true) {
                        "true"
                    } else {
                        "false"
                    }
                }))
            }
            Value::Null | Value::Object(_) | Value::Array(_) => Err(ser_error()),
        }
    }
}

// Raw App extensions are inspected only under the sink's byte cap. Nested
// values remain borrowed spans; callbacks emit fields without retaining a tree.
fn raw_find<'a>(raw: &'a RawValue, name: &str, fold: bool) -> Result<Option<&'a RawValue>, ()> {
    struct Lookup<'a> {
        name: &'a str,
        fold: bool,
    }
    impl<'de> Visitor<'de> for Lookup<'_> {
        type Value = Option<&'de RawValue>;
        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("object")
        }
        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
            let mut selected = None;
            while let Some(key) = map.next_key::<Cow<'de, str>>()? {
                let value = map.next_value::<&'de RawValue>()?;
                if if self.fold {
                    key.eq_ignore_ascii_case(self.name)
                } else {
                    key == self.name
                } {
                    if self.fold && selected.is_some() {
                        return Err(DeError::custom("ambiguous object field"));
                    }
                    selected = Some(value);
                }
            }
            Ok(selected)
        }
    }
    if raw.get() == "null" {
        return Ok(None);
    }
    serde_json::Deserializer::from_str(raw.get())
        .deserialize_map(Lookup { name, fold })
        .map_err(|_error| ())
}
fn raw_lookup<'a>(raw: &'a RawValue, name: &str) -> Result<Option<&'a RawValue>, ()> {
    raw_find(raw, name, true)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FieldKind {
    Text,
    Bool,
    Whole,
    TextArray,
    BoolArray,
    EmptyArray,
    Skip,
}

fn raw_kind(raw: &RawValue) -> Result<FieldKind, ()> {
    match raw.get().as_bytes().first() {
        Some(b'"') => Ok(FieldKind::Text),
        Some(b't' | b'f') => Ok(FieldKind::Bool),
        Some(b'[') => {
            struct Kinds;
            impl<'de> Visitor<'de> for Kinds {
                type Value = FieldKind;
                fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str("array")
                }
                fn visit_seq<M: SeqAccess<'de>>(self, mut seq: M) -> Result<FieldKind, M::Error> {
                    let mut kind = None;
                    while let Some(value) = seq.next_element::<&'de RawValue>()? {
                        let next = match value.get().as_bytes().first() {
                            Some(b'"') => FieldKind::TextArray,
                            Some(b't' | b'f') => FieldKind::BoolArray,
                            _ => FieldKind::Skip,
                        };
                        kind = Some(match kind {
                            None => next,
                            Some(previous) if previous == next => next,
                            _ => FieldKind::Skip,
                        });
                    }
                    Ok(kind.unwrap_or(FieldKind::EmptyArray))
                }
            }
            serde_json::Deserializer::from_str(raw.get())
                .deserialize_seq(Kinds)
                .map_err(|_error| ())
        }
        Some(b'-' | b'0'..=b'9') => {
            let number = serde_json::from_str::<Number>(raw.get()).map_err(|_error| ())?;
            Ok(if whole(&number).is_some() {
                FieldKind::Whole
            } else {
                FieldKind::Skip
            })
        }
        _ => Ok(FieldKind::Skip),
    }
}
struct RawNormalized<'a>(&'a RawValue);
impl Serialize for RawNormalized<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match raw_kind(self.0).map_err(|()| ser_error::<S::Error>())? {
            FieldKind::Text => {
                let value: Cow<'_, str> =
                    serde_json::from_str(self.0.get()).map_err(|_error| ser_error::<S::Error>())?;
                serializer.collect_seq(iter::once(&*value))
            }
            FieldKind::Bool => serializer.collect_seq(iter::once(if self.0.get() == "true" {
                "true"
            } else {
                "false"
            })),
            FieldKind::Whole => {
                let number: Number =
                    serde_json::from_str(self.0.get()).map_err(|_error| ser_error::<S::Error>())?;
                serializer.collect_seq(iter::once(Decimal(
                    whole(&number).ok_or_else(ser_error::<S::Error>)?,
                )))
            }
            FieldKind::TextArray => self.0.serialize(serializer),
            FieldKind::BoolArray => {
                struct Emit<'a, M: SerializeSeq>(&'a mut M);
                impl<'de, M: SerializeSeq> Visitor<'de> for Emit<'_, M> {
                    type Value = ();
                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        formatter.write_str("boolean array")
                    }
                    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
                        while let Some(value) = seq.next_element::<bool>()? {
                            self.0
                                .serialize_element(if value { "true" } else { "false" })
                                .map_err(|_error| {
                                    DeError::custom("bounded serialization failed")
                                })?;
                        }
                        Ok(())
                    }
                }
                let mut seq = serializer.serialize_seq(None)?;
                serde_json::Deserializer::from_str(self.0.get())
                    .deserialize_seq(Emit(&mut seq))
                    .map_err(|_error| ser_error::<S::Error>())?;
                seq.end()
            }
            FieldKind::EmptyArray => serializer.serialize_none(),
            FieldKind::Skip => Err(ser_error()),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Layer<'a> {
    Map(&'a Map<String, Value>),
    Raw(&'a RawValue),
    Page(&'a str),
}
impl Layer<'_> {
    fn contains(self, key: &str) -> Result<bool, ()> {
        match self {
            Self::Map(fields) => Ok(fields.get(key).is_some_and(normalized)),
            Self::Raw(raw) => raw_find(raw, key, false)?
                .map(|v| raw_kind(v).map(|kind| kind != FieldKind::Skip))
                .transpose()
                .map(|v| v.unwrap_or(false)),
            Self::Page(page) => Ok(key == "page" && !page.is_empty()),
        }
    }
}
struct RawEmit<'a, 'b, M: SerializeMap, F> {
    map: &'a mut M,
    later: F,
    error: &'a mut Option<M::Error>,
    raw: &'b RawValue,
}
impl<'de, M: SerializeMap, F: Fn(&str) -> Result<bool, ()>> Visitor<'de> for RawEmit<'_, '_, M, F> {
    type Value = ();
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("target object")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while let Some(key) = map.next_key::<Cow<'de, str>>()? {
            let value = map.next_value::<&'de RawValue>()?;
            let eligible = raw_kind(value).map_err(|()| DeError::custom("invalid target value"))?
                != FieldKind::Skip;
            let overridden =
                (self.later)(&key).map_err(|()| DeError::custom("invalid target layer"))?;
            let last = raw_find(self.raw, &key, false)
                .map_err(|()| DeError::custom("invalid target object"))?;
            if eligible
                && !overridden
                && last.is_some_and(|last_value| last_value.get().as_ptr() == value.get().as_ptr())
                && let Err(error) = self.map.serialize_entry(&*key, &RawNormalized(value))
            {
                *self.error = Some(error);
                return Err(DeError::custom("bounded serialization failed"));
            }
        }
        Ok(())
    }
}
pub(super) struct Target<'a> {
    pub prior: Option<&'a Map<String, Value>>,
    pub layers: [Option<Layer<'a>>; 4],
    pub slot: &'a str,
    pub dfp: &'a str,
    pub keywords: &'a [Value],
    pub host: Option<(&'a str, &'a str)>,
    pub iab: Option<Segments<'a>>,
}
impl Target<'_> {
    fn reserved(&self, key: &str) -> bool {
        self.host.is_some() && matches!(key, "pbs_login" | "pbs_url" | "pbs_version")
            || !self.slot.is_empty() && key == "pbadslot"
            || self.slot.is_empty() && !self.dfp.is_empty() && key == "dfp_ad_unit_code"
            || !self.keywords.is_empty() && key == "keywords"
            || self
                .iab
                .is_some_and(|segments| segments.iter().next().is_some())
                && key == "iab"
    }
    fn overridden(&self, start: usize, key: &str) -> Result<bool, ()> {
        if self.reserved(key) {
            return Ok(true);
        }
        for layer in self.layers.iter().skip(start).flatten() {
            if layer.contains(key)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
impl Serialize for Target<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        if let Some(prior) = self.prior {
            for (key, value) in prior {
                if !self
                    .overridden(0, key)
                    .map_err(|()| ser_error::<S::Error>())?
                {
                    map.serialize_entry(key, value)?;
                }
            }
        }
        for (index, candidate) in self.layers.iter().enumerate() {
            let Some(layer) = candidate else {
                continue;
            };
            let next = index.checked_add(1).ok_or_else(ser_error::<S::Error>)?;
            match layer {
                Layer::Map(fields) => {
                    for (key, value) in *fields {
                        if normalized(value)
                            && !self
                                .overridden(next, key)
                                .map_err(|()| ser_error::<S::Error>())?
                        {
                            map.serialize_entry(key, &Input(value))?;
                        }
                    }
                }
                Layer::Page(page) => {
                    if !page.is_empty()
                        && !self
                            .overridden(next, "page")
                            .map_err(|()| ser_error::<S::Error>())?
                    {
                        map.serialize_entry("page", &[*page])?;
                    }
                }
                Layer::Raw(raw) => {
                    let mut error = None;
                    let result =
                        serde_json::Deserializer::from_str(raw.get()).deserialize_map(RawEmit {
                            map: &mut map,
                            later: |key: &str| self.overridden(next, key),
                            error: &mut error,
                            raw,
                        });
                    if let Some(failure) = error {
                        return Err(failure);
                    }
                    result.map_err(|_error| ser_error::<S::Error>())?;
                }
            }
        }
        match (self.slot.is_empty(), self.dfp.is_empty()) {
            (false, _) => map.serialize_entry("pbadslot", self.slot)?,
            (true, false) => map.serialize_entry("dfp_ad_unit_code", self.dfp)?,
            (true, true) => {}
        }
        if !self.keywords.is_empty() {
            map.serialize_entry("keywords", self.keywords)?;
        }
        if let Some(segments) = self.iab
            && segments.iter().next().is_some()
        {
            map.serialize_entry("iab", &segments)?;
        }
        if let Some((username, external)) = self.host {
            map.serialize_entry("pbs_login", username)?;
            map.serialize_entry("pbs_url", external)?;
            map.serialize_entry("pbs_version", env!("CARGO_PKG_VERSION"))?;
        }
        map.end()
    }
}
#[derive(Clone, Copy)]
pub(super) struct Segments<'a> {
    pub data: Option<&'a [Value]>,
    pub user: bool,
}
impl Segments<'_> {
    pub(super) fn iter(&self) -> impl Iterator<Item = &str> {
        self.data
            .unwrap_or(&[])
            .iter()
            .filter(move |record| {
                let tax = record
                    .as_object()
                    .and_then(|fields| known(fields, "ext").ok().flatten())
                    .and_then(Value::as_object)
                    .and_then(|ext| known(ext, "segtax").ok().flatten())
                    .and_then(Value::as_i64);
                if self.user {
                    tax == Some(4)
                } else {
                    matches!(tax, Some(1 | 2 | 5 | 6))
                }
            })
            .flat_map(|record| {
                record
                    .get("segment")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
            })
            .filter_map(|segment| match segment.get("id") {
                None | Some(Value::Null) => Some(""),
                Some(Value::String(id)) => Some(id.as_str()),
                Some(_) => None,
            })
    }
}
impl Serialize for Segments<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}

impl Request<'_> {
    pub(super) fn target(&self) -> Result<Target<'_>, ()> {
        let imp_ext = self.imp.ext.as_ref().ok_or(())?;
        let rp = object(known(&imp_ext.other, "rp")?)?;
        let prior = object(optional_known(rp, "target")?)?;
        let data = object(known(&imp_ext.other, "data")?)?;
        let slot = text(optional_known(data, "pbadslot")?)?;
        let adserver = object(optional_known(data, "adserver")?)?;
        let dfp = if text(optional_known(adserver, "name")?)? == "gam" {
            text(optional_known(adserver, "adslot")?)?
        } else {
            ""
        };
        let (inventory, page) = if let Some(site) = &self.original.site {
            let site_data = site
                .ext
                .as_ref()
                .map(|ext| known(&ext.other, "data"))
                .transpose()?
                .flatten();
            (
                object(site_data)?.map(Layer::Map),
                site.page.as_deref().unwrap_or(""),
            )
        } else if let Some(app) = &self.original.app {
            let app_data = if let Some(ext) = &app.ext {
                if ext.get().len() > self.raw_cap {
                    return Err(());
                }
                raw_lookup(ext, "data")?
            } else {
                None
            };
            (
                app_data
                    .filter(|value| value.get() != "null")
                    .map(Layer::Raw),
                "",
            )
        } else {
            return Err(());
        };
        Ok(Target {
            prior,
            layers: [
                self.params.inventory.map(Layer::Map),
                inventory,
                (!page.is_empty()).then_some(Layer::Page(page)),
                data.map(Layer::Map),
            ],
            slot,
            dfp,
            keywords: self.params.keywords,
            host: Some((self.credentials.username(), self.external_url)),
            iab: None,
        })
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use crate::bidders::rubicon::test_support::{banner, body, outbound, request};
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::json;

    #[test]
    fn target_merges_follow_source_precedence_and_reserve_host_fields() {
        let mut imp = banner("i");
        imp["ext"]["rp"] =
            json!({"target":{"prior":["keep"],"winner":["prior"],"opaque":{"keep":true}}});
        imp["ext"]["bidder"]["inventory"] = json!({"winner":["inventory"],"inventory":["keep"],
            "flag":true,"bools":[true,false],"integer":3_i32,"fraction":1.5_f64,"numbers":[1_i32,2_i32],
            "object":{"discard":true}});
        imp["ext"]["bidder"]["keywords"] = json!(["key", "key"]);
        imp["ext"]["data"] = json!({"winner":["impression"],"pbadslot":"slot",
            "adserver":{"name":"gam","adslot":"gam-slot"},"pbs_login":"untrusted"});
        let request: BidRequest = serde_json::from_value(json!({"id":"r","imp":[imp],"site":{
            "page":"https://publisher.example/page","ext":{"data":{"winner":["site"],"site":false}}
        }}))
        .unwrap();
        let before = serde_json::to_value(&request).unwrap();
        let calls = outbound(&request);
        assert_eq!(calls.len(), 1);
        assert_eq!(
            body(&calls[0])["imp"][0]["ext"]["rp"]["target"],
            json!({
                "prior":["keep"],"winner":["impression"],"opaque":{"keep":true},"inventory":["keep"],
                "flag":["true"],"bools":["true","false"],"integer":["3"],"site":["false"],
                "page":["https://publisher.example/page"],"pbadslot":"slot","keywords":["key","key"],
                "pbs_login":"xuser","pbs_url":"http://hosturl.com","pbs_version":env!("CARGO_PKG_VERSION")
            })
        );
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn gam_slot_fallback_is_used_only_without_a_nonempty_pbadslot() {
        for (data, key, value) in [
            (
                json!({"adserver":{"name":"gam","adslot":"unit"}}),
                "dfp_ad_unit_code",
                "unit",
            ),
            (
                json!({"pbadslot":"slot","adserver":{"name":"gam","adslot":"unit"}}),
                "pbadslot",
                "slot",
            ),
        ] {
            let mut imp = banner("i");
            imp["ext"]["data"] = data;
            let calls = outbound(&request(&json!([imp])));
            assert_eq!(calls.len(), 1);
            let target = &body(&calls[0])["imp"][0]["ext"]["rp"]["target"];
            assert_eq!(target[key], value);
            if key == "pbadslot" {
                assert!(target.get("dfp_ad_unit_code").is_none());
            }
        }
    }
}
