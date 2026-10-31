//! Fixed, bounded debug projections; callers supply capture permission separately.

use crate::{BodyWriteError, CappedWriter};
use auctioneer_runtime::bounded_json::count_json_nodes;
use auctioneer_runtime::debug_budget::{
    MAX_DEBUG_BODY_DEPTH, MAX_DEBUG_BODY_NODES, MAX_DEBUG_INPUT_BYTES,
    MAX_DEBUG_SANITIZED_BODY_BYTES,
};
use serde::de::{DeserializeSeed, Error as DeError, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::value::RawValue;
use std::{fmt, str};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugBodySchema {
    Unsupported,
    OpenRtb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugBodyEligibility {
    SingleTextValue,
    Ineligible,
}

impl DebugBodyEligibility {
    /// Classify complete original evidence, including non-text header values.
    #[must_use]
    pub fn from_complete_headers<'a, H>(headers: H) -> Self
    where
        H: IntoIterator<Item = (&'a str, Option<&'a str>)>,
    {
        let mut seen = false;
        let mut single_text = false;
        for (name, value) in headers {
            if name.eq_ignore_ascii_case("content-type") {
                if seen {
                    return Self::Ineligible;
                }
                seen = true;
                single_text = value.is_some();
            }
        }
        if single_text {
            Self::SingleTextValue
        } else {
            Self::Ineligible
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugBodyStatus {
    Empty,
    Unavailable,
    OmittedType,
    OmittedSize,
    OmittedStructure,
    OmittedPolicy,
    Captured,
}

#[derive(Debug, PartialEq, Eq)]
pub struct DebugBodyProjection {
    pub body_status: DebugBodyStatus,
    pub body: Option<String>,
}

/// Accept only `application/json`, with parameters parsed by the locked MIME parser.
/// The byte bound is checked before the parser allocates source/parameter storage.
#[must_use]
pub fn is_debug_json_content_type(value: &str) -> bool {
    within_input_cap(value.len())
        && value.parse::<mime::Mime>().is_ok_and(|mime| {
            mime.type_() == mime::APPLICATION
                && mime.subtype() == mime::JSON
                && mime.suffix().is_none()
        })
}

/// Project borrowed request bytes after the caller has authorized capture.
#[must_use]
pub fn project_request_body<'a, H>(
    schema: DebugBodySchema,
    body: &[u8],
    headers: H,
    max_nodes: u32,
) -> DebugBodyProjection
where
    H: IntoIterator<Item = (&'a str, &'a str)>,
{
    if body.is_empty() {
        return omitted(DebugBodyStatus::Empty);
    }
    let mut mime = MimeInput::default();
    let mut count = 0_u8;
    for (name, value) in headers {
        if name.eq_ignore_ascii_case("content-type") {
            count = count.saturating_add(1);
            mime.observe(value);
        }
    }
    let eligibility = if count == 1 {
        DebugBodyEligibility::SingleTextValue
    } else {
        DebugBodyEligibility::Ineligible
    };
    project(schema, body, mime, eligibility, max_nodes, Context::Request)
}

pub(crate) fn project_response_body(
    schema: DebugBodySchema,
    body: &[u8],
    headers: &[(String, String)],
    eligibility: DebugBodyEligibility,
    max_nodes: u32,
) -> DebugBodyProjection {
    if body.is_empty() {
        return omitted(DebugBodyStatus::Empty);
    }
    let mut mime = MimeInput::default();
    for (name, value) in headers {
        if name.eq_ignore_ascii_case("content-type") {
            mime.observe(value);
        }
    }
    // Only the sealed owner can supply original eligibility. Normalized headers
    // provide MIME input, never evidence that an original singleton existed.
    project(
        schema,
        body,
        mime,
        eligibility,
        max_nodes,
        Context::Response,
    )
}

#[derive(Clone, Copy, Default)]
struct MimeInput<'a> {
    value: Option<&'a str>,
    bytes: usize,
}

impl<'a> MimeInput<'a> {
    fn observe(&mut self, value: &'a str) {
        self.value.get_or_insert(value);
        self.bytes = self.bytes.saturating_add(value.len());
    }
}

fn within_input_cap(bytes: usize) -> bool {
    u64::try_from(bytes).is_ok_and(|length| length <= MAX_DEBUG_INPUT_BYTES)
}

fn omitted(body_status: DebugBodyStatus) -> DebugBodyProjection {
    DebugBodyProjection {
        body_status,
        body: None,
    }
}

fn project(
    schema: DebugBodySchema,
    body: &[u8],
    mime: MimeInput<'_>,
    eligibility: DebugBodyEligibility,
    max_nodes: u32,
    context: Context,
) -> DebugBodyProjection {
    project_with_allocator(
        schema,
        body,
        mime,
        eligibility,
        max_nodes,
        context,
        Output::new,
    )
}

fn project_with_allocator(
    schema: DebugBodySchema,
    body: &[u8],
    mime: MimeInput<'_>,
    eligibility: DebugBodyEligibility,
    max_nodes: u32,
    context: Context,
    allocate: fn() -> Result<Output, ()>,
) -> DebugBodyProjection {
    if body.is_empty() {
        return omitted(DebugBodyStatus::Empty);
    }
    let Ok(output) = allocate() else {
        return omitted(DebugBodyStatus::Unavailable);
    };
    if schema == DebugBodySchema::Unsupported {
        return omitted(DebugBodyStatus::OmittedType);
    }
    if !within_input_cap(body.len()) || !within_input_cap(mime.bytes) {
        return omitted(DebugBodyStatus::OmittedSize);
    }
    if eligibility != DebugBodyEligibility::SingleTextValue
        || !mime.value.is_some_and(is_debug_json_content_type)
        || str::from_utf8(body).is_err()
    {
        return omitted(DebugBodyStatus::OmittedType);
    }
    if count_json_nodes(body, max_nodes.min(MAX_DEBUG_BODY_NODES)).is_err() {
        return omitted(DebugBodyStatus::OmittedStructure);
    }
    let mut walk = Walk {
        output,
        invalid_type: false,
    };
    let mut deserializer = serde_json::Deserializer::from_slice(body);
    let projected = Seed {
        walk: &mut walk,
        rule: Rule::Object(context),
        depth: 0,
    }
    .deserialize(&mut deserializer);
    if walk.output.unavailable {
        return omitted(DebugBodyStatus::Unavailable);
    }
    if projected.is_err() || deserializer.end().is_err() {
        return omitted(DebugBodyStatus::OmittedStructure);
    }
    if walk.invalid_type {
        return omitted(DebugBodyStatus::OmittedType);
    }
    if !projected.unwrap_or(false) {
        return omitted(DebugBodyStatus::OmittedPolicy);
    }
    if walk.output.over_cap {
        return omitted(DebugBodyStatus::OmittedSize);
    }
    match String::from_utf8(walk.output.bytes) {
        Ok(sanitized) => DebugBodyProjection {
            body_status: DebugBodyStatus::Captured,
            body: Some(sanitized),
        },
        Err(_) => omitted(DebugBodyStatus::Unavailable),
    }
}

struct Output {
    bytes: Vec<u8>,
    cap: usize,
    over_cap: bool,
    unavailable: bool,
}

impl Output {
    fn new() -> Result<Self, ()> {
        let cap = usize::try_from(MAX_DEBUG_SANITIZED_BODY_BYTES).map_err(|_error| ())?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(cap).map_err(|_error| ())?;
        Ok(Self {
            bytes,
            cap,
            over_cap: false,
            unavailable: false,
        })
    }

    fn checkpoint(&self) -> (usize, bool) {
        (self.bytes.len(), self.over_cap)
    }

    fn rollback(&mut self, (len, over_cap): (usize, bool)) {
        self.bytes.truncate(len);
        self.over_cap = over_cap;
    }

    fn write(&mut self, bytes: &[u8]) {
        if self.over_cap || self.unavailable {
            return;
        }
        let result = CappedWriter::new(&mut self.bytes, self.cap)
            .and_then(|mut writer| writer.write_all(bytes));
        self.record(result);
    }

    fn json<T: Serialize>(&mut self, value: &T) {
        if self.over_cap || self.unavailable {
            return;
        }
        let result = CappedWriter::new(&mut self.bytes, self.cap)
            .and_then(|mut writer| writer.write_json(value));
        self.record(result);
    }

    fn record(&mut self, result: Result<(), BodyWriteError>) {
        match result {
            Ok(()) => {}
            Err(BodyWriteError::OverCap { .. }) => self.over_cap = true,
            Err(BodyWriteError::Serialize | BodyWriteError::UnderCapacity { .. }) => {
                self.unavailable = true;
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Context {
    Request,
    Imp,
    Banner,
    Format,
    Device,
    Regs,
    RegsExt,
    Response,
    Seat,
    Bid,
    Currency,
}

#[derive(Clone, Copy)]
enum Number {
    Auction,
    Flag,
    Duration,
    Dimension,
    Position,
    Price,
}

#[derive(Clone, Copy)]
enum Rule {
    Ignore,
    Mask,
    Object(Context),
    Array(Context),
    Number(Number),
    Currency,
}

fn rule(context: Context, key: &str) -> Rule {
    match (context, key) {
        (Context::Request, "id" | "site" | "app" | "dooh" | "source" | "user" | "ext")
        | (Context::Imp, "id" | "tagid" | "ext")
        | (
            Context::Device,
            "ua" | "sua" | "ip" | "ipv6" | "ifa" | "geo" | "dpidsha1" | "dpidmd5" | "didsha1"
            | "didmd5" | "macsha1" | "macmd5" | "ext",
        )
        | (Context::Regs, "gpp" | "us_privacy")
        | (Context::RegsExt, "gpp" | "us_privacy" | "consent")
        | (Context::Response, "id" | "ext")
        | (Context::Seat, "seat")
        | (
            Context::Bid,
            "id" | "impid" | "crid" | "adid" | "dealid" | "adm" | "nurl" | "burl" | "lurl" | "ext",
        ) => Rule::Mask,
        (Context::Request, "at") => Rule::Number(Number::Auction),
        (Context::Request, "test" | "allimps")
        | (Context::Device, "dnt" | "lmt")
        | (Context::Regs, "gdpr" | "coppa")
        | (Context::RegsExt, "gdpr")
        | (Context::Seat, "group") => Rule::Number(Number::Flag),
        (Context::Request, "tmax") => Rule::Number(Number::Duration),
        (Context::Banner | Context::Format | Context::Bid, "w" | "h") => {
            Rule::Number(Number::Dimension)
        }
        (Context::Banner, "pos") => Rule::Number(Number::Position),
        (Context::Bid, "price") => Rule::Number(Number::Price),
        (Context::Request, "imp") => Rule::Array(Context::Imp),
        (Context::Request, "cur") => Rule::Array(Context::Currency),
        (Context::Request, "device") => Rule::Object(Context::Device),
        (Context::Request, "regs") => Rule::Object(Context::Regs),
        (Context::Imp, "banner") => Rule::Object(Context::Banner),
        (Context::Banner, "format") => Rule::Array(Context::Format),
        (Context::Regs, "ext") => Rule::Object(Context::RegsExt),
        (Context::Response, "cur") => Rule::Currency,
        (Context::Response, "seatbid") => Rule::Array(Context::Seat),
        (Context::Seat, "bid") => Rule::Array(Context::Bid),
        _ => Rule::Ignore,
    }
}

struct Walk {
    output: Output,
    invalid_type: bool,
}

impl Walk {
    fn number(&mut self, raw: &RawValue, domain: Number) -> bool {
        if !matches!(raw.get().as_bytes().first(), Some(b'-' | b'0'..=b'9')) {
            self.invalid_type = true;
            return false;
        }
        if matches!(domain, Number::Price) {
            // f64 underflow can hide a negative nonzero significand as -0.
            let negative_nonzero = raw.get().starts_with('-')
                && raw
                    .get()
                    .bytes()
                    .take_while(|byte| !matches!(byte, b'e' | b'E'))
                    .any(|byte| matches!(byte, b'1'..=b'9'));
            if let Ok(value) = serde_json::from_str::<f64>(raw.get())
                && value.is_finite()
                && value >= 0.0_f64
                && !negative_nonzero
            {
                self.output.write(raw.get().as_bytes());
                return true;
            }
        } else {
            return self.integer(raw, domain);
        }
        self.invalid_type = true;
        false
    }

    fn integer(&mut self, raw: &RawValue, domain: Number) -> bool {
        if let Ok(value) = serde_json::from_str::<u64>(raw.get()) {
            let valid = match domain {
                Number::Auction => matches!(value, 1 | 2 | 500..=0x7FFF_FFFF),
                Number::Flag => value <= 1,
                Number::Duration => i64::try_from(value).is_ok(),
                Number::Dimension => i32::try_from(value).is_ok(),
                Number::Position => value <= 7,
                Number::Price => false,
            };
            if valid {
                self.output.json(&value);
                return true;
            }
        }
        self.invalid_type = true;
        false
    }
}

struct Seed<'a> {
    walk: &'a mut Walk,
    rule: Rule,
    depth: u32,
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = bool;

    fn deserialize<D>(self, deserializer: D) -> Result<bool, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = <&RawValue>::deserialize(deserializer)?;
        // RawValue delegates all syntax to serde and borrows the original span.
        // Dispatch only on its validated leading token, never scan JSON ourselves.
        // This also avoids converting enormous numbers in ignored/masked scalars.
        let token = raw.get().as_bytes().first().copied();
        let object = token == Some(b'{');
        let array = token == Some(b'[');
        let compatible = matches!(
            (self.rule, object, array),
            (Rule::Ignore | Rule::Mask, _, _)
                | (Rule::Object(_), true, _)
                | (Rule::Array(_), _, true)
                | (Rule::Number(_) | Rule::Currency, false, false)
        );
        if !compatible {
            self.walk.invalid_type = true;
        }
        let mut retained = false;
        if object || array {
            let depth = self.depth.saturating_add(1);
            if depth > MAX_DEBUG_BODY_DEPTH {
                return Err(D::Error::custom("debug projection depth limit"));
            }
            let container_rule = if compatible && !matches!(self.rule, Rule::Mask) {
                self.rule
            } else {
                Rule::Ignore
            };
            let mut parser = serde_json::Deserializer::from_str(raw.get());
            let visitor = Container {
                walk: self.walk,
                rule: container_rule,
                depth,
            };
            retained = if object {
                parser.deserialize_map(visitor)
            } else {
                parser.deserialize_seq(visitor)
            }
            .map_err(|_error| D::Error::custom("debug projection structure"))?;
        }
        match self.rule {
            Rule::Mask => {
                self.walk.output.write(b"\"[REDACTED]\"");
                Ok(true)
            }
            Rule::Number(domain) if !object && !array => Ok(self.walk.number(raw, domain)),
            Rule::Currency if !object && !array => match serde_json::from_str::<Usd>(raw.get()) {
                Ok(Usd(true)) => {
                    self.walk.output.write(b"\"USD\"");
                    Ok(true)
                }
                Ok(Usd(false)) => Ok(false),
                Err(_) => {
                    self.walk.invalid_type = true;
                    Ok(false)
                }
            },
            Rule::Ignore | Rule::Object(_) | Rule::Array(_) | Rule::Number(_) | Rule::Currency => {
                Ok(retained)
            }
        }
    }
}

struct Container<'a> {
    walk: &'a mut Walk,
    rule: Rule,
    depth: u32,
}

#[expect(
    clippy::missing_trait_methods,
    reason = "only deserialize_map/seq call this visitor; other defaults reject types, including serde's private hook"
)]
impl<'de> Visitor<'de> for Container<'_> {
    type Value = bool;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a debug projection container")
    }

    fn visit_map<A>(self, mut map: A) -> Result<bool, A::Error>
    where
        A: MapAccess<'de>,
    {
        let checkpoint = self.walk.output.checkpoint();
        let context = if let Rule::Object(object_context) = self.rule {
            Some(object_context)
        } else {
            None
        };
        if context.is_some() {
            self.walk.output.write(b"{");
        }
        let mut retained = false;
        while let Some(Key(key)) = map.next_key::<Key>()? {
            let child_rule = context.map_or(Rule::Ignore, |parent| rule(parent, key));
            let child_checkpoint = self.walk.output.checkpoint();
            if !matches!(child_rule, Rule::Ignore) {
                if retained {
                    self.walk.output.write(b",");
                }
                self.walk.output.json(&key);
                self.walk.output.write(b":");
            }
            let child = map.next_value_seed(Seed {
                walk: self.walk,
                rule: child_rule,
                depth: self.depth,
            })?;
            if child {
                retained = true;
            } else {
                self.walk.output.rollback(child_checkpoint);
            }
        }
        if retained {
            self.walk.output.write(b"}");
        } else {
            self.walk.output.rollback(checkpoint);
        }
        Ok(retained)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<bool, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let checkpoint = self.walk.output.checkpoint();
        let child_rule = match self.rule {
            Rule::Array(Context::Currency) => Rule::Currency,
            Rule::Array(context) => Rule::Object(context),
            Rule::Ignore | Rule::Mask | Rule::Object(_) | Rule::Number(_) | Rule::Currency => {
                Rule::Ignore
            }
        };
        if !matches!(child_rule, Rule::Ignore) {
            self.walk.output.write(b"[");
        }
        let mut retained = false;
        loop {
            let child_checkpoint = self.walk.output.checkpoint();
            if retained {
                self.walk.output.write(b",");
            }
            match seq.next_element_seed(Seed {
                walk: self.walk,
                rule: child_rule,
                depth: self.depth,
            })? {
                Some(true) => retained = true,
                Some(false) => self.walk.output.rollback(child_checkpoint),
                None => {
                    self.walk.output.rollback(child_checkpoint);
                    break;
                }
            }
        }
        if retained {
            self.walk.output.write(b"]");
        } else {
            self.walk.output.rollback(checkpoint);
        }
        Ok(retained)
    }
}

