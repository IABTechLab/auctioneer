//! Authored protocol facts for the shared full-privacy-context fixture.

use serde::Deserialize;
use serde_json::{Map, Value};

const FIXTURE: &str = "full-privacy-context-v1";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivacyWireContract {
    schema_version: u8,
    implementation: String,
    fixture: String,
    publisher: PublisherRule,
    impression_extension: ImpressionExtension,
    request_extension: RequestExtension,
    banner_response_prebid: Map<String, Value>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
enum PublisherRule {
    Preserve,
    Replace { id: String },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
enum ImpressionExtension {
    Preserve,
    Replace { fields: Map<String, Value> },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestExtension {
    preserve_input: bool,
    absent_input: AbsentExtension,
    present_input: Map<String, Value>,
    requested_bidder_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
enum AbsentExtension {
    Missing,
    Object { fields: Map<String, Value> },
}

impl PrivacyWireContract {
    /// Parse facts for one implementation, not observations of its output.
    ///
    /// # Errors
    /// Refuses malformed, unknown, or incorrectly scoped contract facts.
    pub fn parse(bytes: &[u8], implementation: &str) -> Result<Self, String> {
        let facts: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("invalid privacy wire contract: {error}"))?;
        if facts.schema_version != 1
            || facts.fixture != FIXTURE
            || facts.implementation != implementation
        {
            return Err("privacy wire contract ownership or version mismatch".into());
        }
        if let PublisherRule::Replace { id } = &facts.publisher
            && id.is_empty()
        {
            return Err("privacy wire publisher replacement is empty".into());
        }
        if let Some(path) = &facts.request_extension.requested_bidder_path
            && Value::Object(facts.request_extension.present_input.clone())
                .pointer(path)
                .and_then(Value::as_str)
                != Some("")
        {
            return Err("privacy wire requested-bidder slot is not an empty string".into());
        }
        if ["type", "targeting", "meta"]
            .iter()
            .any(|key| facts.banner_response_prebid.contains_key(*key))
        {
            return Err("privacy wire response facts override shared response fields".into());
        }
        Ok(facts)
    }

    /// Additive protocol metadata for a standard banner bid with no response extension.
    #[must_use]
    pub fn banner_response_prebid(&self) -> &Map<String, Value> {
        &self.banner_response_prebid
    }

    /// Whether scoped impression fields survive this protocol's serialization.
    #[must_use]
    pub fn preserves_impression_extension(&self) -> bool {
        matches!(self.impression_extension, ImpressionExtension::Preserve)
    }

    /// Project only independently declared context changes onto a fixture field.
    /// User, device and source fields are never rewritten by these facts.
    ///
    /// # Errors
    /// Refuses a present inventory context without its required publisher field.
    pub fn preserved_value(&self, original: &Value, path: &str) -> Result<Option<Value>, String> {
        if let ImpressionExtension::Replace { fields } = &self.impression_extension
            && let Some(relative) = path.strip_prefix("/imp/0/ext")
            && (relative.is_empty() || relative.starts_with('/'))
        {
            let extension = Value::Object(fields.clone());
            return Ok(extension.pointer(relative).cloned());
        }
        let Some(mut expected) = original.pointer(path).cloned() else {
            return Ok(None);
        };
        if matches!(path, "/site" | "/app")
            && let PublisherRule::Replace { id } = &self.publisher
        {
            let publisher = expected
                .pointer_mut("/publisher/id")
                .ok_or("privacy context fixture lacks its publisher ID")?;
            *publisher = Value::String(id.clone());
        }
        Ok(Some(expected))
    }

    /// Resolve exact extension facts for absent or present caller input.
    ///
    /// # Errors
    /// Refuses a non-object input extension or an invalid requested-bidder slot.
    pub fn request_extension(
        &self,
        input: Option<&Value>,
        bidder: &str,
    ) -> Result<Option<Value>, String> {
        let Some(input_value) = input else {
            return Ok(match &self.request_extension.absent_input {
                AbsentExtension::Missing => None,
                AbsentExtension::Object { fields } => Some(Value::Object(fields.clone())),
            });
        };
        let input_fields = input_value
            .as_object()
            .ok_or("privacy context fixture request extension is not an object")?;
        let mut expected = if self.request_extension.preserve_input {
            input_fields.clone()
        } else {
            Map::new()
        };
        expected.extend(self.request_extension.present_input.clone());
        let mut result = Value::Object(expected);
        if let Some(path) = &self.request_extension.requested_bidder_path {
            let slot = result
                .pointer_mut(path)
                .ok_or("privacy wire requested-bidder slot is missing")?;
            *slot = Value::String(bidder.to_owned());
        }
        Ok(Some(result))
    }
}

#[cfg(test)]
mod tests {
    use super::PrivacyWireContract;
    use serde_json::{Value, json};

    fn record() -> Value {
        json!({
            "schema_version": 1_u8,
            "implementation": "fake_fixture",
            "fixture": "full-privacy-context-v1",
            "publisher": {"mode": "replace", "id": "fake_publisher"},
            "impression_extension": {"mode": "replace", "fields": {"key_val": "slot=top"}},
            "banner_response_prebid": {"video": {"duration": 0_i64, "primary_category": ""}},
            "request_extension": {
                "preserve_input": false,
                "absent_input": {"mode": "object", "fields": {}},
                "present_input": {"wrapper": {"biddercode": ""}},
                "requested_bidder_path": "/wrapper/biddercode"
            }
        })
    }

    fn parse(value: &Value) -> Result<PrivacyWireContract, String> {
        let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
        PrivacyWireContract::parse(&bytes, "fake_fixture")
    }

    #[test]
    fn absent_empty_and_nonempty_request_extensions_have_independent_expectations() {
        let contract = parse(&record()).unwrap();
        assert_eq!(
            contract.request_extension(None, "fake_alias").unwrap(),
            Some(json!({}))
        );
        for input in [json!({}), json!({"prebid": {"nosale": ["*"]}})] {
            assert_eq!(
                contract
                    .request_extension(Some(&input), "fake_alias")
                    .unwrap(),
                Some(json!({"wrapper": {"biddercode": "fake_alias"}}))
            );
        }
        assert_eq!(
            contract.request_extension(Some(&Value::Null), "fake_alias"),
            Err("privacy context fixture request extension is not an object".into())
        );
    }

    #[test]
    fn preserving_extension_facts_keep_caller_fields_without_synthesizing_them() {
        let mut data = record();
        data["request_extension"] = json!({
            "preserve_input": true,
            "absent_input": {"mode": "missing"},
            "present_input": {"wrapper": {"kind": "fake_protocol"}},
            "requested_bidder_path": null
        });
        let contract = parse(&data).unwrap();
        assert_eq!(
            contract.request_extension(None, "fake_alias").unwrap(),
            None
        );
        let original = json!({"caller": {"keep": true}});
        assert_eq!(
            contract
                .request_extension(Some(&original), "fake_alias")
                .unwrap(),
            Some(json!({"caller": {"keep": true}, "wrapper": {"kind": "fake_protocol"}}))
        );
        assert_eq!(original, json!({"caller": {"keep": true}}));
    }

    #[test]
    fn context_facts_cannot_rewrite_user_device_or_source_privacy_fields() {
        let contract = parse(&record()).unwrap();
        let original = json!({
            "site": {"publisher": {"id": "caller"}, "id": "site"},
            "user": {"id": "private-user"},
            "device": {"ifa": "private-device"},
            "source": {"tid": "private-source"},
            "imp": [{"ext": {"tid": "private-imp"}}]
        });
        assert_eq!(
            contract.preserved_value(&original, "/site").unwrap(),
            Some(json!({"publisher": {"id": "fake_publisher"}, "id": "site"}))
        );
        assert_eq!(
            contract
                .preserved_value(&original, "/imp/0/ext/tid")
                .unwrap(),
            None
        );
        assert_eq!(
            contract.preserved_value(&original, "/imp/0/ext").unwrap(),
            Some(json!({"key_val": "slot=top"}))
        );
        for path in ["/user/id", "/device/ifa", "/source/tid"] {
            assert_eq!(
                contract.preserved_value(&original, path).unwrap().as_ref(),
                original.pointer(path)
            );
        }
        assert_eq!(original["site"]["publisher"]["id"], "caller");
        assert_eq!(
            contract.banner_response_prebid(),
            json!({"video": {"duration": 0_i64, "primary_category": ""}})
                .as_object()
                .unwrap()
        );
        assert_eq!(contract.preserved_value(&original, "/app").unwrap(), None);
        assert_eq!(
            contract.preserved_value(&json!({"site": {}}), "/site"),
            Err("privacy context fixture lacks its publisher ID".into())
        );
    }

    #[test]
    fn contracts_reject_unknown_unscoped_and_invalid_binding_facts() {
        for (path, value, message) in [
            (
                "/schema_version",
                json!(2_u8),
                "privacy wire contract ownership or version mismatch",
            ),
            (
                "/implementation",
                json!("fake_other"),
                "privacy wire contract ownership or version mismatch",
            ),
            (
                "/fixture",
                json!("unreviewed-context"),
                "privacy wire contract ownership or version mismatch",
            ),
            (
                "/publisher/id",
                json!(""),
                "privacy wire publisher replacement is empty",
            ),
            (
                "/request_extension/requested_bidder_path",
                json!("/missing"),
                "privacy wire requested-bidder slot is not an empty string",
            ),
        ] {
            let mut invalid = record();
            *invalid.pointer_mut(path).unwrap() = value;
            assert_eq!(parse(&invalid).unwrap_err(), message, "{path}");
        }
        let mut invalid = record();
        invalid["user"] = json!({"mode": "omit"});
        let error = parse(&invalid).unwrap_err();
        assert!(error.contains("unknown field `user`"), "{error}");
        for key in ["type", "targeting", "meta"] {
            let mut conflict = record();
            conflict["banner_response_prebid"][key] = json!({});
            assert_eq!(
                parse(&conflict).unwrap_err(),
                "privacy wire response facts override shared response fields"
            );
        }
    }
}
