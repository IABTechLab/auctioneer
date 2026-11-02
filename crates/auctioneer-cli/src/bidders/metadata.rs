//! Source declarations only: no alias inheritance, activation or routing policy.

use super::Error;
use serde::Serialize;
use serde_json::Value as JsonValue;
use serde_yaml_ng::{Number, Value};

#[derive(Serialize)]
pub(super) struct Converted {
    pub document: JsonValue,
    pub ignored_fields: Vec<String>,
}

#[derive(Clone, Copy)]
enum Shape {
    Text,
    Boolean,
    VendorId,
    TextList,
    Version,
    Object(&'static [(&'static str, Self)]),
}

// YAML contracts from config/bidderinfo.go at
// d7f8515b86258688304b0d9b6668c6a0e258bc9e, not runtime mapstructure spellings.
const BIDDER: &[(&str, Shape)] = &[
    ("aliasOf", Shape::Text),
    ("whiteLabelOnly", Shape::Boolean),
    ("disabled", Shape::Boolean),
    ("endpoint", Shape::Text),
    ("extra_info", Shape::Text),
    ("openrtb", Shape::Object(OPENRTB)),
    ("maintainer", Shape::Object(MAINTAINER)),
    ("capabilities", Shape::Object(CAPABILITIES)),
    ("modifyingVastXmlAllowed", Shape::Boolean),
    ("debug", Shape::Object(DEBUG)),
    ("geoscope", Shape::TextList),
    ("gvlVendorID", Shape::VendorId),
    ("userSync", Shape::Object(SYNC)),
    ("experiment", Shape::Object(EXPERIMENT)),
    ("xapi", Shape::Object(XAPI)),
    ("platform_id", Shape::Text),
    ("app_secret", Shape::Text),
    ("endpointCompression", Shape::Text),
    // Historical source spellings ignored by the upstream top-level model.
    ("usersync", Shape::Object(SYNC)),
    ("enabled", Shape::Boolean),
];
const MAINTAINER: &[(&str, Shape)] = &[("email", Shape::Text), ("gvlVendorID", Shape::VendorId)];
const CAPABILITIES: &[(&str, Shape)] = &[
    ("app", Shape::Object(PLATFORM)),
    ("site", Shape::Object(PLATFORM)),
    ("dooh", Shape::Object(PLATFORM)),
];
const PLATFORM: &[(&str, Shape)] = &[("mediaTypes", Shape::TextList)];
const DEBUG: &[(&str, Shape)] = &[("allow", Shape::Boolean)];
const OPENRTB: &[(&str, Shape)] = &[
    // The pin uses both quoted "2.6" and numeric 2.6; retain their source types.
    ("version", Shape::Version),
    ("gpp-supported", Shape::Boolean),
    ("gpp_supported", Shape::Boolean),
    ("multiformat-supported", Shape::Boolean),
];
const SYNC: &[(&str, Shape)] = &[
    ("key", Shape::Text),
    ("supports", Shape::TextList),
    ("iframe", Shape::Object(SYNC_ENDPOINT)),
    ("redirect", Shape::Object(SYNC_ENDPOINT)),
    ("externalUrl", Shape::Text),
    ("formatOverride", Shape::Text),
    ("enabled", Shape::Boolean),
    ("skipwhen", Shape::Object(SKIP_WHEN)),
    // Present in sspBC.yaml, but not a field of the upstream Syncer model.
    ("default", Shape::Text),
];
const SYNC_ENDPOINT: &[(&str, Shape)] = &[
    ("url", Shape::Text),
    ("redirectUrl", Shape::Text),
    ("externalUrl", Shape::Text),
    ("userMacro", Shape::Text),
];
const SKIP_WHEN: &[(&str, Shape)] = &[("gdpr", Shape::Boolean), ("gpp_sid", Shape::TextList)];
const XAPI: &[(&str, Shape)] = &[
    ("username", Shape::Text),
    ("password", Shape::Text),
    ("tracker", Shape::Text),
];
const EXPERIMENT: &[(&str, Shape)] = &[("adsCert", Shape::Object(ADS_CERT))];
const ADS_CERT: &[(&str, Shape)] = &[("enabled", Shape::Boolean)];

