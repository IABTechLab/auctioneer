use super::projection::{preserve, put, ser_error};
use auctioneer_openrtb::types::bid_request::Native;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{DeserializeSeed, Error as DeError, MapAccess, SeqAccess, Visitor},
    ser::SerializeMap as _,
};
use serde_json::value::RawValue;
use std::fmt;

fn json_space(input: &str) -> &str {
    input.trim_start_matches([' ', '\t', '\r', '\n'])
}

fn next_native_input(input: &str) -> &str {
    let trimmed = json_space(input);
    json_space(trimmed.strip_prefix(',').unwrap_or(trimmed))
}

fn native_hex_escape(bytes: &mut impl Iterator<Item = u8>) -> Option<u16> {
    let mut code = 0_u16;
    for _ in 0_u8..4 {
        let digit = u16::try_from(char::from(bytes.next()?).to_digit(16)?).ok()?;
        code = code.checked_mul(16)?.checked_add(digit)?;
    }
    Some(code)
}

fn native_string_unicode(raw: &RawValue) -> bool {
    let mut bytes = raw.get().bytes();
    while let Some(byte) = bytes.next() {
        if byte != b'\\' {
            continue;
        }
        if bytes.next() != Some(b'u') {
            continue;
        }
        let Some(code) = native_hex_escape(&mut bytes) else {
            return false;
        };
        match code {
            0xD800_u16..=0xDBFF_u16 => {
                if bytes.next() != Some(b'\\') || bytes.next() != Some(b'u') {
                    return false;
                }
                if !native_hex_escape(&mut bytes)
                    .is_some_and(|low| matches!(low, 0xDC00_u16..=0xDFFF_u16))
                {
                    return false;
                }
            }
            0xDC00_u16..=0xDFFF_u16 => return false,
            _ => {}
        }
    }
    true
}

// Serde validates the key's JSON syntax. Compare its encoded ASCII name without
// asking Serde to allocate decoded scratch for escaped or oversized keys.
fn raw_key_eq(raw: &RawValue, expected: &str, fold: bool) -> bool {
    let Some(encoded) = raw
        .get()
        .strip_prefix('"')
        .and_then(|key| key.strip_suffix('"'))
    else {
        return false;
    };
    let mut bytes = encoded.bytes();
    for expected_byte in expected.bytes() {
        let Some(next) = bytes.next() else {
            return false;
        };
        let actual_byte = if next == b'\\' {
            if bytes.next() != Some(b'u') {
                return false;
            }
            let Some(unicode) = native_hex_escape(&mut bytes) else {
                return false;
            };
            let Ok(ascii) = u8::try_from(unicode) else {
                return false;
            };
            ascii
        } else {
            next
        };
        if !(actual_byte == expected_byte
            || fold && actual_byte.eq_ignore_ascii_case(&expected_byte))
        {
            return false;
        }
    }
    bytes.next().is_none()
}

struct NativeChecked<'a> {
    remaining: &'a str,
    valid: bool,
}

struct NativeValue<'a> {
    input: &'a str,
    root: bool,
}

fn native_suffix<'a, E: DeError>(input: &'a str, raw: &RawValue) -> Result<&'a str, E> {
    if input.as_ptr() != raw.get().as_ptr() {
        return Err(E::custom("invalid native projection position"));
    }
    input
        .get(raw.get().len()..)
        .ok_or_else(|| E::custom("invalid native projection span"))
}

