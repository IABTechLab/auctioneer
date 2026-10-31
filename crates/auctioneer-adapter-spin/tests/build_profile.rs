//! Spin deployment omits debuginfo without changing native release diagnostics.
//! Executable workflow and artifact gates live in `.github/workflows/ci-spin.yml`.

#[cfg(test)]
mod tests {
    use toml::Value;

    const COMPONENT: &str = "target/wasm32-wasip2/wasm-release/auctioneer_adapter_spin.wasm";

    fn workspace_manifest() -> Value {
        toml::from_str(include_str!("../../../Cargo.toml")).expect("valid workspace manifest")
    }

    fn spin_manifest() -> Value {
        toml::from_str(include_str!("../spin.toml")).expect("valid Spin manifest")
    }

    #[test]
    fn spin_variable_names_have_no_empty_words() {
        let manifest = spin_manifest();
        for variables in [
            &manifest["variables"],
            &manifest["component"]["auctioneer-adapter-spin"]["variables"],
        ] {
            for name in variables.as_table().expect("variable table").keys() {
                assert!(
                    name.split('_').all(|word| !word.is_empty()),
                    "Spin rejects empty words in variable {name:?}"
                );
            }
        }
    }

    #[test]
    fn native_release_preserves_workspace_line_info() {
        let manifest = workspace_manifest();
        let release = &manifest["profile"]["release"];
        assert_eq!(release["debug"].as_integer(), Some(1));
        assert_eq!(release["package"]["*"]["debug"].as_bool(), Some(false));
        assert!(
            release.get("strip").is_none_or(|strip| {
                strip.as_bool() == Some(false) || strip.as_str() == Some("none")
            }),
            "native release must not strip its line info"
        );
    }

    #[test]
    fn wasm_release_disables_and_strips_debug_info() {
        let manifest = workspace_manifest();
        let deploy = manifest["profile"]
            .get("wasm-release")
            .expect("wasm deployment needs a separate profile from native release");
        assert_eq!(deploy["inherits"].as_str(), Some("release"));
        assert_eq!(deploy["debug"].as_bool(), Some(false));
        assert_eq!(deploy["strip"].as_str(), Some("debuginfo"));
    }

    #[test]
    fn spin_build_uses_the_wasm_release_profile_and_component() {
        let manifest = spin_manifest();
        let component = &manifest["component"]["auctioneer-adapter-spin"];
        assert_eq!(
            component["build"]["command"]
                .as_str()
                .expect("Spin build command")
                .split_whitespace()
                .collect::<Vec<_>>(),
            [
                "cargo",
                "build",
                "--target",
                "wasm32-wasip2",
                "--profile",
                "wasm-release",
            ]
        );
        assert_eq!(
            component["source"].as_str(),
            Some(format!("../../{COMPONENT}").as_str())
        );
    }
}
