//! Bounded structural inputs for the user-sync HTTP endpoints.

use std::{collections::BTreeMap, fmt, marker::PhantomData};

use auctioneer_config::usersync::{
    MAX_SYNC_QUERY_PARAMS, SyncQueryEncodingError, validate_sync_query_encoding,
};
use auctioneer_runtime::{BoundedJsonError, count_json_nodes};
use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};
use serde_json::{Number, Value, value::RawValue};
use url::form_urlencoded;

pub const MAX_COOKIE_SYNC_BODY_BYTES: usize = 32 * 1024;
pub const MAX_QUERY_BYTES: usize = 32 * 1024;
pub const MAX_QUERY_PARAMS: usize = MAX_SYNC_QUERY_PARAMS;

// Derived struct deserializers accept positional arrays. These wire roots
// require objects without losing borrowed fields or duplicate-field checks.
pub(super) struct ObjectOnly<T>(T);

impl<T> ObjectOnly<T> {
    pub(super) fn into_inner(self) -> T {
        self.0
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for ObjectOnly<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor<T>(PhantomData<T>);

        #[expect(
            clippy::missing_trait_methods,
            reason = "Serde's provided visitor methods reject non-map values; its private hooks must remain library-owned"
        )]
        impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectVisitor<T> {
            type Value = ObjectOnly<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a JSON object")
            }

            fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
                T::deserialize(MapAccessDeserializer::new(map)).map(ObjectOnly)
            }
        }

        deserializer.deserialize_map(ObjectVisitor(PhantomData))
    }

    fn deserialize_in_place<D: Deserializer<'de>>(
        deserializer: D,
        place: &mut Self,
    ) -> Result<(), D::Error> {
        *place = Self::deserialize(deserializer)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WireError {
    BodyTooLong,
    QueryTooLong,
    TooManyParameters,
    InvalidPercentEncoding,
    InvalidUtf8,
    InvalidJson,
    JsonNodeLimit,
}

pub struct ParsedQuery {
    first_values: BTreeMap<String, String>,
    parameter_count: usize,
}

impl fmt::Debug for ParsedQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ParsedQuery")
            .field("parameter_count", &self.parameter_count)
            .finish_non_exhaustive()
    }
}

impl ParsedQuery {
    pub fn first(&self, name: &str) -> Option<&str> {
        self.first_values.get(name).map(String::as_str)
    }

    #[must_use]
    pub fn parameter_count(&self) -> usize {
        self.parameter_count
    }
}

/// # Errors
/// Rejects excessive bytes/parameters, malformed percent escapes, or decoded
/// values that are not UTF-8 before retaining query fields.
pub fn parse_setuid_query(query: Option<&str>) -> Result<ParsedQuery, WireError> {
    let raw = query.unwrap_or_default();
    if raw.len() > MAX_QUERY_BYTES {
        return Err(WireError::QueryTooLong);
    }
    let parameter_count = if raw.is_empty() {
        0
    } else {
        raw.split('&').count()
    };
    if parameter_count > MAX_QUERY_PARAMS {
        return Err(WireError::TooManyParameters);
    }
    validate_sync_query_encoding(raw).map_err(|error| match error {
        SyncQueryEncodingError::InvalidPercentEncoding => WireError::InvalidPercentEncoding,
        SyncQueryEncodingError::InvalidUtf8 => WireError::InvalidUtf8,
    })?;
    let mut first_values = BTreeMap::new();
    for (name, value) in form_urlencoded::parse(raw.as_bytes()) {
        first_values
            .entry(name.into_owned())
            .or_insert_with(|| value.into_owned());
    }
    Ok(ParsedQuery {
        first_values,
        parameter_count,
    })
}

