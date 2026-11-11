//! Worker cookie-key provisioning follows the literal SDK secret binding name.

#[cfg(test)]
mod tests {
    use auctioneer_core::App;
    use edgezero_core::app::Hooks;
    use toml::{Value, from_str};

    #[test]
    fn cookie_keyring_uses_the_existing_default_binding_and_no_plaintext_vars() {
        let manifest: Value =
            from_str(include_str!("../wrangler.toml")).expect("the Wrangler manifest parses");
        let stores = <App as Hooks>::stores()
            .secrets
            .expect("the default secret store exists");
        assert_eq!(stores.default, "auctioneer_secrets");
        assert_eq!(stores.ids, &["auctioneer_secrets"]);
        let variables = manifest["vars"].as_table().expect("Worker variables");
        assert_eq!(
            variables["EDGEZERO__STORES__SECRETS__AUCTIONEER_SECRETS__NAME"].as_str(),
            Some("auctioneer_secrets")
        );
        for name in [
            "usersync_cookie_keyring",
            "EDGEZERO__SECRETS__USERSYNC_COOKIE_KEYRING",
            "AUCTIONEER__USERSYNC__COOKIE_KEYRING",
        ] {
            assert!(
                !variables.contains_key(name),
                "{name} must not be a plaintext Worker variable"
            );
        }
    }

    #[test]
    fn provisioning_uses_the_exact_lowercase_worker_secret_name() {
        assert!(
            include_str!("../wrangler.toml")
                .lines()
                .any(|line| { line.trim() == "# wrangler secret put usersync_cookie_keyring" }),
            "CloudflareSecretStore calls Env::secret(key) without a prefix or case conversion"
        );
    }
}