impl<'de> DeserializeSeed<'de> for NativeValue<'de> {
    type Value = NativeChecked<'de>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        let input = json_space(self.input);
        match input.as_bytes().first() {
            Some(b'{') => deserializer.deserialize_map(NativeObject {
                input: input
                    .strip_prefix('{')
                    .ok_or_else(|| D::Error::custom("invalid native object"))?,
                root: self.root,
            }),
            Some(b'[') => deserializer.deserialize_seq(NativeArray {
                input: input
                    .strip_prefix('[')
                    .ok_or_else(|| D::Error::custom("invalid native array"))?,
            }),
            Some(_) => {
                // Capture only scalar spans. Containers use Serde's recursive
                // accessors, so neither its RawValue nesting Vec nor decoded
                // string scratch is needed on this preflight path.
                let raw = <&RawValue as Deserialize>::deserialize(deserializer)?;
                let numeric = matches!(input.as_bytes().first(), Some(b'-' | b'0'..=b'9'));
                let valid = if numeric {
                    raw.get().parse::<f64>().is_ok_and(f64::is_finite)
                } else {
                    native_string_unicode(raw)
                };
                Ok(NativeChecked {
                    remaining: native_suffix(input, raw)?,
                    valid,
                })
            }
            None => Err(D::Error::custom("missing native value")),
        }
    }
}

struct NativeObject<'a> {
    input: &'a str,
    root: bool,
}

impl<'de> Visitor<'de> for NativeObject<'de> {
    type Value = NativeChecked<'de>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("native object")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
        let mut input = json_space(self.input);
        let mut seen = 0_u8;
        let mut valid = true;
        while let Some(key) = map.next_key::<&'de RawValue>()? {
            valid &= native_string_unicode(key);
            let after_key = json_space(native_suffix::<M::Error>(input, key)?);
            let value_input = json_space(
                after_key
                    .strip_prefix(':')
                    .ok_or_else(|| M::Error::custom("missing native field separator"))?,
            );
            if self.root {
                let known_field = [
                    ("eventtrackers", 1_u8),
                    ("plcmttype", 2_u8),
                    ("context", 4_u8),
                ]
                .into_iter()
                .find(|(name, _)| raw_key_eq(key, name, false));
                if let Some((_, bit)) = known_field {
                    valid &= seen & bit == 0;
                    seen |= bit;
                    valid &= match bit {
                        1 => value_input.starts_with('['),
                        2 => matches!(value_input.as_bytes().first(), Some(b'-' | b'0'..=b'9')),
                        4 => {
                            value_input.starts_with("null")
                                || matches!(
                                    value_input.as_bytes().first(),
                                    Some(b'-' | b'0'..=b'9')
                                )
                        }
                        _ => false,
                    };
                }
            }
            let value = map.next_value_seed(NativeValue {
                input: value_input,
                root: false,
            })?;
            valid &= value.valid;
            input = next_native_input(value.remaining);
        }
        if self.root {
            valid &= seen & 3 == 3;
        }
        let remaining = input
            .strip_prefix('}')
            .ok_or_else(|| M::Error::custom("missing native object end"))?;
        Ok(NativeChecked { remaining, valid })
    }
}

struct NativeArray<'a> {
    input: &'a str,
}

impl<'de> Visitor<'de> for NativeArray<'de> {
    type Value = NativeChecked<'de>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("native array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut input = json_space(self.input);
        let mut valid = true;
        while let Some(value) = seq.next_element_seed(NativeValue { input, root: false })? {
            valid &= value.valid;
            input = next_native_input(value.remaining);
        }
        let remaining = input
            .strip_prefix(']')
            .ok_or_else(|| A::Error::custom("missing native array end"))?;
        Ok(NativeChecked { remaining, valid })
    }
}

pub(super) fn native_eligible(native: &Native) -> bool {
    if matches!(native.ver.as_deref(), Some("1.0" | "1.1")) {
        return true;
    }
    let Some(source) = native.request.as_deref() else {
        return false;
    };
    let input = json_space(source);
    if !input.starts_with('{') {
        return false;
    }
    let mut deserializer = serde_json::Deserializer::from_str(source);
    let Ok(checked) = (NativeValue { input, root: true }).deserialize(&mut deserializer) else {
        return false;
    };
    checked.valid && json_space(checked.remaining).is_empty() && deserializer.end().is_ok()
}