// Keys are immediately mapped to fixed spellings, never retained from the body.
struct Key(&'static str);

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(KeyVisitor)
    }

    fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        *place = Self::deserialize(deserializer)?;
        Ok(())
    }
}

struct KeyVisitor;

#[expect(
    clippy::missing_trait_methods,
    reason = "deserialize_str calls visit_str or its forwarding defaults; other types and serde's private hook stay rejected"
)]
impl Visitor<'_> for KeyVisitor {
    type Value = Key;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an object key")
    }
    fn visit_str<E>(self, v: &str) -> Result<Key, E>
    where
        E: DeError,
    {
        let key = match v {
            "id" => "id",
            "site" => "site",
            "app" => "app",
            "dooh" => "dooh",
            "source" => "source",
            "user" => "user",
            "ext" => "ext",
            "tagid" => "tagid",
            "ua" => "ua",
            "sua" => "sua",
            "ip" => "ip",
            "ipv6" => "ipv6",
            "ifa" => "ifa",
            "geo" => "geo",
            "dpidsha1" => "dpidsha1",
            "dpidmd5" => "dpidmd5",
            "didsha1" => "didsha1",
            "didmd5" => "didmd5",
            "macsha1" => "macsha1",
            "macmd5" => "macmd5",
            "gpp" => "gpp",
            "us_privacy" => "us_privacy",
            "consent" => "consent",
            "seat" => "seat",
            "impid" => "impid",
            "crid" => "crid",
            "adid" => "adid",
            "dealid" => "dealid",
            "adm" => "adm",
            "nurl" => "nurl",
            "burl" => "burl",
            "lurl" => "lurl",
            "at" => "at",
            "test" => "test",
            "allimps" => "allimps",
            "dnt" => "dnt",
            "lmt" => "lmt",
            "gdpr" => "gdpr",
            "coppa" => "coppa",
            "group" => "group",
            "tmax" => "tmax",
            "w" => "w",
            "h" => "h",
            "pos" => "pos",
            "price" => "price",
            "imp" => "imp",
            "cur" => "cur",
            "device" => "device",
            "regs" => "regs",
            "banner" => "banner",
            "format" => "format",
            "seatbid" => "seatbid",
            "bid" => "bid",
            _ => "",
        };
        Ok(Key(key))
    }
}