#[expect(
    clippy::partial_pub_fields,
    reason = "raw filters stay private so callers use deferred shape validation; other wire fields are already typed"
)]
#[derive(Default, Deserialize)]
#[serde(default)]
pub struct CookieSyncWire {
    pub bidders: Vec<String>,
    pub gdpr: Option<Box<RawValue>>,
    pub gdpr_consent: Option<String>,
    pub us_privacy: Option<String>,
    pub gpp: Option<String>,
    pub gpp_sid: Option<String>,
    #[serde(rename = "coopSync")]
    pub coop_sync: Option<bool>,
    pub limit: Option<Number>,
    #[serde(rename = "filterSettings")]
    filter_settings: Option<Box<RawValue>>,
    pub account: Option<String>,
    pub debug: bool,
}

impl fmt::Debug for CookieSyncWire {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CookieSyncWire")
            .field("bidder_count", &self.bidders.len())
            .field("has_filter_settings", &self.filter_settings.is_some())
            .finish_non_exhaustive()
    }
}

impl CookieSyncWire {
    /// # Errors
    /// Returns `InvalidJson` for an invalid filter object or unknown fields.
    pub fn parse_filter_settings(&self) -> Result<Option<FilterSettingsWire>, WireError> {
        self.filter_settings
            .as_ref()
            .map(|value| serde_json::from_str(value.get()).map_err(|_error| WireError::InvalidJson))
            .transpose()
    }

    /// # Errors
    /// Returns `InvalidJson` when the filter object fails structural validation.
    pub fn filter_settings_value(&self) -> Result<Option<Value>, WireError> {
        self.parse_filter_settings()?;
        self.filter_settings_unchecked_value()
    }

