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
                let input = json!({
                    "schema_version": 1_u64,
                    "bidders": {
                        "fake_future_alias": {
                            "settings": {
                                "implementation": manifest["key"].as_str().unwrap(),
                                "values": {field:"private-host-marker"},
                            },
                        },
                    },
                });
                let source = toml::to_string(&input).unwrap();
                let config: AuctioneerAppConfig = toml::from_str(&source).unwrap();
                assert!(!format!("{config:?}").contains("private-host-marker"));
                assert_eq!(
                    serde_json::to_value(&config).unwrap()["bidders"]["fake_future_alias"]["settings"]
                        ["values"][field],
                    "private-host-marker"
                );
                checked += 1;
            }
        }
        assert!(
            checked > 0,
            "runtime settings redaction coverage must not be empty"
        );
    }
}
