//! Local cookie-key provisioning preserves the existing health secret and alias.

#[cfg(test)]
mod tests {
    use auctioneer_core::App;
    use edgezero_core::app::Hooks;
    use toml::{Value, from_str};

    #[test]
    fn cookie_keyring_is_env_backed_in_the_existing_default_secret_store() {
        let manifest: Value =
            from_str(include_str!("../../../fastly.toml")).expect("the Fastly manifest parses");
        let stores = <App as Hooks>::stores()
            .secrets
            .expect("the default secret store exists");
        assert_eq!(stores.default, "default");
        assert_eq!(stores.ids, &["default"]);
        let local_stores = manifest["local_server"]["secret_stores"]
            .as_table()
            .expect("local secret stores");
        assert_eq!(local_stores.len(), 1_usize, "no new store alias is needed");
        let entries = local_stores[stores.default]
            .as_array()
            .expect("secret entries");
        for (key, environment) in [
            (
                "usersync_cookie_keyring",
                "AUCTIONEER__USERSYNC__COOKIE_KEYRING",
            ),
            ("health_gvl_token", "AUCTIONEER__RUNTIME__HEALTH_GVL_TOKEN"),
        ] {
            let matching = entries
                .iter()
                .filter(|entry| entry["key"].as_str() == Some(key))
                .collect::<Vec<_>>();
            assert_eq!(matching.len(), 1_usize, "{key} must occur exactly once");
            let entry = matching[0].as_table().expect("a secret entry is a table");
            assert_eq!(entry.get("env").and_then(Value::as_str), Some(environment));
            assert_eq!(
                entry.len(),
                2_usize,
                "{key} must have only key/env, never plaintext data or a file source"
            );
        }
    }
}
