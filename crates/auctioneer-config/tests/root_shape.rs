//! The two consumers of [`AuctioneerConfigSource`] instantiate it with
//! different generic arguments, so nothing but a shared test proves
//! they still agree on the ROOT shape. One fixture document must
//! deserialize under both instantiations — mismatched namespace names,
//! a required field turning optional on one side, or a
//! `deny_unknown_fields` slipping off would show up here first.

#[cfg(test)]
mod tests {
    use auctioneer_config::{AuctioneerConfigSource, OperatorConfigSource};
    use serde::Deserialize;
    use toml::{Table, Value as TomlValue};

    /// A minimal document with both namespaces present. The baked
    /// tables carry a couple of keys each so a `toml::Value`
    /// instantiation has something to hold; the real strict shapes
    /// live in `auctioneer-config-data`'s build script.
    const FIXTURE: &str = r#"
[baked.host]
status_response = ""

[baked.account]
disabled = false

[runtime]
[runtime.auction]
"#;

    /// Stand-in for the build script's `HostDefaultRaw` / `AccountRaw`:
    /// a STRICT subtree type, which is what that side instantiates.
    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct StrictHost {
        status_response: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct StrictAccount {
        disabled: bool,
    }

    /// Stand-in for `AuctioneerAppConfig`: an all-`Option` overlay type,
    /// which is what the CLI side instantiates.
    #[derive(Debug, Default, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct OverlayRuntime {
        #[serde(default)]
        auction: Option<toml::Value>,
    }

    const OPERATOR_FIXTURE: &str = r#"
[startup]
status_response = ""

[runtime]
disabled = false
"#;

    #[test]
    fn operator_consumers_agree_on_the_root_shape() {
        let startup_side: OperatorConfigSource<StrictHost, toml::Value> =
            toml::from_str(OPERATOR_FIXTURE).expect("startup can be typed independently");
        assert_eq!(startup_side.startup.status_response, "");
        assert!(startup_side.runtime.is_table());

        let runtime_side: OperatorConfigSource<toml::Value, StrictAccount> =
            toml::from_str(OPERATOR_FIXTURE).expect("runtime can be typed independently");
        assert!(runtime_side.startup.is_table());
        assert!(!runtime_side.runtime.disabled);
    }

    #[cfg(feature = "config-model")]
    mod typed_operator {
        use auctioneer_config::{
            OperatorConfigSource, policy::OperatorPolicy, startup::StartupConfig,
        };
        use serde::{Deserialize, Serialize};
        use serde_json::{error::Category, json};
        use validator::Validate;

        #[derive(Default, Deserialize, Serialize, Validate)]
        #[serde(deny_unknown_fields)]
        struct FakeSettings {}

        type OperatorDocument = OperatorConfigSource<StartupConfig, OperatorPolicy<FakeSettings>>;

        #[test]
        fn actual_startup_and_policy_models_share_one_strict_document() {
            let document = json!({
                "startup": {"config": {"source": "store", "store": "fake_config", "key": "fake_key", "max_config_fetch_ms": 2_000_u32}},
                "runtime": {"schema_version": 1_u64, "host": {"status_response": "fake_ready"}}
            });
            let text = toml::to_string(&document).unwrap();
            let from_json: OperatorDocument = serde_json::from_value(document).unwrap();
            let from_toml: OperatorDocument = toml::from_str(&text).unwrap();
            for source in [from_json, from_toml] {
                source.startup.validate().unwrap();
                source.runtime.validate().unwrap();
                assert_eq!(
                    serde_json::to_value(source.startup).unwrap(),
                    json!({"config": {"source": "store", "store": "fake_config", "key": "fake_key", "max_config_fetch_ms": 2_000_u32}})
                );
                assert_eq!(
                    source.runtime.host.status_response.as_deref(),
                    Some("fake_ready")
                );
            }
        }

