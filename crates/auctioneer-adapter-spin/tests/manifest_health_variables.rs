//! The SDK secret reader needs an application declaration and component mapping.

#[cfg(test)]
mod tests {
    #[test]
    fn health_secret_is_declared_secret_and_mapped_without_an_adapter_policy_flag() {
        let manifest: toml::Value = toml::from_str(include_str!("../spin.toml")).unwrap();
        let variables = manifest["variables"].as_table().unwrap();
        let token = &variables["health_gvl_token"];
        assert_eq!(token["secret"].as_bool(), Some(true));
        assert_eq!(token["default"].as_str(), Some(""));
        assert_eq!(
            manifest["component"]["auctioneer-adapter-spin"]["variables"]["health_gvl_token"]
                .as_str(),
            Some("{{ health_gvl_token }}")
        );
        assert!(!variables.contains_key("allow_unauthenticated_health"));
        assert!(
            manifest["component"]["auctioneer-adapter-spin"]["variables"]
                .get("allow_unauthenticated_health")
                .is_none()
        );
    }

    #[test]
    fn spin_boot_does_not_read_health_policy_or_credentials() {
        let source = include_str!("../src/lib.rs");
        assert!(source.contains("install_health()?"));
        for removed in [
            "health::FLAG_VARIABLE",
            "variables::get(\"health_gvl_token\")",
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