fn validate(
    value: &Value,
    shape: Shape,
    path: &str,
    ignored_fields: &mut Vec<String>,
) -> Result<(), Error> {
    // An explicit null is a source fact, not an absent field or a source-model default.
    if matches!(value, Value::Null) {
        return Ok(());
    }
    match (shape, value) {
        (Shape::Text, Value::String(_))
        | (Shape::Boolean, Value::Bool(_))
        | (Shape::Version, Value::String(_) | Value::Number(_)) => Ok(()),
        (Shape::VendorId, Value::Number(number))
            if number.as_u64().is_some_and(|id| u16::try_from(id).is_ok()) =>
        {
            Ok(())
        }
        (Shape::TextList, Value::Sequence(values))
            if values.iter().all(|item| matches!(item, Value::String(_))) =>
        {
            Ok(())
        }
        (Shape::Object(fields), Value::Mapping(values)) => {
            for (raw_key, field_value) in values {
                let Value::String(key) = raw_key else {
                    return Err(Error::InvalidSource);
                };
                let (_, field_shape) = fields
                    .iter()
                    .find(|(name, _shape)| *name == key.as_str())
                    .ok_or(Error::InvalidSource)?;
                let field_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                // An ignored object path marks its entire subtree as unnormalized.
                if matches!(
                    field_path.as_str(),
                    "usersync"
                        | "enabled"
                        | "userSync.default"
                        | "usersync.default"
                        | "maintainer.gvlVendorID"
                        | "openrtb.gpp_supported"
                ) {
                    ignored_fields.push(field_path.clone());
                }
                validate(field_value, *field_shape, &field_path, ignored_fields)?;
            }
            Ok(())
        }
        _ => Err(Error::InvalidSource),
    }
}

fn json_number(number: &Number) -> Result<JsonValue, Error> {
    if let Some(unsigned) = number.as_u64() {
        return Ok(JsonValue::Number(unsigned.into()));
    }
    if let Some(signed) = number.as_i64() {
        return Ok(JsonValue::Number(signed.into()));
    }
    let float = number.as_f64().ok_or(Error::InvalidSource)?;
    let json_float = serde_json::Number::from_f64(float).ok_or(Error::InvalidSource)?;
    // Reject nonfinite values and floats the configured JSON parser cannot retain.
    // Comparing bits also retains the sign of a declared negative zero.
    let bytes = serde_json::to_vec(&json_float).map_err(|_error| Error::InvalidSource)?;
    let decoded: serde_json::Number =
        serde_json::from_slice(&bytes).map_err(|_error| Error::InvalidSource)?;
    if decoded.as_f64().map(f64::to_bits) != Some(float.to_bits()) {
        return Err(Error::InvalidSource);
    }
    Ok(JsonValue::Number(json_float))
}

fn json(input: Value) -> Result<JsonValue, Error> {
    match input {
        Value::Null => Ok(JsonValue::Null),
        Value::Bool(boolean) => Ok(JsonValue::Bool(boolean)),
        Value::String(text) => Ok(JsonValue::String(text)),
        Value::Number(number) => json_number(&number),
        Value::Sequence(values) => values
            .into_iter()
            .map(json)
            .collect::<Result<Vec<_>, _>>()
            .map(JsonValue::Array),
        Value::Mapping(values) => {
            let mut object = serde_json::Map::new();
            for (raw_key, field_value) in values {
                let Value::String(key) = raw_key else {
                    return Err(Error::InvalidSource);
                };
                if object.insert(key, json(field_value)?).is_some() {
                    return Err(Error::InvalidSource);
                }
            }
            object.sort_keys();
            Ok(JsonValue::Object(object))
        }
        Value::Tagged(_) => Err(Error::InvalidSource),
    }
}