        #[test]
        fn typed_document_never_accepts_an_opaque_startup_or_runtime_subtree() {
            let mut checked = 0_usize;
            for (scope, replacement, reason) in [
                (
                    "startup",
                    json!({"config": {"source": "host", "key": "fake_key"}}),
                    "unknown field `key`",
                ),
                (
                    "startup",
                    json!({"config": {"source": "store", "key": null}}),
                    "invalid type",
                ),
                (
                    "runtime",
                    json!({"host": {}}),
                    "invalid_application_schema_version",
                ),
                (
                    "runtime",
                    json!({"schema_version": 1_u64, "auction": {}}),
                    "unknown field `auction`",
                ),
                (
                    "runtime",
                    json!({"schema_version": 2_u64}),
                    "unsupported_application_schema_version",
                ),
            ] {
                let mut document = json!({"startup": {}, "runtime": {"schema_version": 1_u64}});
                document[scope] = replacement;
                let error = serde_json::from_value::<OperatorDocument>(document).unwrap_err();
                assert_eq!(error.classify(), Category::Data);
                assert!(
                    error.to_string().contains(reason),
                    "{scope}: expected {reason}, got {error}"
                );
                checked += 1;
            }
            assert_eq!(checked, 5);
        }
    }

    #[test]
    fn operator_root_requires_startup() {
        let error =
            toml::from_str::<OperatorConfigSource<toml::Value, OverlayRuntime>>("[runtime]\n")
                .expect_err("startup must not be defaulted");
        assert!(error.to_string().contains("startup"));
    }

    #[test]
    fn operator_root_requires_runtime_even_when_the_subtree_has_defaults() {
        let error =
            toml::from_str::<OperatorConfigSource<toml::Value, OverlayRuntime>>("[startup]\n")
                .expect_err("runtime must not be defaulted");
        assert!(error.to_string().contains("runtime"));
    }

    #[test]
    fn operator_root_requires_sections_even_for_optional_subtree_types() {
        type OptionalDocument = OperatorConfigSource<Option<TomlValue>, Option<TomlValue>>;
        let json_source: OptionalDocument =
            serde_json::from_value(serde_json::json!({"startup": {}, "runtime": {}})).unwrap();
        let toml_source: OptionalDocument = toml::from_str("[startup]\n[runtime]\n").unwrap();
        for source in [json_source, toml_source] {
            assert_eq!(source.startup, Some(TomlValue::Table(Table::new())));
            assert_eq!(source.runtime, Some(TomlValue::Table(Table::new())));
        }
        for (document, missing) in [("[startup]\n", "runtime"), ("[runtime]\n", "startup")] {
            let error = toml::from_str::<
                OperatorConfigSource<Option<toml::Value>, Option<toml::Value>>,
            >(document)
            .expect_err("generic subtree optionality must not weaken root presence");
            assert!(error.to_string().contains(missing));
        }
    }

    #[test]
    fn operator_root_rejects_unknown_namespaces() {
        for unknown in ["runtim", "startpu", "other"] {
            let document = format!("{OPERATOR_FIXTURE}\n[{unknown}]\n");
            let error =
                toml::from_str::<OperatorConfigSource<StrictHost, StrictAccount>>(&document)
                    .expect_err("unknown operator namespaces must fail closed");
            assert!(error.to_string().contains("unknown field"));
            assert!(error.to_string().contains(unknown));
        }
    }

    #[test]
    fn operator_root_rejects_baked_instead_of_aliasing_it() {
        for document in [FIXTURE.to_owned(), format!("{OPERATOR_FIXTURE}\n[baked]\n")] {
            let error = toml::from_str::<OperatorConfigSource<toml::Value, toml::Value>>(&document)
                .expect_err("the new root must never accept the legacy baked namespace");
            assert!(error.to_string().contains("unknown field"));
            assert!(error.to_string().contains("baked"));
        }
    }

    #[test]
    fn operator_root_preserves_strict_subtree_parsing() {
        for document in [
            OPERATOR_FIXTURE.replace("status_response = \"\"", "status_response = false"),
            OPERATOR_FIXTURE.replace("disabled = false", "disabled = 0"),
        ] {
            let error =
                toml::from_str::<OperatorConfigSource<StrictHost, StrictAccount>>(&document)
                    .unwrap_err();
            assert!(error.to_string().contains("invalid type"));
        }
    }

