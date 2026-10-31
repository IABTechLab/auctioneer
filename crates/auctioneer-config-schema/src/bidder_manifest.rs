//! Shared build-time and native-tooling representation of bidder-owned provenance.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub key: String,
    pub factory: bool,
    pub required_feature: Option<String>,
    /// Restricts public definitions, not the owning executable candidate factory.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub native_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<EndpointSchema>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<SettingsSchema>,
    #[serde(default)]
    pub definitions: BTreeMap<String, toml::Table>,
    pub provenance: Provenance,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointSchema {
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsSchema {
    #[serde(default)]
    pub fields: BTreeMap<String, SettingKind>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingKind {
    OptionalString,
    OptionalSecretReference,
    OptionalI64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "origin", rename_all = "snake_case", deny_unknown_fields)]
pub enum Provenance {
    #[expect(
        clippy::empty_enum_variants_with_brackets,
        reason = "Serde needs a struct variant to reject unexpected local-provenance fields"
    )]
    Local {},
    Reference {
        sources: BTreeMap<String, Source>,
        #[serde(default)]
        trees: Vec<SourceTree>,
        #[serde(default)]
        artifacts: Vec<Artifact>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub repository: String,
    pub revision: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Implementation,
    Helper,
    Schema,
    Metadata,
    Protocol,
    Parameter,
    License,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceTree {
    pub category: Category,
    pub source: String,
    pub source_path: String,
    pub tree: String,
    #[serde(default)]
    pub port_differences: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub category: Category,
    pub source: String,
    pub source_path: String,
    pub source_blob: String,
    pub source_bytes: u64,
    pub source_sha256: String,
    pub retained: Option<Retained>,
    pub transformation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub https_default: Option<HttpsEndpointDefault>,
    #[serde(default)]
    pub port_differences: Vec<String>,
}

/// Native transport-selection provenance, not a validated or callable endpoint.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HttpsEndpointDefault {
    pub source_url: String,
    pub selected_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inherited_from: Option<String>,
}

/// A local artifact copy; its source path is relative to the owning bidder folder.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Retained {
    #[serde(rename = "source_path")]
    pub path: String,
    #[serde(rename = "source_bytes")]
    pub bytes: u64,
    #[serde(rename = "source_sha256")]
    pub sha256: String,
}

#[cfg(test)]
mod tests {
    use super::{HttpsEndpointDefault, Retained, SettingKind, SettingsSchema};

    const WITNESS: &str = "source_path = 'parameters.json'\nsource_bytes = 3778\nsource_sha256 = '20a3f193c62b1b3d6d2a0daf5dfaa3935a045c52b83af236693ebe9c3459106d'\n";

    #[test]
    fn https_default_records_preserve_exact_templates_and_optional_parent() {
        for inherited in [None, Some("fake_parent".to_owned())] {
            let expected = HttpsEndpointDefault {
                source_url: "http://Bid.Example.test:80/a%2fb?account={{.AccountID}}".to_owned(),
                selected_url: "https://Bid.Example.test:80/a%2fb?account={{.AccountID}}".to_owned(),
                inherited_from: inherited,
            };
            let encoded = toml::to_string(&expected).unwrap();
            let document: toml::Value = toml::from_str(&encoded).unwrap();
            assert_eq!(
                document["source_url"].as_str(),
                Some(expected.source_url.as_str())
            );
            assert_eq!(
                document["selected_url"].as_str(),
                Some(expected.selected_url.as_str())
            );
            assert_eq!(
                document.get("inherited_from").and_then(toml::Value::as_str),
                expected.inherited_from.as_deref()
            );
            assert_eq!(
                toml::from_str::<HttpsEndpointDefault>(&encoded).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn https_default_records_have_a_closed_required_field_shape() {
        let mut valid = toml::Table::new();
        valid.insert(
            "source_url".to_owned(),
            "http://bid.example.test/rtb".into(),
        );
        valid.insert(
            "selected_url".to_owned(),
            "https://bid.example.test/rtb".into(),
        );
        for field in ["source_url", "selected_url"] {
            let mut missing = valid.clone();
            missing.remove(field).unwrap();
            toml::Value::Table(missing)
                .try_into::<HttpsEndpointDefault>()
                .unwrap_err();
        }
        for field in ["approved", "callable", "fallback_url", "typo"] {
            let mut extra = valid.clone();
            extra.insert(field.to_owned(), true.into());
            toml::Value::Table(extra)
                .try_into::<HttpsEndpointDefault>()
                .unwrap_err();
        }
        toml::Value::Table(valid)
            .try_into::<HttpsEndpointDefault>()
            .unwrap();
    }

    #[test]
    fn secret_reference_is_a_closed_setting_kind_that_round_trips() {
        let source = "[fields]\nfake_reference = 'optional_secret_reference'\n";
        let schema: SettingsSchema = toml::from_str(source).unwrap();
        assert!(matches!(
            schema.fields["fake_reference"],
            SettingKind::OptionalSecretReference
        ));
        let encoded = toml::to_string(&schema).unwrap();
        assert_eq!(
            toml::from_str::<toml::Value>(&encoded).unwrap(),
            toml::from_str::<toml::Value>(source).unwrap(),
        );
        for kind in [
            "'optional_secret'",
            "'optional_secret_reference_128'",
            "{ kind = 'optional_secret_reference', max_bytes = 128 }",
        ] {
            let invalid = format!("[fields]\nfake_reference = {kind}\n");
            toml::from_str::<SettingsSchema>(&invalid).unwrap_err();
        }
    }

    #[test]
    fn retained_witness_round_trips_only_source_prefixed_fields() {
        let witness: Retained = toml::from_str(WITNESS).unwrap();
        let encoded = toml::to_string(&witness).unwrap();
        assert_eq!(
            toml::from_str::<toml::Value>(&encoded).unwrap(),
            toml::from_str::<toml::Value>(WITNESS).unwrap(),
        );
    }

    #[test]
    fn retained_witness_rejects_every_legacy_and_mixed_field_shape() {
        let fields = [
            ("source_path", "path"),
            ("source_bytes", "bytes"),
            ("source_sha256", "sha256"),
        ];
        for mask in 1..8_u8 {
            let mut document = WITNESS.to_owned();
            for (bit, (current, legacy)) in fields.iter().enumerate() {
                if mask & (1 << bit) != 0 {
                    document = document.replacen(current, legacy, 1);
                }
            }
            assert!(
                toml::from_str::<Retained>(&document).is_err(),
                "mask {mask}"
            );
        }
        for (_, legacy) in fields {
            let mixed = format!("{WITNESS}{legacy} = 'obsolete'\n");
            assert!(toml::from_str::<Retained>(&mixed).is_err(), "{legacy}");
        }
    }
}