pub(super) fn validate_app_fields(raw: &RawValue) -> Result<(), ()> {
    struct Fields<'a> {
        input: &'a str,
    }
    impl<'de> Visitor<'de> for Fields<'de> {
        type Value = bool;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("app extension object")
        }

        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<bool, M::Error> {
            let mut input = json_space(self.input);
            let mut seen = false;
            let mut valid = true;
            while let Some(key) = map.next_key::<&'de RawValue>()? {
                let after_key = json_space(native_suffix::<M::Error>(input, key)?);
                let value_input = json_space(
                    after_key
                        .strip_prefix(':')
                        .ok_or_else(|| M::Error::custom("missing app field separator"))?,
                );
                if raw_key_eq(key, "data", true) {
                    valid &=
                        !seen && (value_input.starts_with('{') || value_input.starts_with("null"));
                    seen = true;
                }
                // Traverse unknown containers without RawValue's nesting allocation.
                let value = map.next_value_seed(NativeValue {
                    input: value_input,
                    root: false,
                })?;
                input = next_native_input(value.remaining);
            }
            Ok(valid)
        }
    }
    let input = json_space(raw.get());
    if input == "null" {
        return Ok(());
    }
    let fields = Fields {
        input: input.strip_prefix('{').ok_or(())?,
    };
    let valid = serde_json::Deserializer::from_str(raw.get())
        .deserialize_map(fields)
        .map_err(|_error| ())?;
    valid.then_some(()).ok_or(())
}