    /// Diagnostic-only view. Callers must still use `filter_settings_value`
    /// before accepting a filter shape.
    ///
    /// # Errors
    /// Returns `InvalidJson` when the raw filter value cannot be deserialized.
    pub fn filter_settings_unchecked_value(&self) -> Result<Option<Value>, WireError> {
        self.filter_settings
            .as_ref()
            .map(|value| serde_json::from_str(value.get()).map_err(|_error| WireError::InvalidJson))
            .transpose()
    }
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FilterSettingsWire {
    pub iframe: Option<FilterRuleWire>,
    pub image: Option<FilterRuleWire>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FilterRuleWire {
    pub bidders: Option<FilterBiddersWire>,
    pub filter: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum FilterBiddersWire {
    Literal(String),
    Names(Vec<String>),
}

/// # Errors
/// Rejects excessive body bytes or JSON nodes, non-object roots, invalid JSON,
/// and wrong field types.
pub fn parse_cookie_sync_body(
    body: &[u8],
    max_json_nodes: u32,
) -> Result<CookieSyncWire, WireError> {
    if body.len() > MAX_COOKIE_SYNC_BODY_BYTES {
        return Err(WireError::BodyTooLong);
    }
    // Count every subtree before capturing RawValue fields. Typed parsing uses
    // the same byte-capped body and the handler's prepaid aux parse allowance.
    count_json_nodes(body, max_json_nodes).map_err(|error| match error {
        BoundedJsonError::Syntax(_) => WireError::InvalidJson,
        BoundedJsonError::NodeBudget(_) | BoundedJsonError::RawValue(_) => WireError::JsonNodeLimit,
    })?;
    serde_json::from_slice::<ObjectOnly<CookieSyncWire>>(body)
        .map(ObjectOnly::into_inner)
        .map_err(|_error| WireError::InvalidJson)
}

#[cfg(test)]
mod tests {
    use serde::Deserialize as _;
    use serde_json::json;

    use super::{
        CookieSyncWire, MAX_COOKIE_SYNC_BODY_BYTES, MAX_QUERY_BYTES, MAX_QUERY_PARAMS, ObjectOnly,
        WireError, parse_cookie_sync_body, parse_setuid_query,
    };

    #[test]
    fn query_decodes_once_and_first_duplicate_wins() {
        let parsed = parse_setuid_query(Some(
            "bidder=fake_first&bidder=fake_second&uid=%252F+%E2%98%83&unused=x",
        ))
        .unwrap();
        assert_eq!(parsed.first("bidder"), Some("fake_first"));
        assert_eq!(parsed.first("uid"), Some("%2F \u{2603}"));
        assert_eq!(parsed.first("unused"), Some("x"));
        assert_eq!(parsed.parameter_count(), 4);
    }

    #[test]
    fn query_counts_unknown_and_duplicate_parameters_before_retention() {
        let accepted = (0..MAX_QUERY_PARAMS)
            .map(|index| format!("unknown={index}"))
            .collect::<Vec<_>>()
            .join("&");
        let parsed = parse_setuid_query(Some(&accepted)).unwrap();
        assert_eq!(parsed.parameter_count(), MAX_QUERY_PARAMS);
        assert_eq!(parsed.first("unknown"), Some("0"));
        assert_eq!(
            parse_setuid_query(Some(&format!("{accepted}&unknown=overflow"))).unwrap_err(),
            WireError::TooManyParameters,
        );
    }

    #[test]
    fn query_rejects_bad_percent_syntax_and_invalid_decoded_utf8() {
        for query in ["uid=%", "uid=%2", "uid=%GG", "uid=%FF", "uid=%E2%82"] {
            assert!(parse_setuid_query(Some(query)).is_err(), "{query}");
        }
        assert_eq!(
            parse_setuid_query(Some("uid=%252F")).unwrap().first("uid"),
            Some("%2F")
        );
    }

    #[test]
    fn query_byte_cap_is_checked_before_percent_or_parameter_work() {
        let exact = format!("x={}", "a".repeat(MAX_QUERY_BYTES - 2));
        parse_setuid_query(Some(&exact)).unwrap();
        let oversized = format!("{}%", "a".repeat(MAX_QUERY_BYTES));
        assert_eq!(
            parse_setuid_query(Some(&oversized)).unwrap_err(),
            WireError::QueryTooLong
        );
    }

    #[test]
    fn cookie_sync_json_enforces_body_cap_before_parse() {
        let exact = format!(
            "{{\"unknown\":\"{}\"}}",
            "a".repeat(MAX_COOKIE_SYNC_BODY_BYTES - 14)
        );
        assert_eq!(exact.len(), MAX_COOKIE_SYNC_BODY_BYTES);
        parse_cookie_sync_body(exact.as_bytes(), 16).unwrap();
        let oversized = format!("{}%", "a".repeat(MAX_COOKIE_SYNC_BODY_BYTES));
        assert_eq!(
            parse_cookie_sync_body(oversized.as_bytes(), 16).unwrap_err(),
            WireError::BodyTooLong,
        );
    }

    #[test]
    fn cookie_sync_json_rejects_wrong_known_types_but_defers_legacy_gdpr_value() {
        for value in [json!({"bidders": "not-an-array"}), json!({"debug": "yes"})] {
            let raw = serde_json::to_vec(&value).unwrap();
            assert_eq!(
                parse_cookie_sync_body(&raw, 64).unwrap_err(),
                WireError::InvalidJson
            );
        }
        let raw = serde_json::to_vec(&json!({"gdpr": "bad", "bidders": ["fake_A"]})).unwrap();
        let parsed = parse_cookie_sync_body(&raw, 64).unwrap();
        assert_eq!(parsed.bidders, ["fake_A"]);
        assert_eq!(serde_json::to_string(&parsed.gdpr).unwrap(), "\"bad\"");
        let filter_body = serde_json::to_vec(&json!({
            "filterSettings": {"image": {"bidders": 42_i32}}
        }))
        .unwrap();
        let deferred = parse_cookie_sync_body(&filter_body, 64).unwrap();
        assert!(matches!(
            deferred.parse_filter_settings(),
            Err(WireError::InvalidJson)
        ));
    }

    #[test]
    fn cookie_sync_json_rejects_non_object_roots() {
        for value in [
            json!([]),
            json!([[]]),
            json!([[], 0_i32]),
            json!([
                ["fake_aaa"],
                0_i32,
                null,
                null,
                null,
                null,
                false,
                1_i32,
                null,
                "publisher",
                true
            ]),
            json!(null),
            json!(true),
            json!(0_i32),
            json!("request"),
        ] {
            let raw = serde_json::to_vec(&value).unwrap();
            assert_eq!(
                parse_cookie_sync_body(&raw, 64).unwrap_err(),
                WireError::InvalidJson,
                "{value}"
            );
        }
    }

    #[test]
    fn cookie_sync_non_object_roots_keep_resource_limit_precedence() {
        let raw = serde_json::to_vec(&json!([[], 0_i32])).unwrap();
        assert_eq!(
            parse_cookie_sync_body(&raw, 2).unwrap_err(),
            WireError::JsonNodeLimit
        );
        assert_eq!(
            parse_cookie_sync_body(&raw, 3).unwrap_err(),
            WireError::InvalidJson
        );
        let oversized =
            serde_json::to_vec(&json!(["x".repeat(MAX_COOKIE_SYNC_BODY_BYTES)])).unwrap();
        assert_eq!(
            parse_cookie_sync_body(&oversized, 1).unwrap_err(),
            WireError::BodyTooLong
        );
    }

    #[test]
    fn cookie_sync_object_roots_preserve_defaults_and_deferred_nested_values() {
        let empty = serde_json::to_vec(&json!({})).unwrap();
        let defaults = parse_cookie_sync_body(&empty, 1).unwrap();
        assert!(defaults.bidders.is_empty());
        assert!(defaults.gdpr.is_none());
        assert!(defaults.coop_sync.is_none());

        let filter = json!({"image": {"bidders": 42_i32}});
        let raw = serde_json::to_vec(&json!({
            "bidders": ["fake_aaa"],
            "gdpr": [1_i32],
            "filterSettings": filter,
            "unknown": {"nested": [true, false]},
        }))
        .unwrap();
        let parsed = parse_cookie_sync_body(&raw, 64).unwrap();
        assert_eq!(parsed.bidders, ["fake_aaa"]);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(parsed.gdpr.as_ref().unwrap().get()).unwrap(),
            json!([1_i32])
        );
        assert_eq!(
            parsed.filter_settings_unchecked_value().unwrap(),
            Some(filter)
        );
        assert_eq!(
            parsed.filter_settings_value().unwrap_err(),
            WireError::InvalidJson
        );
    }

    #[test]
    fn object_only_in_place_deserialization_preserves_state_on_rejection() {
        let raw = serde_json::to_vec(&json!({"bidders": ["fake_aaa"], "gdpr": 1_i32})).unwrap();
        let mut value: ObjectOnly<CookieSyncWire> = serde_json::from_slice(&raw).unwrap();
        let rejected = serde_json::to_vec(&json!([[], 0_i32])).unwrap();
        let mut rejected_deserializer = serde_json::Deserializer::from_slice(&rejected);
        assert!(ObjectOnly::deserialize_in_place(&mut rejected_deserializer, &mut value).is_err());
        assert_eq!(value.0.bidders, ["fake_aaa"]);
        assert_eq!(value.0.gdpr.as_ref().unwrap().get(), "1");

        let replacement =
            serde_json::to_vec(&json!({"bidders": ["fake_bbb"], "gdpr": 0_i32})).unwrap();
        let mut replacement_deserializer = serde_json::Deserializer::from_slice(&replacement);
        ObjectOnly::deserialize_in_place(&mut replacement_deserializer, &mut value).unwrap();
        replacement_deserializer.end().unwrap();
        assert_eq!(value.0.bidders, ["fake_bbb"]);
        assert_eq!(value.0.gdpr.as_ref().unwrap().get(), "0");
    }

    #[test]
    fn cookie_sync_coop_sync_preserves_absence_and_explicit_values() {
        for (body, expected) in [
            (json!({}), None),
            (json!({"coopSync": false}), Some(false)),
            (json!({"coopSync": true}), Some(true)),
        ] {
            let raw = serde_json::to_vec(&body).unwrap();
            assert_eq!(
                parse_cookie_sync_body(&raw, 16).unwrap().coop_sync,
                expected
            );
        }
    }

    #[test]
    fn cookie_sync_filter_settings_rejects_unknown_keys_at_each_level() {
        for value in [
            json!({"filterSettings": {"other": {}}}),
            json!({"filterSettings": {"iframe": {"other": true}}}),
            json!({"filterSettings": {"image": {"other": true}}}),
        ] {
            let raw = serde_json::to_vec(&value).unwrap();
            let parsed = parse_cookie_sync_body(&raw, 32).unwrap();
            assert!(matches!(
                parsed.parse_filter_settings(),
                Err(WireError::InvalidJson)
            ));
        }
    }

    #[test]
    fn valid_sid_can_override_out_of_range_legacy_gdpr_number_after_structural_parse() {
        // Literal exemption: 1e400 is valid JSON but cannot be represented by
        // json!'s Number; this test requires the opaque token to survive parsing.
        let parsed = parse_cookie_sync_body(br#"{"gdpr":1e400,"gpp_sid":"-1"}"#, 64).unwrap();
        assert_eq!(serde_json::to_string(&parsed.gdpr).unwrap(), "1e400");
        assert_eq!(parsed.gpp_sid.as_deref(), Some("-1"));
        // Literal exemption: retain the same out-of-range numeric token inside
        // the deferred filter so its type error is not replaced by serialization.
        let deferred =
            parse_cookie_sync_body(br#"{"filterSettings":{"image":{"bidders":1e400}}}"#, 64)
                .unwrap();
        assert!(matches!(
            deferred.parse_filter_settings(),
            Err(WireError::InvalidJson)
        ));
    }

    #[test]
    fn cookie_sync_json_node_budget_counts_discarded_unknown_values() {
        let raw = serde_json::to_vec(&json!({
            "unknown": [1_i32, 2_i32, 3_i32], "bidders": []
        }))
        .unwrap();
        assert_eq!(
            parse_cookie_sync_body(&raw, 4).unwrap_err(),
            WireError::JsonNodeLimit
        );
        parse_cookie_sync_body(&raw, 9).unwrap();
    }

    #[test]
    fn cookie_sync_json_node_budget_counts_nested_unknown_values_inside_raw_fields() {
        let nested = json!({"unknown": [{"nested": [1_i32, 2_i32]}, 3_i32]});
        for field in ["gdpr", "filterSettings"] {
            let body = serde_json::to_vec(&json!({(field): nested})).unwrap();
            // Root + field key + object + unknown key + array + object +
            // nested key + array + three numbers = 11 syntactic nodes.
            let parsed = parse_cookie_sync_body(&body, 11).unwrap();
            let captured = match field {
                "gdpr" => parsed.gdpr.as_ref().unwrap(),
                _ => parsed.filter_settings.as_ref().unwrap(),
            };
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(captured.get()).unwrap(),
                nested
            );
            assert_eq!(
                parse_cookie_sync_body(&body, 10).unwrap_err(),
                WireError::JsonNodeLimit,
                "{field}"
            );
        }
    }

    #[test]
    fn cookie_sync_json_node_budget_counts_duplicate_unknown_values_inside_raw_fields() {
        // Literal exemption: json! collapses duplicate keys, losing the nested
        // values that must consume the budget even when a later key replaces them.
        let body = br#"{"gdpr":{"unknown":[{"nested":[1,2]},3],"unknown":0},"filterSettings":{"unknown":[{"nested":[1,2]},3],"unknown":0}}"#;
        // Root + two (field key + 9-node nested object + duplicate key/value)
        // pairs = 25 syntactic nodes; a retained Value would contain only 9.
        let parsed = parse_cookie_sync_body(body, 25).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(parsed.gdpr.as_ref().unwrap().get()).unwrap(),
            json!({"unknown": 0_i32})
        );
        assert_eq!(
            parsed.filter_settings_unchecked_value().unwrap(),
            Some(json!({"unknown": 0_i32}))
        );
        assert!(matches!(
            parsed.parse_filter_settings(),
            Err(WireError::InvalidJson)
        ));
        assert_eq!(
            parse_cookie_sync_body(body, 24).unwrap_err(),
            WireError::JsonNodeLimit
        );
    }
}
