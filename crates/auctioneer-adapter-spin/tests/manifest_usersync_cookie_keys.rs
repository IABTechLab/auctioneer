//! The shipped Spin manifest provisions cookie keys without baking credentials.

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        process::{self, Command, Output},
    };
    use toml::{Value, from_str};

    const KEY: &str = "usersync_cookie_keyring";

    fn manifest() -> Value {
        from_str(include_str!("../spin.toml")).expect("the Spin manifest parses")
    }

    #[test]
    fn cookie_keyring_is_an_optional_secret_with_an_unusable_empty_default() {
        let manifest = manifest();
        let declaration = manifest["variables"]
            .get(KEY)
            .and_then(Value::as_table)
            .expect("the cookie keyring must be declared as an application variable");
        assert_eq!(
            declaration.get("required").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            declaration.get("secret").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            declaration.get("default").and_then(Value::as_str),
            Some(""),
            "Spin requires a default for an optional variable; empty is not a signing key"
        );
    }

    #[test]
    fn cookie_keyring_is_mapped_to_its_literal_lowercase_variable() {
        let manifest = manifest();
        assert_eq!(
            manifest["component"]["auctioneer-adapter-spin"]["variables"]
                .get(KEY)
                .and_then(Value::as_str),
            Some("{{ usersync_cookie_keyring }}"),
            "declaring the variable alone does not make the SDK lookup resolve"
        );
    }

    fn spin_load(manifest: &Value, supplied: bool, case: &str) -> Output {
        let directory =
            env::temp_dir().join(format!("auctioneer-spin-manifest-{}-{case}", process::id()));
        fs::create_dir_all(&directory).unwrap();
        assert!(
            !directory
                .join("missing-component.wasm")
                .try_exists()
                .unwrap()
        );
        let mut staged = manifest.clone();
        // Stop after real manifest loading, without a build, guest, or listening server.
        staged["component"]["auctioneer-adapter-spin"]["source"] =
            Value::String("missing-component.wasm".to_owned());
        fs::write(
            directory.join("spin.toml"),
            toml::to_string(&staged).unwrap(),
        )
        .unwrap();
        let mut command = Command::new("spin");
        command.current_dir(&directory).args([
            "up",
            "--from",
            "spin.toml",
            "--state-dir",
            "state",
            "--temp",
            "assets",
            "--cache-dir",
            "cache",
            "--log-dir",
            "logs",
            "--disable-cache",
        ]);
        for (name, _) in env::vars_os() {
            if name.to_string_lossy().starts_with("SPIN_") || name == "RUNTIME_CONFIG_FILE" {
                command.env_remove(name);
            }
        }
        if supplied {
            // Synthetic placeholders, not usable credentials or operator values.
            command.args([
                "--variable",
                "health_gvl_token=synthetic-health-token",
                "--variable",
                "usersync_cookie_keyring=synthetic-cookie-keyring",
            ]);
        }
        let output = command.output();
        fs::remove_dir_all(&directory).unwrap();
        output.expect("Spin 4.1.0 must be installed; the vendor CI job drives this test")
    }

    #[test]
    #[ignore = "requires Spin 4.1.0; ci-spin.yml explicitly drives this vendor-loader test"]
    fn spin_cli_accepts_optional_secrets_and_rejects_missing_defaults() {
        let manifest = manifest();
        for supplied in [false, true] {
            let output = spin_load(&manifest, supplied, &format!("valid-{supplied}"));
            let error = String::from_utf8_lossy(&output.stderr);
            assert_eq!(output.status.code(), Some(1_i32));
            assert!(
                error.contains("failed to read component source from disk"),
                "{error}"
            );
            assert!(error.contains("missing-component.wasm"), "{error}");
            for key in ["health_gvl_token", KEY] {
                let mut invalid = manifest.clone();
                assert!(
                    invalid["variables"][key]
                        .as_table_mut()
                        .unwrap()
                        .remove("default")
                        .is_some()
                );
                let refused = spin_load(&invalid, supplied, &format!("invalid-{key}-{supplied}"));
                let diagnostic = String::from_utf8_lossy(&refused.stderr);
                assert_eq!(refused.status.code(), Some(1_i32));
                assert!(
                    diagnostic.contains("must be `required` OR have a `default`"),
                    "{diagnostic}"
                );
                assert!(
                    !diagnostic.contains("failed to read component source from disk"),
                    "{diagnostic}"
                );
            }
        }
    }
}