    #[test]
    fn operator_root_and_both_sections_require_objects() {
        use serde_json::{error::Category, json};

        type Document = OperatorConfigSource<toml::Value, OverlayRuntime>;
        let valid = json!({"startup": {}, "runtime": {}});
        serde_json::from_value::<Document>(valid).unwrap();
        for document in [
            json!([{}, {}]),
            json!({"startup": [], "runtime": {}}),
            json!({"startup": {}, "runtime": []}),
        ] {
            let error = serde_json::from_value::<Document>(document).unwrap_err();
            assert_eq!(error.classify(), Category::Data);
            assert!(error.to_string().contains("invalid type"));
        }
    }

    #[test]
    fn unknown_subtree_keys_are_rejected_with_valid_required_fields() {
        for document in [
            OPERATOR_FIXTURE.replace(
                "status_response = \"\"",
                "status_response = \"\"\nunexpected = true",
            ),
            OPERATOR_FIXTURE.replace("disabled = false", "disabled = false\nunexpected = true"),
        ] {
            let error =
                toml::from_str::<OperatorConfigSource<StrictHost, StrictAccount>>(&document)
                    .expect_err("valid required fields cannot mask an unknown sibling");
            assert!(error.to_string().contains("unknown field"));
            assert!(error.to_string().contains("unexpected"));
        }
    }

    #[test]
    fn build_and_cli_agree_on_the_root_shape() {
        // The build script's instantiation: baked strict, runtime opaque.
        let build_side: AuctioneerConfigSource<StrictHost, StrictAccount, toml::Value> =
            toml::from_str(FIXTURE).expect("the build-side instantiation must accept the document");
        assert_eq!(build_side.baked.host.status_response, "");
        assert!(!build_side.baked.account.disabled);
        assert!(
            build_side.runtime.is_table(),
            "the build side needs `[runtime]` only to prove it exists",
        );

        // The CLI's instantiation: baked opaque, runtime typed.
        let cli_side: AuctioneerConfigSource<toml::Value, toml::Value, OverlayRuntime> =
            toml::from_str(FIXTURE).expect("the CLI-side instantiation must accept the document");
        assert!(
            cli_side.baked.host.is_table() && cli_side.baked.account.is_table(),
            "the CLI carries the baked tables as opaque values it never re-validates",
        );
        assert!(
            cli_side.runtime.auction.is_some(),
            "the CLI side is the one that types `[runtime]`",
        );
    }

    #[test]
    fn root_rejects_an_unknown_namespace() {
        // A `[runtim]` typo must not sail past as an unread table.
        let typo = FIXTURE.replace("[runtime]", "[runtim]\n[runtime]");
        let err =
            toml::from_str::<AuctioneerConfigSource<StrictHost, StrictAccount, toml::Value>>(&typo)
                .expect_err("an unknown root namespace must be rejected");
        assert!(
            err.to_string().contains("unknown field") && err.to_string().contains("runtim"),
            "the error should name the unknown root key. Got: {err}",
        );
    }

    #[test]
    fn baked_namespace_rejects_an_unknown_table() {
        let stray = format!("{FIXTURE}\n[baked.other]\nx = 1\n");
        let err = toml::from_str::<AuctioneerConfigSource<StrictHost, StrictAccount, toml::Value>>(
            &stray,
        )
        .expect_err("an unknown `[baked.*]` table must be rejected");
        assert!(
            err.to_string().contains("unknown field") && err.to_string().contains("other"),
            "the error should name the unknown baked namespace. Got: {err}",
        );
    }

    #[test]
    fn both_namespaces_are_required() {
        let no_runtime = FIXTURE.replace("[runtime]\n[runtime.auction]\n", "");
        let err = toml::from_str::<AuctioneerConfigSource<StrictHost, StrictAccount, toml::Value>>(
            &no_runtime,
        )
        .expect_err("`[runtime]` is required, not defaulted");
        assert!(
            err.to_string().contains("runtime"),
            "the error should name the missing `runtime` field. Got: {err}",
        );

        let no_baked = "[runtime]\n";
        let baked_err = toml::from_str::<
            AuctioneerConfigSource<StrictHost, StrictAccount, toml::Value>,
        >(no_baked)
        .expect_err("`[baked]` is required, not defaulted");
        assert!(
            baked_err.to_string().contains("baked"),
            "the error should name the missing `baked` field. Got: {baked_err}",
        );
    }
}
