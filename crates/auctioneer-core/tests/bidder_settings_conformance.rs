//! Generated provider settings must decode through the real runtime overlay.

#[cfg(test)]
mod tests {
    use auctioneer_core::config::AuctioneerAppConfig;
    use serde_json::json;
    use std::{fs, path::Path};

    #[test]
    fn bidder_overrides_decode_generated_settings_without_exposing_values_in_debug() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../auctioneer-bidder/src/bidders");
        let mut checked = 0_usize;
        for entry in fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path().join("bidder.toml");
            if !path.is_file() {
                continue;
            }
            let manifest: toml::Value = toml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
            let Some(fields) = manifest
                .get("settings")
                .and_then(|settings| settings.get("fields"))
                .and_then(toml::Value::as_table)
            else {
                continue;
            };
            for (field, kind) in fields {
                if kind.as_str() != Some("optional_string") {
                    continue;
                }
                let owner_settings = json!({field:"private-host-marker"});
                let input = json!({
                    "schema_version": 1_u64,
                    "bidders": {
                        "fake_future_alias": {
                            "implementation": manifest["key"].as_str().unwrap(),
                            "settings": owner_settings,
                        },
                    },
                });
                let source = toml::to_string(&input).unwrap();
                let config: AuctioneerAppConfig = toml::from_str(&source).unwrap();
                let json_config: AuctioneerAppConfig = serde_json::from_value(input).unwrap();
                assert!(!format!("{config:?}").contains("private-host-marker"));
                assert_eq!(
                    serde_json::to_value(&config).unwrap(),
                    serde_json::to_value(json_config).unwrap(),
                );
                let serialized = serde_json::to_value(&config).unwrap();
                assert_eq!(
                    serialized["bidders"]["fake_future_alias"]["implementation"],
                    manifest["key"].as_str().unwrap(),
                );
                assert_eq!(
                    serialized["bidders"]["fake_future_alias"]["settings"],
                    owner_settings,
                );
                checked += 1;
            }
        }
        assert!(
            checked > 0,
            "runtime settings redaction coverage must not be empty"
        );
    }

    #[test]
    fn supplied_settings_require_the_bidder_row_owner_and_a_separate_subconfig() {
        let omitted = json!({
            "schema_version": 1_u64,
            "bidders": {"fake_future_alias": {}},
        });
        let omitted_json: AuctioneerAppConfig = serde_json::from_value(omitted.clone()).unwrap();
        let omitted_toml: AuctioneerAppConfig =
            toml::from_str(&toml::to_string(&omitted).unwrap()).unwrap();
        assert!(omitted_json.bidders["fake_future_alias"].settings.is_none());
        assert!(omitted_toml.bidders["fake_future_alias"].settings.is_none());

        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../auctioneer-bidder/src/bidders");
        let mut owners = 0_usize;
        let mut refusals = 0_usize;
        for entry in fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path().join("bidder.toml");
            if !path.is_file() {
                continue;
            }
            let manifest: toml::Value = toml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
            let Some(fields) = manifest
                .get("settings")
                .and_then(|settings| settings.get("fields"))
                .and_then(toml::Value::as_table)
            else {
                continue;
            };
            let owner = manifest["key"].as_str().unwrap();
            let owner_settings: serde_json::Map<_, _> = fields
                .iter()
                .map(|(field, kind)| {
                    let value = match kind.as_str().unwrap() {
                        "optional_string" => json!("fake_value"),
                        "optional_i64" => json!(7_i64),
                        "optional_secret_reference" => json!("fake_secret_bundle"),
                        other => panic!("unsupported declared settings kind: {other}"),
                    };
                    (field.clone(), value)
                })
                .collect();
            let mut nested_owner = owner_settings.clone();
            nested_owner.insert("implementation".to_owned(), json!(owner));
            for row in [
                json!({"settings":{}}),
                json!({"settings":owner_settings}),
                json!({"settings":nested_owner}),
                json!({"implementation":owner, "settings":{"values":owner_settings}}),
                json!({"implementation":owner, "settings":{"fake_unknown_settings_field":{"fake_value":true}}}),
            ] {
                let input = json!({
                    "schema_version": 1_u64,
                    "bidders": {"fake_future_alias": row},
                });
                serde_json::from_value::<AuctioneerAppConfig>(input.clone()).expect_err(
                    "supplied settings require the bidder row owner and owner-only fields",
                );
                toml::from_str::<AuctioneerAppConfig>(&toml::to_string(&input).unwrap())
                    .expect_err("TOML uses the same bidder row owner and settings subconfig");
                refusals += 1;
            }

            let empty = json!({
                "schema_version": 1_u64,
                "bidders": {"fake_future_alias": {"implementation":owner, "settings":{}}},
            });
            let from_json: AuctioneerAppConfig = serde_json::from_value(empty.clone()).unwrap();
            let from_toml: AuctioneerAppConfig =
                toml::from_str(&toml::to_string(&empty).unwrap()).unwrap();
            assert_eq!(
                from_json.bidders["fake_future_alias"]
                    .settings
                    .as_ref()
                    .unwrap()
                    .implementation_key(),
                owner,
            );
            assert_eq!(
                serde_json::to_value(&from_json).unwrap(),
                serde_json::to_value(from_toml).unwrap(),
            );
            assert_eq!(
                serde_json::to_value(from_json).unwrap()["bidders"]["fake_future_alias"],
                empty["bidders"]["fake_future_alias"],
            );
            owners += 1;
        }
        assert!(
            owners > 0,
            "settings owner boundary coverage must not be empty"
        );
        assert_eq!(refusals, owners * 5);
    }
}