pub(super) struct NativeProjection<'a>(pub &'a Native);
impl Serialize for NativeProjection<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Native {
            request,
            ver,
            ext,
            other,
        } = self.0;
        let mut map = serializer.serialize_map(None)?;
        // Emit the retained source string before allocating nested parse scratch.
        put(&mut map, "request", request.as_ref())?;
        put(&mut map, "ver", ver.as_ref())?;
        put(&mut map, "ext", ext.as_ref())?;
        preserve(&mut map, other, &["request_native"])?;
        if !matches!(ver.as_deref(), Some("1.0" | "1.1")) {
            if !native_eligible(self.0) {
                return Err(ser_error());
            }
            let source = request.as_deref().ok_or_else(ser_error::<S::Error>)?;
            let raw = serde_json::from_str::<&RawValue>(source)
                .map_err(|_error| ser_error::<S::Error>())?;
            map.serialize_entry("request_native", raw)?;
        }
        map.end()
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use crate::FloorConversion;
    use crate::bidders::rubicon::test_support::{
        assert_credential_eligibility, banner, body, native_impression, outbound, parameters,
        request,
    };
    use serde_json::json;

    #[test]
    fn native_preflight_requires_the_exact_lowercase_eventtrackers_map_key() {
        let exact = native_impression("1.2", &json!({"eventtrackers":[],"plcmttype":2_i32}));
        assert_credential_eligibility(&request(&json!([exact])), FloorConversion::default(), true);
        let wrong_case = native_impression("1.2", &json!({"EventTrackers":[],"plcmttype":2_i32}));
        assert_credential_eligibility(
            &request(&json!([wrong_case])),
            FloorConversion::default(),
            false,
        );
    }

    #[test]
    fn credential_eligibility_native_missing_eventtrackers_is_rejected() {
        let imp = native_impression("1.2", &json!({"plcmttype":2_i32}));
        assert_credential_eligibility(&request(&json!([imp])), FloorConversion::default(), false);
    }

    #[test]
    fn credential_eligibility_native_nonarray_eventtrackers_are_rejected() {
        let imp = native_impression("1.2", &json!({"eventtrackers":false,"plcmttype":2_i32}));
        assert_credential_eligibility(&request(&json!([imp])), FloorConversion::default(), false);
    }

    #[test]
    fn credential_eligibility_native_missing_placement_is_rejected() {
        let imp = native_impression("1.2", &json!({"eventtrackers":[]}));
        assert_credential_eligibility(&request(&json!([imp])), FloorConversion::default(), false);
    }

    #[test]
    fn credential_eligibility_native_nonnumeric_placement_is_rejected() {
        let imp = native_impression("1.2", &json!({"eventtrackers":[],"plcmttype":"2"}));
        assert_credential_eligibility(&request(&json!([imp])), FloorConversion::default(), false);
    }

    #[test]
    fn credential_eligibility_native_nonnumeric_context_is_rejected() {
        let imp = native_impression(
            "1.2",
            &json!({"eventtrackers":[],"plcmttype":2_i32,"context":"1"}),
        );
        assert_credential_eligibility(&request(&json!([imp])), FloorConversion::default(), false);
    }

    #[test]
    fn credential_eligibility_native_malformed_embedded_json_is_rejected() {
        let mut imp = native_impression("1.2", &json!({}));
        imp["native"]["request"] = json!("not-json");
        assert_credential_eligibility(&request(&json!([imp])), FloorConversion::default(), false);
    }

    #[test]
    fn credential_eligibility_valid_sibling_rescues_each_invalid_new_native_request() {
        for payload in [
            json!({"plcmttype":2_i32}),
            json!({"eventtrackers":false,"plcmttype":2_i32}),
            json!({"eventtrackers":[]}),
            json!({"eventtrackers":[],"plcmttype":"2"}),
            json!({"eventtrackers":[],"plcmttype":2_i32,"context":"1"}),
        ] {
            for invalid_first in [false, true] {
                let invalid = native_impression("1.2", &payload);
                let valid = banner("valid");
                let imps = if invalid_first {
                    json!([invalid, valid])
                } else {
                    json!([valid, invalid])
                };
                assert_credential_eligibility(&request(&imps), FloorConversion::default(), true);
            }
        }
    }

    #[test]
    fn credential_eligibility_native_version_bypass_and_new_native_numeric_controls_are_preserved()
    {
        for version in ["1.0", "1.1"] {
            let mut imp = native_impression(version, &json!({}));
            imp["native"]["request"] = json!("not-json");
            assert_credential_eligibility(
                &request(&json!([imp])),
                FloorConversion::default(),
                true,
            );
        }
        for payload in [
            json!({"eventtrackers":[],"plcmttype":2_i32}),
            json!({"eventtrackers":[],"plcmttype":2_i32,"context":null}),
            json!({"eventtrackers":[],"plcmttype":2.5_f64,"context":1.5_f64}),
        ] {
            let imp = native_impression("1.2", &payload);
            assert_credential_eligibility(
                &request(&json!([imp])),
                FloorConversion::default(),
                true,
            );
        }
    }

    #[test]
    fn native_legacy_requests_stay_strings_and_new_requests_gain_request_native() {
        for version in ["1.0", "1.1", "1.2"] {
            let native = json!({"eventtrackers":[],"plcmttype":2_i32,"context":1_i32,"assets":[]});
            let mut imp = banner("i");
            imp.as_object_mut().unwrap().remove("banner");
            imp["native"] = json!({"ver":version,"request":native.to_string(),"keep":true});
            let calls = outbound(&request(&json!([imp])));
            assert_eq!(calls.len(), 1);
            let body = body(&calls[0]);
            assert_eq!(body["imp"][0]["native"]["request"], native.to_string());
            assert_eq!(body["imp"][0]["native"]["keep"], true);
            if version == "1.2" {
                assert_eq!(body["imp"][0]["native"]["request_native"], native);
            } else {
                assert!(body["imp"][0]["native"].get("request_native").is_none());
            }
        }
    }

    #[test]
    fn invalid_native_controls_do_not_remove_valid_impression_siblings() {
        for native in [
            json!({"plcmttype":1_i32}),
            json!({"eventtrackers":false,"plcmttype":1_i32}),
            json!({"eventtrackers":[]}),
            json!({"eventtrackers":[],"plcmttype":"1"}),
            json!({"eventtrackers":[],"plcmttype":1_i32,"context":"1"}),
        ] {
            let invalid = json!({"id":"bad","native":{"ver":"1.2","request":native.to_string()},
                "ext":{"bidder":parameters()}});
            let request = request(&json!([invalid, banner("i")]));
            let calls = outbound(&request);
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].covered_imp_ids().collect::<Vec<_>>(), ["i"]);
        }
    }
}