struct Usd(bool);

impl<'de> Deserialize<'de> for Usd {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(UsdVisitor)
    }

    fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        *place = Self::deserialize(deserializer)?;
        Ok(())
    }
}

struct UsdVisitor;

#[expect(
    clippy::missing_trait_methods,
    reason = "deserialize_str calls visit_str or its forwarding defaults; other types and serde's private hook stay rejected"
)]
impl Visitor<'_> for UsdVisitor {
    type Value = Usd;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a currency string")
    }
    fn visit_str<E>(self, v: &str) -> Result<Usd, E>
    where
        E: DeError,
    {
        Ok(Usd(v == "USD"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::iter;

    const JSON: [(&str, &str); 1] = [("Content-Type", "application/json")];

    fn request(value: &Value) -> DebugBodyProjection {
        project_request_body(
            DebugBodySchema::OpenRtb,
            &serde_json::to_vec(value).unwrap(),
            JSON,
            4096,
        )
    }

    fn response(value: &Value) -> DebugBodyProjection {
        project_response_body(
            DebugBodySchema::OpenRtb,
            &serde_json::to_vec(value).unwrap(),
            &[("content-type".into(), "application/json".into())],
            DebugBodyEligibility::SingleTextValue,
            4096,
        )
    }

    fn price_wire(price: &str) -> String {
        // Exact numeric spelling is the assertion; Value loses underflow and signed zero.
        format!(r#"{{"seatbid":[{{"bid":[{{"price":{price}}}]}}]}}"#)
    }

    fn price_response(price: &str) -> DebugBodyProjection {
        let body = price_wire(price);
        project_response_body(
            DebugBodySchema::OpenRtb,
            body.as_bytes(),
            &[("content-type".into(), "application/json".into())],
            DebugBodyEligibility::SingleTextValue,
            4096,
        )
    }

    fn captured(projection: DebugBodyProjection) -> Value {
        assert_eq!(projection.body_status, DebugBodyStatus::Captured);
        serde_json::from_str(&projection.body.unwrap()).unwrap()
    }

    fn omitted(projection: DebugBodyProjection, status: DebugBodyStatus) {
        let DebugBodyProjection { body_status, body } = projection;
        assert_eq!(body_status, status);
        assert_eq!(body, None);
    }

    fn at_path(path: &str, leaf: Value) -> Value {
        path.split('.').rev().fold(leaf, |value, part| {
            if part == "*" {
                json!([value])
            } else {
                json!({part: value})
            }
        })
    }

    #[test]
    fn repeated_wire_fields_are_each_validated_or_masked_before_projection() {
        use serde::ser::SerializeMap as _;

        let repeated = |key: &str, values: [Value; 2]| {
            let mut bytes = Vec::new();
            let mut serializer = serde_json::Serializer::new(&mut bytes);
            let mut object = serde::Serializer::serialize_map(&mut serializer, Some(2)).unwrap();
            for value in values {
                object.serialize_entry(key, &value).unwrap();
            }
            object.end().unwrap();
            bytes
        };

        let allowed = repeated("at", [json!(1_i32), json!(2_i32)]);
        let allowed_projection =
            project_request_body(DebugBodySchema::OpenRtb, &allowed, JSON, 4096);
        assert_eq!(allowed_projection.body_status, DebugBodyStatus::Captured);
        assert_eq!(allowed_projection.body.unwrap().as_bytes(), allowed);

        let secret = repeated("id", [json!("FIRST_SECRET"), json!("SECOND_SECRET")]);
        let masked_projection = project_request_body(DebugBodySchema::OpenRtb, &secret, JSON, 4096);
        assert_eq!(masked_projection.body_status, DebugBodyStatus::Captured);
        assert_eq!(
            masked_projection.body.unwrap().as_bytes(),
            repeated("id", [json!("[REDACTED]"), json!("[REDACTED]")])
        );

        for values in [
            [json!(1_i32), json!("SECRET")],
            [json!("SECRET"), json!(1_i32)],
        ] {
            omitted(
                project_request_body(
                    DebugBodySchema::OpenRtb,
                    &repeated("at", values),
                    JSON,
                    4096,
                ),
                DebugBodyStatus::OmittedType,
            );
        }
    }

    #[test]
    fn every_request_allowed_leaf_is_projected() {
        let source = json!({"at":2_i32,"test":1_i32,"tmax":100_i32,"allimps":0_i32,
            "imp":[{"banner":{"w":300_i32,"h":250_i32,"pos":1_i32,"format":[{"w":320_i32,"h":50_i32}]}}],
            "device":{"dnt":1_i32,"lmt":0_i32}, "regs":{"gdpr":1_i32,"coppa":0_i32,"ext":{"gdpr":0_i32}},
            "cur":["USD"]});
        assert_eq!(captured(request(&source)), source);
    }

    #[test]
    fn every_response_allowed_leaf_is_projected() {
        let source = json!({"cur":"USD","seatbid":[{"group":0_i32,"bid":[{"price":1.25_f64,"w":300_i32,"h":250_i32}]}]});
        assert_eq!(captured(response(&source)), source);
    }

    #[test]
    fn every_request_mask_replaces_the_whole_value_of_any_type() {
        for path in [
            "id",
            "site",
            "app",
            "dooh",
            "source",
            "user",
            "ext",
            "imp.*.id",
            "imp.*.tagid",
            "imp.*.ext",
            "device.ua",
            "device.sua",
            "device.ip",
            "device.ipv6",
            "device.ifa",
            "device.geo",
            "device.dpidsha1",
            "device.dpidmd5",
            "device.didsha1",
            "device.didmd5",
            "device.macsha1",
            "device.macmd5",
            "device.ext",
            "regs.gpp",
            "regs.us_privacy",
            "regs.ext.gpp",
            "regs.ext.us_privacy",
            "regs.ext.consent",
        ] {
            for secret in [
                json!(null),
                json!(true),
                json!(1_i32),
                json!("SECRET"),
                json!(["SECRET"]),
                json!({"SECRET_KEY":{"ext":{"consent":"SECRET"},"test":false}}),
            ] {
                assert_eq!(
                    captured(request(&at_path(path, secret))),
                    at_path(path, json!("[REDACTED]")),
                    "{path}"
                );
            }
        }
    }

    #[test]
    fn every_response_mask_replaces_the_whole_value_of_any_type() {
        for path in [
            "id",
            "ext",
            "seatbid.*.seat",
            "seatbid.*.bid.*.id",
            "seatbid.*.bid.*.impid",
            "seatbid.*.bid.*.crid",
            "seatbid.*.bid.*.adid",
            "seatbid.*.bid.*.dealid",
            "seatbid.*.bid.*.adm",
            "seatbid.*.bid.*.nurl",
            "seatbid.*.bid.*.burl",
            "seatbid.*.bid.*.lurl",
            "seatbid.*.bid.*.ext",
        ] {
            for secret in [
                json!(null),
                json!(false),
                json!(-1_i32),
                json!("SECRET"),
                json!(["SECRET"]),
                json!({"SECRET_KEY":1_i32}),
            ] {
                assert_eq!(
                    captured(response(&at_path(path, secret))),
                    at_path(path, json!("[REDACTED]")),
                    "{path}"
                );
            }
        }
    }

    #[test]
    fn unknown_keys_and_empty_ancestors_are_not_retained() {
        let source = json!({"KEY_CANARY":"VALUE_CANARY","at":1_i32,
            "imp":[{"banner":{"ext":{"KEY_CANARY":1_i32}}}],
            "regs":{"ext":{"gpc":"VALUE_CANARY","dsa":{"KEY_CANARY":1_i32}}}});
        assert_eq!(captured(request(&source)), json!({"at":1_i32}));
        omitted(
            request(&json!({"imp":[{"banner":{}}],"device":{},"cur":[]})),
            DebugBodyStatus::OmittedPolicy,
        );
        omitted(
            response(&json!({"seatbid":[{"bid":[{}]}]})),
            DebugBodyStatus::OmittedPolicy,
        );
    }

    #[test]
    fn currencies_are_filtered_without_manufacturing_a_response_default() {
        assert_eq!(
            captured(request(&json!({"cur":["EUR","USD","usd","USD",""]}))),
            json!({"cur":["USD","USD"]})
        );
        for cur in [None, Some(json!("")), Some(json!("EUR"))] {
            let source = cur.map_or_else(
                || json!({"id":1_i32}),
                |currency| json!({"id":1_i32,"cur":currency}),
            );
            assert_eq!(captured(response(&source)), json!({"id":"[REDACTED]"}));
        }
        omitted(response(&json!({"cur":""})), DebugBodyStatus::OmittedPolicy);
        omitted(response(&json!({})), DebugBodyStatus::OmittedPolicy);
    }

    #[test]
    fn original_header_evidence_counts_nontext_duplicates_without_mime_matching() {
        use DebugBodyEligibility::{Ineligible, SingleTextValue};
        for (headers, expected) in [
            (vec![], Ineligible),
            (vec![("CONTENT-TYPE", Some("text/html"))], SingleTextValue),
            (vec![("content-type", None)], Ineligible),
            (
                vec![("x", None), ("Content-Type", Some("application/json"))],
                SingleTextValue,
            ),
            (
                vec![
                    ("content-type", Some("application/json")),
                    ("Content-Type", None),
                ],
                Ineligible,
            ),
            (
                vec![
                    ("content-type", Some("application/json")),
                    ("content-type", Some("application/json")),
                ],
                Ineligible,
            ),
        ] {
            assert_eq!(
                DebugBodyEligibility::from_complete_headers(headers),
                expected
            );
        }
    }

    #[test]
    fn mime_uses_exact_json_type_and_valid_parameters_under_the_byte_bound() {
        for value in [
            "application/json",
            "Application/JSON",
            "application/json; charset=utf-8",
            "application/json; p=\"quoted value\"",
            "application/json; p=\"json+secret\"",
        ] {
            assert!(is_debug_json_content_type(value), "{value}");
        }
        for value in [
            "",
            "text/html",
            "text/json",
            "application/problem+json",
            "application/json+secret",
            "application/json+json",
            "Application/JSON+XML; charset=utf-8",
            "application/json,application/json",
            "application/json;broken",
            "application/json\r\n",
            "application/json; p=\"unterminated",
        ] {
            assert!(!is_debug_json_content_type(value), "{value}");
        }
        let long = format!("application/json;x={}", "a".repeat(0x0001_0000 - 19));
        assert_eq!(long.len(), 0x0001_0000);
        assert!(is_debug_json_content_type(&long));
        let mut over_cap = long;
        over_cap.push('a');
        assert!(!is_debug_json_content_type(&over_cap));
        let dense = format!("application/json{}", ";a=b".repeat(16380));
        assert!(dense.len() <= 0x0001_0000);
        assert!(is_debug_json_content_type(&dense));
    }

    #[test]
    fn request_mime_suffixes_do_not_authorize_body_projection() {
        let body = serde_json::to_vec(&json!({"id":"SECRET","at":1_i32})).unwrap();
        for content_type in ["application/json", "Application/JSON; p=\"json+secret\""] {
            assert_eq!(
                captured(project_request_body(
                    DebugBodySchema::OpenRtb,
                    &body,
                    [("content-type", content_type)],
                    4096,
                )),
                json!({"id":"[REDACTED]","at":1_i32})
            );
        }
        for content_type in [
            "application/json+secret",
            "application/json+json",
            "Application/JSON+XML; charset=utf-8",
            "application/problem+json",
        ] {
            omitted(
                project_request_body(
                    DebugBodySchema::OpenRtb,
                    &body,
                    [("content-type", content_type)],
                    4096,
                ),
                DebugBodyStatus::OmittedType,
            );
            omitted(
                project_request_body(
                    DebugBodySchema::OpenRtb,
                    &[],
                    [("content-type", content_type)],
                    4096,
                ),
                DebugBodyStatus::Empty,
            );
        }
    }

    #[test]
    fn response_mime_suffixes_do_not_authorize_body_projection() {
        let body = serde_json::to_vec(&json!({"id":"SECRET","cur":"USD"})).unwrap();
        let project = |content_type: &str, bytes: Vec<u8>| {
            crate::BidderHttpResponse::from_transport(
                200,
                vec![("content-type".into(), content_type.into())],
                bytes,
                DebugBodyEligibility::SingleTextValue,
            )
            .within_node_budget(4096)
            .project_debug_body(DebugBodySchema::OpenRtb)
        };
        for content_type in ["application/json", "Application/JSON; p=\"json+secret\""] {
            assert_eq!(
                captured(project(content_type, body.clone())),
                json!({"id":"[REDACTED]","cur":"USD"})
            );
        }
        for content_type in [
            "application/json+secret",
            "application/json+json",
            "Application/JSON+XML; charset=utf-8",
            "application/problem+json",
        ] {
            omitted(
                project(content_type, body.clone()),
                DebugBodyStatus::OmittedType,
            );
            omitted(project(content_type, Vec::new()), DebugBodyStatus::Empty);
        }
    }

    #[test]
    fn media_evidence_and_status_precedence_are_fixed() {
        let body = serde_json::to_vec(&json!({"id":"SECRET"})).unwrap();
        for headers in [
            vec![],
            vec![("content-type", "text/html")],
            vec![
                ("content-type", "application/json"),
                ("CONTENT-TYPE", "application/json"),
            ],
            vec![
                ("content-type", "application/json"),
                ("content-type", "text/html"),
            ],
            vec![("content-type", "application/json, application/json")],
        ] {
            omitted(
                project_request_body(DebugBodySchema::OpenRtb, &body, headers.clone(), 4096),
                DebugBodyStatus::OmittedType,
            );
            omitted(
                project_request_body(DebugBodySchema::Unsupported, &[], headers, 0),
                DebugBodyStatus::Empty,
            );
        }
        let too_large = vec![b' '; 0x0001_0001];
        omitted(
            project_request_body(
                DebugBodySchema::OpenRtb,
                &too_large,
                [("content-type", "text/html")],
                4096,
            ),
            DebugBodyStatus::OmittedSize,
        );
        omitted(
            project_request_body(DebugBodySchema::Unsupported, &too_large, JSON, 4096),
            DebugBodyStatus::OmittedType,
        );
        let long = "x".repeat(0x0001_0001);
        omitted(
            project_request_body(
                DebugBodySchema::OpenRtb,
                &body,
                [("content-type", long.as_str())],
                4096,
            ),
            DebugBodyStatus::OmittedSize,
        );
        omitted(
            project_request_body(DebugBodySchema::OpenRtb, b"\xff", JSON, 4096),
            DebugBodyStatus::OmittedType,
        );
        for eligibility in [
            DebugBodyEligibility::Ineligible,
            DebugBodyEligibility::SingleTextValue,
        ] {
            let expected = if eligibility == DebugBodyEligibility::Ineligible {
                DebugBodyStatus::OmittedType
            } else {
                DebugBodyStatus::Captured
            };
            let result = project_response_body(
                DebugBodySchema::OpenRtb,
                &body,
                &[("content-type".into(), "application/json".into())],
                eligibility,
                4096,
            );
            assert_eq!(result.body_status, expected);
        }
    }

    #[test]
    fn request_numeric_domains_reject_negative_overflow_wrong_types_and_null() {
        let domains = [
            ("test", 0_u64, 1_u64),
            ("allimps", 0, 1),
            ("tmax", 0, u64::try_from(i64::MAX).unwrap()),
            ("imp.*.banner.w", 0, u64::try_from(i32::MAX).unwrap()),
            ("imp.*.banner.h", 0, u64::try_from(i32::MAX).unwrap()),
            ("imp.*.banner.pos", 0, 7),
            (
                "imp.*.banner.format.*.w",
                0,
                u64::try_from(i32::MAX).unwrap(),
            ),
            (
                "imp.*.banner.format.*.h",
                0,
                u64::try_from(i32::MAX).unwrap(),
            ),
            ("device.dnt", 0, 1),
            ("device.lmt", 0, 1),
            ("regs.gdpr", 0, 1),
            ("regs.coppa", 0, 1),
            ("regs.ext.gdpr", 0, 1),
        ];
        for (path, min, max) in domains {
            for valid in [min, max] {
                assert_eq!(
                    captured(request(&at_path(path, json!(valid)))),
                    at_path(path, json!(valid))
                );
            }
            for invalid in [
                json!(-1_i32),
                json!(max + 1),
                json!(0.5_f64),
                json!(true),
                json!("SECRET"),
                json!(null),
                json!([]),
                json!({}),
            ] {
                omitted(
                    request(&at_path(path, invalid)),
                    DebugBodyStatus::OmittedType,
                );
            }
        }
    }

    #[test]
    fn auction_type_includes_exchange_specific_values() {
        for valid in [1_i32, 2_i32, 500_i32, i32::MAX] {
            assert_eq!(captured(request(&json!({"at":valid}))), json!({"at":valid}));
        }
        for invalid in [
            json!(0_i32),
            json!(3_i32),
            json!(499_i32),
            json!(-1_i32),
            json!(0x8000_0000_u64),
            json!(null),
            json!(false),
            json!("SECRET"),
            json!(1.5_f64),
            json!([]),
            json!({}),
        ] {
            omitted(
                request(&json!({"at":invalid})),
                DebugBodyStatus::OmittedType,
            );
        }
    }

    #[test]
    fn response_numeric_domains_reject_negative_overflow_wrong_types_and_null() {
        for (path, max) in [
            ("seatbid.*.group", 1_u64),
            ("seatbid.*.bid.*.w", u64::try_from(i32::MAX).unwrap()),
            ("seatbid.*.bid.*.h", u64::try_from(i32::MAX).unwrap()),
        ] {
            for valid in [0, max] {
                assert_eq!(
                    captured(response(&at_path(path, json!(valid)))),
                    at_path(path, json!(valid))
                );
            }
            for invalid in [
                json!(-1_i32),
                json!(max + 1),
                json!(1.5_f64),
                json!(null),
                json!("SECRET"),
                json!(true),
                json!([]),
                json!({}),
            ] {
                omitted(
                    response(&at_path(path, invalid)),
                    DebugBodyStatus::OmittedType,
                );
            }
        }
        for valid in [json!(0_i32), json!(1.25_f64), json!(f64::MAX)] {
            assert_eq!(
                captured(response(&at_path("seatbid.*.bid.*.price", valid.clone()))),
                at_path("seatbid.*.bid.*.price", valid)
            );
        }
        for invalid in [
            json!(-0.1_f64),
            json!(null),
            json!(true),
            json!("SECRET"),
            json!([]),
            json!({}),
        ] {
            omitted(
                response(&at_path("seatbid.*.bid.*.price", invalid)),
                DebugBodyStatus::OmittedType,
            );
        }
    }

    #[test]
    fn price_tokens_reject_negative_nonzero_significands_that_underflow() {
        for price in ["-1e-400", "-1E-400", "-0.1e-400", "-9.99E-999"] {
            omitted(price_response(price), DebugBodyStatus::OmittedType);
        }
    }

    #[test]
    fn price_tokens_preserve_negative_zero() {
        for price in ["-0", "-0.0", "-0.000e-400", "-0E+12", "-0.0e0"] {
            assert_eq!(
                price_response(price),
                DebugBodyProjection {
                    body_status: DebugBodyStatus::Captured,
                    body: Some(price_wire(price)),
                },
                "{price}"
            );
        }
    }

    #[test]
    fn price_tokens_preserve_positive_underflow() {
        for price in ["1e-400", "1E-400", "0.1e-400", "9.99E-999"] {
            assert_eq!(
                price_response(price),
                DebugBodyProjection {
                    body_status: DebugBodyStatus::Captured,
                    body: Some(price_wire(price)),
                },
                "{price}"
            );
        }
    }

    #[test]
    fn price_tokens_preserve_zero_and_ordinary_positive_values() {
        for price in ["0", "0.0", "1", "1.25", "1e2", "1E+2", "0.25e-1"] {
            assert_eq!(
                price_response(price),
                DebugBodyProjection {
                    body_status: DebugBodyStatus::Captured,
                    body: Some(price_wire(price)),
                },
                "{price}"
            );
        }
    }

    #[test]
    fn traversed_containers_require_their_declared_shape() {
        for path in [
            "imp",
            "cur",
            "imp.*.banner.format",
            "device",
            "regs",
            "regs.ext",
            "imp.*.banner",
            "imp.*",
        ] {
            omitted(
                request(&at_path(path, json!(null))),
                DebugBodyStatus::OmittedType,
            );
        }
        for path in ["seatbid", "seatbid.*", "seatbid.*.bid", "seatbid.*.bid.*"] {
            omitted(
                response(&at_path(path, json!(null))),
                DebugBodyStatus::OmittedType,
            );
        }
        for root in [json!([]), json!(null), json!(1_i32), json!("SECRET")] {
            omitted(request(&root), DebugBodyStatus::OmittedType);
        }
    }

    #[test]
    fn syntax_and_depth_take_precedence_over_invalid_allowed_leaves() {
        // Malformed/trailing wire syntax cannot be represented by json!.
        for body in [
            br#"{"test":"SECRET", "x": }"#.as_slice(),
            br#"{"test":false} {}"#,
            br#"{"id":null} trailing"#,
        ] {
            omitted(
                project_request_body(DebugBodySchema::OpenRtb, body, JSON, 4096),
                DebugBodyStatus::OmittedStructure,
            );
        }
        for key in ["UNKNOWN_SECRET_KEY", "user"] {
            for depth in [63_i32, 64_i32, 65_i32] {
                let nested = (1_i32..depth).fold(json!(null), |v, _| json!([v]));
                let source = json!({key:nested,"test":false});
                let expected = if depth > 64_i32 {
                    DebugBodyStatus::OmittedStructure
                } else {
                    DebugBodyStatus::OmittedType
                };
                omitted(request(&source), expected);
                let valid = json!({key:nested,"test":1_i32});
                if depth <= 64_i32 {
                    assert_eq!(
                        captured(request(&valid)),
                        if key == "user" {
                            json!({"user":"[REDACTED]","test":1_i32})
                        } else {
                            json!({"test":1_i32})
                        }
                    );
                } else {
                    omitted(request(&valid), DebugBodyStatus::OmittedStructure);
                }
            }
        }
    }

    #[test]
    fn huge_valid_numbers_only_fail_on_allowed_numeric_paths() {
        // Preserve out-of-range numeric tokens that Value would reject or round.
        for body in [
            br#"{"unknown":1e400,"id":1e400}"#.as_slice(),
            br#"{"unknown":999999999999999999999999999999999999999,"id":null}"#,
        ] {
            assert_eq!(
                captured(project_request_body(
                    DebugBodySchema::OpenRtb,
                    body,
                    JSON,
                    4096
                )),
                json!({"id":"[REDACTED]"})
            );
        }
        // json! cannot construct the non-finite numeric token on an allowed path.
        omitted(
            project_request_body(DebugBodySchema::OpenRtb, br#"{"tmax":1e400}"#, JSON, 4096),
            DebugBodyStatus::OmittedType,
        );
        // The response path must reject the same original out-of-range token.
        omitted(
            project_response_body(
                DebugBodySchema::OpenRtb,
                br#"{"seatbid":[{"bid":[{"price":1e400}]}]}"#,
                &[("content-type".into(), "application/json".into())],
                DebugBodyEligibility::SingleTextValue,
                4096,
            ),
            DebugBodyStatus::OmittedType,
        );
    }

    #[test]
    fn input_and_node_limits_include_unknown_and_masked_subtrees() {
        for nodes in [4095_usize, 4096, 4097] {
            for key in ["unknown", "user"] {
                let source = json!({"test":1_i32,key:vec![json!(null);nodes-5]});
                let bytes = serde_json::to_vec(&source).unwrap();
                for budget in [4095, 4096, 4097, u32::MAX] {
                    let result =
                        project_request_body(DebugBodySchema::OpenRtb, &bytes, JSON, budget);
                    assert_eq!(
                        result.body_status,
                        if u64::try_from(nodes).unwrap() <= u64::from(budget.min(4096)) {
                            DebugBodyStatus::Captured
                        } else {
                            DebugBodyStatus::OmittedStructure
                        }
                    );
                }
            }
        }
        let body = serde_json::to_vec(&json!({"test":1_i32})).unwrap();
        for budget in [0, 1, 2, 3] {
            assert_eq!(
                project_request_body(DebugBodySchema::OpenRtb, &body, JSON, budget).body_status,
                if budget == 3 {
                    DebugBodyStatus::Captured
                } else {
                    DebugBodyStatus::OmittedStructure
                }
            );
        }
        for size in [0xFFFF, 0x0001_0000, 0x0001_0001] {
            let mut bytes = serde_json::to_vec(&json!({"id":"SECRET"})).unwrap();
            bytes.resize(size, b' ');
            assert_eq!(
                project_request_body(DebugBodySchema::OpenRtb, &bytes, JSON, 4096).body_status,
                if size <= 0x0001_0000 {
                    DebugBodyStatus::Captured
                } else {
                    DebugBodyStatus::OmittedSize
                }
            );
        }
    }

    #[test]
    fn sanitized_output_boundary_is_exact_and_source_is_unchanged() {
        // Each masked impression adds 20 bytes; tmax tunes the final byte.
        for (tail, expected) in [
            (100_000_u64, 16383_usize),
            (1_000_000, 0x4000),
            (10_000_000, 0x4001),
        ] {
            let source = json!({"imp":vec![json!({"id":"SECRET"});818],"tmax":tail});
            let bytes = serde_json::to_vec(&source).unwrap();
            let original = bytes.clone();
            let projected = json!({"imp":vec![json!({"id":"[REDACTED]"});818],"tmax":tail});
            assert_eq!(serde_json::to_vec(&projected).unwrap().len(), expected);
            let result = project_request_body(DebugBodySchema::OpenRtb, &bytes, JSON, 4096);
            if expected <= 0x4000 {
                assert_eq!(captured(result), projected);
            } else {
                omitted(result, DebugBodyStatus::OmittedSize);
            }
            assert_eq!(bytes, original);
        }
    }

    #[test]
    fn statuses_serialize_as_the_fixed_snake_case_vocabulary() {
        for (status, name) in [
            (DebugBodyStatus::Empty, "empty"),
            (DebugBodyStatus::Unavailable, "unavailable"),
            (DebugBodyStatus::OmittedType, "omitted_type"),
            (DebugBodyStatus::OmittedSize, "omitted_size"),
            (DebugBodyStatus::OmittedStructure, "omitted_structure"),
            (DebugBodyStatus::OmittedPolicy, "omitted_policy"),
            (DebugBodyStatus::Captured, "captured"),
        ] {
            assert_eq!(serde_json::to_value(status).unwrap(), json!(name));
        }
    }

    #[test]
    fn allocation_refusal_precedes_schema_size_media_and_structure() {
        let body = serde_json::to_vec(&json!({"test":false})).unwrap();
        for schema in [DebugBodySchema::Unsupported, DebugBodySchema::OpenRtb] {
            omitted(
                project_with_allocator(
                    schema,
                    &body,
                    MimeInput::default(),
                    DebugBodyEligibility::Ineligible,
                    0,
                    Context::Request,
                    || Err(()),
                ),
                DebugBodyStatus::Unavailable,
            );
        }
        omitted(
            project_with_allocator(
                DebugBodySchema::Unsupported,
                &[],
                MimeInput::default(),
                DebugBodyEligibility::Ineligible,
                0,
                Context::Request,
                || panic!("known empty bodies must not allocate"),
            ),
            DebugBodyStatus::Empty,
        );
    }

    #[test]
    fn output_overflow_never_hides_a_later_type_or_depth_failure() {
        let mut source = json!({"imp":vec![json!({"id":"SECRET"});900],"test":false});
        omitted(request(&source), DebugBodyStatus::OmittedType);
        source["test"] = json!(1_i32);
        source["user"] = (1..65_u32).fold(json!(null), |nested, _| json!([nested]));
        omitted(request(&source), DebugBodyStatus::OmittedStructure);
    }

    #[test]
    fn speculative_empty_branches_do_not_cause_false_output_overflow() {
        let mut imps = vec![json!({"id":"SECRET"}); 818];
        imps.push(json!({"banner":{"format":[{},{}]},"KEY_CANARY":{}}));
        let source = json!({"imp":imps,"tmax":1_000_000_u64});
        let result = request(&source);
        assert_eq!(result.body_status, DebugBodyStatus::Captured);
        assert_eq!(result.body.unwrap().len(), 0x4000);
    }

    #[test]
    fn escaped_fixed_keys_and_currency_decode_without_retaining_unknown_keys() {
        // Exact escaped key/value spellings are the subject; json! normalizes them.
        let body =
            br#"{"\u0069d":{"SECRET":1},"\u0074est":1,"cur":["\u0055SD"],"SECRET\u005fKEY":null}"#;
        assert_eq!(
            captured(project_request_body(
                DebugBodySchema::OpenRtb,
                body,
                JSON,
                4096
            )),
            json!({"id":"[REDACTED]","test":1_i32,"cur":["USD"]})
        );
    }

    #[test]
    fn known_empty_request_does_not_even_iterate_headers() {
        let headers = iter::from_fn(|| -> Option<(&str, &str)> {
            panic!("known empty bodies must not inspect MIME evidence")
        });
        omitted(
            project_request_body(DebugBodySchema::Unsupported, &[], headers, 0),
            DebugBodyStatus::Empty,
        );
    }

    #[test]
    fn long_masks_and_unknown_response_fields_never_leak_or_mutate_source_bytes() {
        let secret = "SECRET\u{1f600}\u{0000}".repeat(1000);
        let fixtures = [
            json!({"user":{"ext":{"consent":secret}},"test":1_i32}),
            json!({"seatbid":[{"bid":[{"adm":secret,"price":1_i32,"KEY_CANARY":secret}],"KEY_CANARY":secret}],"KEY_CANARY":secret}),
        ];
        for (index, source) in fixtures.iter().enumerate() {
            let bytes = serde_json::to_vec(source).unwrap();
            assert!(bytes.len() <= 0x0001_0000);
            let before = bytes.clone();
            let projection = if index == 0 {
                project_request_body(DebugBodySchema::OpenRtb, &bytes, JSON, 4096)
            } else {
                project_response_body(
                    DebugBodySchema::OpenRtb,
                    &bytes,
                    &[("content-type".into(), "application/json".into())],
                    DebugBodyEligibility::SingleTextValue,
                    4096,
                )
            };
            assert_eq!(
                captured(projection),
                if index == 0 {
                    json!({"user":"[REDACTED]","test":1_i32})
                } else {
                    json!({"seatbid":[{"bid":[{"adm":"[REDACTED]","price":1_i32}]}]})
                }
            );
            assert_eq!(bytes, before);
        }
    }
}
