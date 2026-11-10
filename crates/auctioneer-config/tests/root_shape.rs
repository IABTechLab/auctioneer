//! The authoring document is the same direct versioned payload used at runtime.

#[cfg(test)]
mod tests {
    use auctioneer_config::policy::OperatorPolicy;
    use serde::{Deserialize, Serialize};
    use serde_json::{Value, error::Category, json};
    use validator::Validate;

    #[derive(Debug, Deserialize, Serialize, Validate)]
    #[serde(deny_unknown_fields)]
    struct Settings {
        implementation: String,
        #[validate(nested)]
        settings: Fields,
    }

    #[derive(Debug, Deserialize, Serialize, Validate)]
    #[serde(deny_unknown_fields)]
    struct Fields {}

    type Document = OperatorPolicy<Settings>;

    fn fixture() -> Value {
        json!({
            "schema_version": 1_u64,
            "host": {
                "status_response": "",
                "resource_limits": {"default_tmax_ms": 500_u64},
                "gvl": {
                    "authenticate_health": true,
                    "authenticate_health_token_secret": "health_gvl_token"
                }
            },
            "account_defaults": {"disabled": false, "targeting_prefix": "hb"},
            "accounts": {"fake_publisher": {"debug_allow": true}},
            "bidders": {"fake_bidder": {"implementation": "fake", "settings": {}, "enabled": false}},
            "features": {"gzip_outbound": false},
            "floor_currency": {"expires_at": 2000_i64, "usd_per_unit": {"EUR": 1.25_f64}}
        })
    }

    #[test]
    fn json_toml_and_authoring_use_the_same_direct_runtime_payload() {
        let input = fixture();
        let json_policy: Document = serde_json::from_value(input.clone()).unwrap();
        let authoring = toml::to_string(&input).unwrap();
        let toml_policy: Document = toml::from_str(&authoring).unwrap();
        json_policy.validate().unwrap();
        toml_policy.validate().unwrap();
        let canonical = serde_json::to_value(&json_policy).unwrap();
        assert_eq!(serde_json::to_value(&toml_policy).unwrap(), canonical);
        assert_eq!(canonical["schema_version"], json!(1_u64));
        assert_eq!(canonical["host"]["status_response"], json!(""));
        assert_eq!(canonical["account_defaults"]["disabled"], json!(false));
        assert_eq!(
            canonical["accounts"]["fake_publisher"]["debug_allow"],
            json!(true)
        );
        assert_eq!(
            canonical["bidders"]["fake_bidder"]["implementation"],
            json!("fake")
        );
        assert_eq!(
            canonical["bidders"]["fake_bidder"]["settings"],
            input["bidders"]["fake_bidder"]["settings"]
        );
        assert_eq!(canonical["floor_currency"], input["floor_currency"]);
        assert_eq!(canonical["host"]["gvl"], input["host"]["gvl"]);
        assert_eq!(canonical["host"]["gvl"]["authenticate_health"], json!(true));
        assert_eq!(
            canonical["host"]["gvl"]["authenticate_health_token_secret"],
            json!("health_gvl_token")
        );
        assert!(canonical.get("gvl").is_none());
        let restored: Document = toml::from_str(&toml::to_string(&toml_policy).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(restored).unwrap(), canonical);
    }

    #[test]
    fn retired_wrappers_and_unknown_namespaces_are_not_wire_aliases() {
        for name in ["startup", "baked", "runtime", "runtim", "gvl"] {
            let mut document = fixture();
            document
                .as_object_mut()
                .unwrap()
                .insert(name.to_owned(), json!({}));
            let json_error = serde_json::from_value::<Document>(document.clone()).unwrap_err();
            assert_eq!(json_error.classify(), Category::Data);
            assert!(
                json_error
                    .to_string()
                    .contains(&format!("unknown field `{name}`"))
            );
            let toml_error =
                toml::from_str::<Document>(&toml::to_string(&document).unwrap()).unwrap_err();
            assert!(
                toml_error
                    .to_string()
                    .contains(&format!("unknown field `{name}`"))
            );
        }
        for document in [
            json!({"startup": {}, "runtime": fixture()}),
            json!({"baked": {"host": {}, "account": {}}, "runtime": fixture()}),
            json!({"runtime": fixture()}),
        ] {
            serde_json::from_value::<Document>(document.clone())
                .expect_err("retired wrappers must be rejected");
            toml::from_str::<Document>(&toml::to_string(&document).unwrap())
                .expect_err("retired wrappers must be rejected");
        }
    }

    #[test]
    fn schema_version_is_required_and_only_version_one_is_supported() {
        let mut missing = fixture();
        assert!(
            missing
                .as_object_mut()
                .unwrap()
                .remove("schema_version")
                .is_some()
        );
        let missing_error = serde_json::from_value::<Document>(missing.clone()).unwrap_err();
        assert_eq!(missing_error.classify(), Category::Data);
        assert!(
            missing_error
                .to_string()
                .contains("application_schema_version")
        );
        toml::from_str::<Document>(&toml::to_string(&missing).unwrap())
            .expect_err("schema_version is required");
        for version in [
            json!(0_u64),
            json!(2_u64),
            json!(-1_i64),
            json!(1.0_f64),
            json!("1"),
            json!(true),
            json!(null),
        ] {
            let mut document = fixture();
            document["schema_version"] = version.clone();
            let version_error = serde_json::from_value::<Document>(document.clone()).unwrap_err();
            assert_eq!(version_error.classify(), Category::Data);
            assert!(
                version_error
                    .to_string()
                    .contains("application_schema_version")
            );
            if !version.is_null() {
                toml::from_str::<Document>(&toml::to_string(&document).unwrap())
                    .expect_err("unsupported schema versions must be rejected");
            }
        }
    }

    #[test]
    fn root_host_and_account_sections_are_objects_not_positional_arrays() {
        let root_error =
            serde_json::from_value::<Document>(json!([1_u64, {}, {}, {}, {}, {}])).unwrap_err();
        assert_eq!(root_error.classify(), Category::Data);
        for path in [
            "/host",
            "/account_defaults",
            "/accounts/fake_publisher",
            "/host/gvl",
        ] {
            for value in [
                json!([]),
                json!(null),
                json!(true),
                json!(1_u64),
                json!("fake"),
            ] {
                let mut document = fixture();
                *document.pointer_mut(path).unwrap() = value.clone();
                let section_error =
                    serde_json::from_value::<Document>(document.clone()).unwrap_err();
                assert_eq!(section_error.classify(), Category::Data, "{path}");
                if !value.is_null() {
                    toml::from_str::<Document>(&toml::to_string(&document).unwrap())
                        .expect_err(path);
                }
            }
        }
    }

    #[test]
    fn unknown_fields_are_rejected_inside_typed_sections() {
        for path in [
            "/host",
            "/host/resource_limits",
            "/account_defaults",
            "/accounts/fake_publisher",
            "/bidders/fake_bidder",
            "/bidders/fake_bidder/settings",
            "/features",
            "/floor_currency",
            "/host/gvl",
        ] {
            let mut document = fixture();
            document
                .pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("unexpected".to_owned(), json!(true));
            let json_error = serde_json::from_value::<Document>(document.clone()).unwrap_err();
            assert!(
                json_error
                    .to_string()
                    .contains("unknown field `unexpected`"),
                "{path}"
            );
            let toml_error =
                toml::from_str::<Document>(&toml::to_string(&document).unwrap()).unwrap_err();
            assert!(
                toml_error
                    .to_string()
                    .contains("unknown field `unexpected`"),
                "{path}"
            );
        }
    }
}