pub(super) fn convert(value: Value) -> Result<Converted, Error> {
    if !matches!(&value, Value::Mapping(_)) {
        return Err(Error::InvalidSource);
    }
    let mut ignored_fields = Vec::new();
    validate(&value, Shape::Object(BIDDER), "", &mut ignored_fields)?;
    let document = json(value)?;
    ignored_fields.sort_unstable();
    Ok(Converted {
        document,
        ignored_fields,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn yaml(source: &str) -> Value {
        serde_yaml_ng::from_str(source).unwrap()
    }

    fn at(path: &str, mut value: Value) -> Value {
        for key in path.rsplit('.') {
            let mut object = serde_yaml_ng::Mapping::new();
            object.insert(Value::String(key.to_owned()), value);
            value = Value::Mapping(object);
        }
        value
    }

    fn invalid(value: Value) {
        assert!(matches!(convert(value), Err(Error::InvalidSource)));
    }

    #[test]
    fn preserves_typed_source_fields_without_producing_runtime_policy() {
        let source = json!({
            "aliasOf": "fake_parent",
            "gvlVendorID": 0xffff_u64,
            "capabilities": {
                "app": {"mediaTypes": ["banner", "video"]},
                "site": {"mediaTypes": ["native", "audio"]},
                "dooh": {"mediaTypes": []}
            },
            "endpoint": "http://REGION.example.invalid/{{.AccountID}}",
            "geoscope": ["global", "!USA"],
            "maintainer": {"email": "support@example.invalid"},
            "userSync": {
                "key": "fake_parent",
                "supports": ["iframe", "redirect"],
                "iframe": {
                    "url": "https://example.invalid/sync?redirect={{.RedirectURL}}",
                    "redirectUrl": "{{.ExternalURL}}/setuid?uid={{.UserMacro}}",
                    "externalUrl": "https://host.invalid",
                    "userMacro": "$UID"
                },
                "redirect": {
                    "url": "https://example.invalid/pixel",
                    "redirectUrl": "https://host.invalid/setuid",
                    "externalUrl": "https://other.invalid",
                    "userMacro": "[UID]"
                },
                "externalUrl": "https://host.invalid",
                "formatOverride": "b",
                "enabled": false,
                "skipwhen": {"gdpr": true, "gpp_sid": ["7", "8"]}
            },
            "extra_info": "{\"app_endpoint\":\"https://other.invalid\"}",
            "modifyingVastXmlAllowed": true,
            "disabled": false,
            "endpointCompression": "GZIP",
            "openrtb": {
                "version": "2.6",
                "gpp-supported": true,
                "multiformat-supported": false
            },
            "platform_id": "123",
            "xapi": {"username": "user", "password": "password", "tracker": "tracker"},
            "whiteLabelOnly": true,
            "debug": {"allow": false},
            "app_secret": "source-placeholder",
            "experiment": {"adsCert": {"enabled": true}}
        });
        let value = serde_yaml_ng::to_value(&source).unwrap();
        let converted = convert(value).unwrap();
        assert_eq!(converted.document, source);
        assert!(converted.ignored_fields.is_empty());
        let serialized = serde_json::to_value(converted).unwrap();
        assert_eq!(
            serialized,
            json!({"document": source, "ignored_fields": []})
        );
    }

    #[test]
    fn preserves_ignored_spellings_without_normalizing_or_activating_them() {
        let converted = convert(yaml(
            "usersync:\n  default: redirect\n  enabled: true\n  redirect:\n    url: https://ignored.invalid\nuserSync:\n  default: iframe\nenabled: true\n",
        ))
        .unwrap();
        assert_eq!(
            converted.document,
            json!({
                "usersync": {
                    "default": "redirect", "enabled": true,
                    "redirect": {"url": "https://ignored.invalid"}
                },
                "userSync": {"default": "iframe"},
                "enabled": true
            })
        );
        assert_eq!(
            converted.ignored_fields,
            [
                "enabled",
                "userSync.default",
                "usersync",
                "usersync.default"
            ]
        );
        assert!(converted.document["userSync"].get("redirect").is_none());
        assert!(converted.document.get("disabled").is_none());
        assert!(converted.document.get("defaultEnabled").is_none());
        let only_ignored = convert(yaml("usersync: {key: historical}\n")).unwrap();
        assert!(only_ignored.document.get("userSync").is_none());
        assert_eq!(only_ignored.ignored_fields, ["usersync"]);
    }

    #[test]
    fn preserves_ignored_nested_fields_without_promoting_or_overwriting_them() {
        let source = json!({
            "maintainer": {"gvlVendorID": 440_u64},
            "openrtb": {"gpp_supported": true}
        });
        let converted = convert(serde_yaml_ng::to_value(&source).unwrap()).unwrap();
        assert_eq!(converted.document, source);
        assert_eq!(
            converted.ignored_fields,
            ["maintainer.gvlVendorID", "openrtb.gpp_supported"]
        );
        assert!(converted.document.get("gvlVendorID").is_none());
        assert!(converted.document["openrtb"].get("gpp-supported").is_none());

        let conflicting_source = json!({
            "gvlVendorID": 0_u64,
            "maintainer": {"gvlVendorID": 440_u64},
            "openrtb": {"gpp_supported": true, "gpp-supported": false}
        });
        let conflicting_converted =
            convert(serde_yaml_ng::to_value(&conflicting_source).unwrap()).unwrap();
        assert_eq!(conflicting_converted.document, conflicting_source);
        assert_eq!(
            conflicting_converted.ignored_fields,
            ["maintainer.gvlVendorID", "openrtb.gpp_supported"]
        );
    }

    #[test]
    fn retains_declared_null_instead_of_omitting_or_defaulting_it() {
        for path in [
            "aliasOf",
            "gvlVendorID",
            "capabilities",
            "endpoint",
            "geoscope",
            "maintainer",
            "userSync",
            "extra_info",
            "modifyingVastXmlAllowed",
            "disabled",
            "endpointCompression",
            "openrtb",
            "platform_id",
            "usersync",
            "xapi",
            "enabled",
            "whiteLabelOnly",
            "debug",
            "app_secret",
            "experiment",
            "capabilities.app",
            "capabilities.site",
            "capabilities.dooh",
            "capabilities.app.mediaTypes",
            "capabilities.site.mediaTypes",
            "capabilities.dooh.mediaTypes",
            "maintainer.email",
            "debug.allow",
            "openrtb.version",
            "openrtb.gpp-supported",
            "openrtb.multiformat-supported",
            "userSync.key",
            "userSync.supports",
            "userSync.iframe",
            "userSync.redirect",
            "userSync.externalUrl",
            "userSync.formatOverride",
            "userSync.enabled",
            "userSync.skipwhen",
            "userSync.default",
            "userSync.iframe.url",
            "userSync.iframe.redirectUrl",
            "userSync.iframe.externalUrl",
            "userSync.iframe.userMacro",
            "userSync.redirect.url",
            "userSync.redirect.redirectUrl",
            "userSync.redirect.externalUrl",
            "userSync.redirect.userMacro",
            "userSync.skipwhen.gdpr",
            "userSync.skipwhen.gpp_sid",
            "usersync.default",
            "xapi.username",
            "xapi.password",
            "xapi.tracker",
            "experiment.adsCert",
            "experiment.adsCert.enabled",
        ] {
            let input = at(path, Value::Null);
            let converted = convert(input.clone()).unwrap();
            let restored: Value = serde_yaml_ng::to_value(&converted.document).unwrap();
            assert_eq!(restored, input);
        }
        let empty = convert(yaml("{}")).unwrap();
        assert_eq!(empty.document, json!({}));
        assert!(empty.ignored_fields.is_empty());
        let nulls = convert(yaml(
            "enabled: null\nusersync: null\nuserSync: {default: null}",
        ))
        .unwrap();
        assert_eq!(
            nulls.ignored_fields,
            ["enabled", "userSync.default", "usersync"]
        );
        assert_ne!(nulls.document, empty.document);
    }

    #[test]
    fn rejects_unknown_fields_and_nearby_case_or_runtime_spellings() {
        for path in [
            "unknown",
            "AliasOf",
            "aliasof",
            "gvlVendorId",
            "UserSync",
            "user_sync",
            "Disabled",
            "defaultEnabled",
            "callable_endpoint",
            "endpoint_url",
            "adapters",
            "default",
            "platformId",
            "appSecret",
            "extraInfo",
            "capabilities.unknown",
            "capabilities.App",
            "capabilities.DOOH",
            "capabilities.app.MediaTypes",
            "capabilities.site.mediatypes",
            "capabilities.dooh.unknown",
            "maintainer.Email",
            "maintainer.name",
            "debug.Allow",
            "debug.enabled",
            "openrtb.gppSupported",
            "openrtb.Version",
            "openrtb.multiformatSupported",
            "userSync.unknown",
            "userSync.skipWhen",
            "userSync.format_override",
            "userSync.external_url",
            "userSync.Enabled",
            "userSync.iframe.URL",
            "userSync.iframe.redirect_url",
            "userSync.iframe.enabled",
            "userSync.redirect.user_macro",
            "userSync.redirect.unknown",
            "userSync.skipwhen.GDPR",
            "userSync.skipwhen.gppSid",
            "usersync.unknown",
            "usersync.redirect.unknown",
            "usersync.skipwhen.unknown",
            "xapi.user",
            "xapi.pass",
            "xapi.Username",
            "experiment.unknown",
            "experiment.adscert",
            "experiment.adsCert.unknown",
        ] {
            invalid(at(path, Value::Null));
        }
    }

    #[test]
    fn rejects_nonobject_documents_and_wrong_nested_object_shapes() {
        for source in ["null", "true", "17", "string", "[]"] {
            invalid(yaml(source));
        }
        for path in [
            "capabilities",
            "capabilities.app",
            "capabilities.site",
            "capabilities.dooh",
            "maintainer",
            "debug",
            "openrtb",
            "userSync",
            "userSync.iframe",
            "userSync.redirect",
            "userSync.skipwhen",
            "usersync",
            "usersync.iframe",
            "usersync.redirect",
            "usersync.skipwhen",
            "xapi",
            "experiment",
            "experiment.adsCert",
        ] {
            for source in ["true", "17", "string", "[]"] {
                invalid(at(path, yaml(source)));
            }
        }
    }

    #[test]
    fn rejects_string_and_boolean_coercions_including_ignored_fields() {
        for path in [
            "aliasOf",
            "endpoint",
            "extra_info",
            "endpointCompression",
            "platform_id",
            "app_secret",
            "maintainer.email",
            "userSync.key",
            "userSync.externalUrl",
            "userSync.formatOverride",
            "userSync.default",
            "userSync.iframe.url",
            "userSync.iframe.redirectUrl",
            "userSync.iframe.externalUrl",
            "userSync.iframe.userMacro",
            "userSync.redirect.url",
            "userSync.redirect.redirectUrl",
            "userSync.redirect.externalUrl",
            "userSync.redirect.userMacro",
            "usersync.key",
            "usersync.default",
            "usersync.redirect.url",
            "xapi.username",
            "xapi.password",
            "xapi.tracker",
        ] {
            for source in ["true", "17", "2.6", "[]", "{}"] {
                invalid(at(path, yaml(source)));
            }
        }
        for path in [
            "disabled",
            "modifyingVastXmlAllowed",
            "whiteLabelOnly",
            "enabled",
            "debug.allow",
            "openrtb.gpp-supported",
            "openrtb.multiformat-supported",
            "userSync.enabled",
            "userSync.skipwhen.gdpr",
            "usersync.enabled",
            "usersync.skipwhen.gdpr",
            "experiment.adsCert.enabled",
        ] {
            for source in ["'true'", "1", "0", "[]", "{}"] {
                invalid(at(path, yaml(source)));
            }
        }
        for source in ["true", "[]", "{}"] {
            invalid(at("openrtb.version", yaml(source)));
        }
    }

    #[test]
    fn rejects_nonstring_lists_and_items_without_dropping_them() {
        for path in [
            "geoscope",
            "capabilities.app.mediaTypes",
            "capabilities.site.mediaTypes",
            "capabilities.dooh.mediaTypes",
            "userSync.supports",
            "userSync.skipwhen.gpp_sid",
            "usersync.supports",
            "usersync.skipwhen.gpp_sid",
        ] {
            for source in [
                "string", "true", "1", "{}", "[1]", "[true]", "[null]", "[{}]", "[[]]",
            ] {
                invalid(at(path, yaml(source)));
            }
        }
    }

    #[test]
    fn enforces_the_unsigned_16_bit_vendor_id_contract() {
        for source in [
            "-1",
            "65536",
            "18446744073709551615",
            "1.0",
            "'1'",
            "true",
            "[]",
            "{}",
        ] {
            invalid(at("gvlVendorID", yaml(source)));
        }
        for value in [0_u16, 1_u16, u16::MAX] {
            let converted = convert(at("gvlVendorID", Value::Number(value.into()))).unwrap();
            assert_eq!(converted.document, json!({"gvlVendorID": value}));
        }
    }

    #[test]
    fn rejects_nonstring_mapping_keys_instead_of_stringifying_them() {
        for key in [
            yaml("1"),
            yaml("true"),
            Value::Null,
            yaml("[key]"),
            yaml("{key: value}"),
        ] {
            let mut object = serde_yaml_ng::Mapping::new();
            object.insert(key, Value::Null);
            invalid(Value::Mapping(object.clone()));
            for path in [
                "maintainer",
                "capabilities.app",
                "userSync",
                "usersync.redirect",
                "xapi",
            ] {
                invalid(at(path, Value::Mapping(object.clone())));
            }
        }
    }

    #[test]
    fn preserves_numeric_and_string_versions_as_distinct_source_values() {
        for (source, expected) in [
            ("2.6", json!(2.6_f64)),
            ("'2.6'", json!("2.6")),
            ("2", json!(2_i32)),
        ] {
            let converted = convert(at("openrtb.version", yaml(source))).unwrap();
            assert_eq!(
                converted.document,
                json!({"openrtb": {"version": expected}})
            );
        }
        let negative_zero =
            convert(at("openrtb.version", Value::Number((-0.0_f64).into()))).unwrap();
        assert_eq!(
            negative_zero.document["openrtb"]["version"]
                .as_f64()
                .unwrap()
                .to_bits(),
            (-0.0_f64).to_bits()
        );
    }

    #[test]
    fn preserves_integer_number_boundaries_without_float_coercion() {
        for (source, expected) in [
            ("-9223372036854775808", json!(i64::MIN)),
            ("-1", json!(-1_i32)),
            ("0", json!(0_i32)),
            ("9223372036854775807", json!(i64::MAX)),
            ("9007199254740993", json!(0x0020_0000_0000_0001_u64)),
            ("18446744073709551615", json!(u64::MAX)),
        ] {
            let converted = convert(at("openrtb.version", yaml(source))).unwrap();
            assert_eq!(
                converted.document,
                json!({"openrtb": {"version": expected}})
            );
            let encoded = serde_json::to_vec(&converted.document).unwrap();
            let decoded: JsonValue = serde_json::from_slice(&encoded).unwrap();
            assert_eq!(decoded, converted.document);
        }
    }

    #[test]
    fn rejects_nonfinite_numbers_and_float_json_roundtrip_changes() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            invalid(at("openrtb.version", Value::Number(value.into())));
        }
        for value in [
            2.6_f64,
            -0.0_f64,
            0.1_f64,
            1.000_000_000_000_000_2_f64,
            1e23_f64,
            1e-300_f64,
            f64::MIN_POSITIVE,
            f64::MAX,
            f64::from_bits(1_u64),
        ] {
            let number = serde_json::Number::from_f64(value).unwrap();
            let encoded = serde_json::to_vec(&number).unwrap();
            let decoded_result = serde_json::from_slice::<serde_json::Number>(&encoded);
            let roundtrips = decoded_result
                .ok()
                .and_then(|parsed_number| parsed_number.as_f64())
                .is_some_and(|decoded_float| decoded_float.to_bits() == value.to_bits());
            let result = convert(at("openrtb.version", Value::Number(value.into())));
            if roundtrips {
                let converted = result.unwrap();
                assert_eq!(
                    converted.document["openrtb"]["version"]
                        .as_f64()
                        .unwrap()
                        .to_bits(),
                    value.to_bits()
                );
            } else {
                assert!(matches!(result, Err(Error::InvalidSource)));
            }
        }
    }

    #[test]
    fn rejects_tags_even_when_called_without_the_parent_preflight() {
        for source in [
            "!source {}",
            "maintainer: !source {}",
            "endpoint: !source string",
            "enabled: !source null",
            "geoscope: [!source string]",
        ] {
            invalid(yaml(source));
        }
    }

    #[test]
    fn canonical_json_and_ignored_paths_are_independent_of_mapping_order() {
        let first = convert(yaml("usersync: {key: ignored}\nenabled: false\nuserSync: {default: iframe, key: actual}\nmaintainer: {email: support@example.invalid}")).unwrap();
        let second = convert(yaml("maintainer: {email: support@example.invalid}\nuserSync: {key: actual, default: iframe}\nenabled: false\nusersync: {key: ignored}")).unwrap();
        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&second).unwrap()
        );
    }
}
