//! Regressions on the deployed Worker artifact, not just parser defaults.

#[cfg(test)]
mod tests {
    use auctioneer_core::App;
    use edgezero_core::app::Hooks;
    use toml::{Table, Value, from_str};

    const TOKEN_SECRET: &str = "health_gvl_token";

    fn variables() -> Table {
        let manifest: Value = from_str(include_str!("../wrangler.toml"))
            .expect("the shipped Wrangler manifest is valid TOML");
        manifest
            .get("vars")
            .and_then(Value::as_table)
            .expect("the Worker must declare its non-secret runtime variables")
            .clone()
    }

    #[test]
    fn every_worker_runtime_variable_is_declared() {
        let variables = variables();
        for name in [
            "EDGEZERO__ADAPTER__HOST",
            "EDGEZERO__ADAPTER__PORT",
            "EDGEZERO__LOGGING__LEVEL",
        ] {
            assert!(
                variables.get(name).and_then(Value::as_str).is_some(),
                "the Worker reads {name} from Env, not the process environment"
            );
        }
        // Store keys are derived from the same baked declarations the
        // pinned EdgeZero Worker reader uses, so new store ids need wiring.
        let stores = <App as Hooks>::stores();
        for (kind, metadata) in [
            ("CONFIG", stores.config),
            ("KV", stores.kv),
            ("SECRETS", stores.secrets),
        ] {
            if let Some(store_metadata) = metadata {
                for id in store_metadata.ids {
                    let prefix = format!("EDGEZERO__STORES__{kind}__{}", id.to_ascii_uppercase());
                    for suffix in if kind == "CONFIG" {
                        &["NAME", "KEY"][..]
                    } else {
                        &["NAME"][..]
                    } {
                        let key = format!("{prefix}__{suffix}");
                        assert_eq!(
                            variables.get(&key).and_then(Value::as_str),
                            Some(*id),
                            "{key} must preserve the runtime's logical-id default"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn logging_and_health_have_safe_defaults_and_the_token_is_a_secret() {
        let variables = variables();
        assert_eq!(variables["EDGEZERO__LOGGING__LEVEL"].as_str(), Some("INFO"));
        assert!(!variables.contains_key("AUCTIONEER__RUNTIME__ALLOW_UNAUTHENTICATED_HEALTH"));
        assert!(
            !variables.contains_key(TOKEN_SECRET),
            "a credential must never be shipped as a plaintext [vars] default"
        );
        assert!(
            include_str!("../wrangler.toml").contains("wrangler secret put health_gvl_token"),
            "the secret binding must be documented without embedding a token"
        );
    }
    #[test]
    fn worker_boot_does_not_resolve_health_policy_or_credentials() {
        let source = include_str!("../src/lib.rs");
        assert!(source.contains("install_health()?"));
        for removed in [
            "env.secret(",
            "health::FLAG_VARIABLE",
            "raw_token",
            "raw_flag",
        ] {
            assert!(
                !source.contains(removed),
                "provider boot must not resolve {removed}"
            );
        }
    }
}
