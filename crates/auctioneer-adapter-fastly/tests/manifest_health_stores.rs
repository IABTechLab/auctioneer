//! Local provisioning and real shared health requests through SDK-backed guest stores.

#[cfg(test)]
mod tests {
    use auctioneer_core::App;
    use edgezero_core::app::Hooks;
    #[cfg(not(target_arch = "wasm32"))]
    use edgezero_core::blob_envelope::BlobEnvelope;
    #[cfg(not(target_arch = "wasm32"))]
    use serde_json::json;
    use toml::{Value, from_str};

    #[cfg(not(target_arch = "wasm32"))]
    const TOKEN: &str = "auctioneer-development-only-health-test-token";

    #[cfg(all(unix, not(target_arch = "wasm32")))]
    #[test]
    fn health_fixture_children_do_not_inherit_cookie_signing_keys() {
        use std::env;
        use std::fs;
        use std::os::unix::fs::PermissionsExt as _;
        use std::process::{self, Command};

        let directory =
            env::temp_dir().join(format!("auctioneer-fastly-health-env-{}", process::id()));
        fs::create_dir_all(&directory).unwrap();
        let probe = directory.join("viceroy");
        // Fail deliberately after observing each real fixture child's environment.
        fs::write(
            &probe,
            r#"#!/bin/sh
if [ "${AUCTIONEER__TEST__FIXTURE_ENV_CONTROL-}" != "synthetic-control" ]; then
  echo 'fixture-child-env: control missing'
elif [ "${AUCTIONEER__USERSYNC__COOKIE_KEYRING+x}" = x ]; then
  echo 'fixture-child-env: keyring inherited'
else
  echo 'fixture-child-env: keyring absent; control retained'
fi
exit 71
"#,
        )
        .unwrap();
        fs::set_permissions(&probe, fs::Permissions::from_mode(0o755)).unwrap();
        let test = "tests::runtime_health_policy_and_provider_faults_execute_in_the_guest";
        let output = Command::new(env::current_exe().unwrap())
            .args(["--exact", test, "--ignored", "--nocapture"])
            .env_clear()
            .env("PATH", &directory)
            .env(
                "AUCTIONEER__TEST__FASTLY_LOGGING_WASM",
                "synthetic-unused-guest.wasm",
            )
            .env(
                "AUCTIONEER__USERSYNC__COOKIE_KEYRING",
                "synthetic-keyring-sentinel",
            )
            .env("AUCTIONEER__TEST__FIXTURE_ENV_CONTROL", "synthetic-control")
            .output()
            .unwrap();
        fs::remove_dir_all(directory).unwrap();
        let logs = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.status.success(), "the probe must stop {test}");
        assert!(
            logs.contains("fixture-child-env: keyring absent; control retained"),
            "{test}: the health child must remove inherited signing keys: {logs}"
        );
        assert!(
            !logs.contains("fixture-child-env: keyring inherited"),
            "{logs}"
        );
    }

    fn manifest() -> Value {
        from_str(include_str!("../../../fastly.toml")).expect("local Fastly manifest parses")
    }

    #[test]
    fn local_config_is_empty_until_push_and_has_no_provider_health_authority() {
        let manifest = manifest();
        assert_eq!(manifest["service_id"].as_str(), Some(""));
        let config_id = <App as Hooks>::stores().config.unwrap().default;
        let config = &manifest["local_server"]["config_stores"][config_id];
        assert_eq!(config["format"].as_str(), Some("inline-toml"));
        let contents = config["contents"].as_table().unwrap();
        assert_eq!(contents.get(config_id).and_then(Value::as_str), Some(""));
        assert!(!contents.contains_key("allow_unauthenticated_health"));
        assert!(!contents.contains_key("health_gvl_token"));
    }

    #[test]
    fn local_health_secret_is_env_backed_in_the_declared_default_store() {
        let manifest = manifest();
        let secrets_id = <App as Hooks>::stores().secrets.unwrap().default;
        let entries = manifest["local_server"]["secret_stores"][secrets_id]
            .as_array()
            .unwrap();
        let token = entries
            .iter()
            .find(|entry| entry["key"].as_str() == Some("health_gvl_token"))
            .expect("local health provisioning placeholder")
            .as_table()
            .unwrap();
        assert_eq!(
            token.get("env").and_then(Value::as_str),
            Some("AUCTIONEER__RUNTIME__HEALTH_GVL_TOKEN")
        );
        assert!(!token.contains_key("data") && !token.contains_key("file"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn runtime_fixture(case: &str) -> (Value, &'static str) {
        let stores = <App as Hooks>::stores();
        let config_id = stores.config.unwrap().default;
        let secrets_id = stores.secrets.unwrap().default;
        let mut fixture = manifest();
        let mut data = json!({"schema_version": 1_u8, "host": {}});
        if case != "omitted" {
            data["host"]["gvl"] = json!({
                "authenticate_health": case != "anonymous",
                "authenticate_health_token_secret": if case == "renamed-entry" { "renamed_health_token" } else { "health_gvl_token" },
            });
            if case == "anonymous" {
                assert!(
                    data["host"]["gvl"]
                        .as_object_mut()
                        .unwrap()
                        .remove("authenticate_health_token_secret")
                        .is_some()
                );
            }
            if case == "malformed" {
                data["host"]["gvl"]["authenticate_health"] = json!("true");
            }
        }
        let blob = BlobEnvelope::new(data, "1970-01-01T00:00:00Z".to_owned());
        fixture["local_server"]["config_stores"][config_id]["contents"][config_id] =
            Value::String(serde_json::to_string(&blob).unwrap());
        if case == "missing-config" {
            assert!(
                fixture["local_server"]["config_stores"]
                    .as_table_mut()
                    .unwrap()
                    .remove(config_id)
                    .is_some()
            );
        }
        if case == "missing-secret-store" {
            assert!(
                fixture["local_server"]["secret_stores"]
                    .as_table_mut()
                    .unwrap()
                    .remove(secrets_id)
                    .is_some()
            );
        }
        if case == "renamed-entry" {
            let entries = fixture["local_server"]["secret_stores"][secrets_id]
                .as_array_mut()
                .unwrap();
            let token = entries
                .iter_mut()
                .find(|entry| entry["key"].as_str() == Some("health_gvl_token"))
                .unwrap();
            token["key"] = Value::String("renamed_health_token".to_owned());
        }
        let test = match case {
            "token" | "renamed-entry" => {
                "health_setup_tests::runtime_token_policy_uses_sdk_bindings"
            }
            "anonymous" => "health_setup_tests::runtime_anonymous_policy_is_explicit",
            "wrong-bearer" => "health_setup_tests::runtime_token_policy_denies_wrong_bearer",
            "absent-bearer" => "health_setup_tests::runtime_token_policy_denies_absent_bearer",
            _ => "health_setup_tests::runtime_faults_close_only_health",
        };
        (fixture, test)
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "requires Viceroy and AUCTIONEER__TEST__FASTLY_LOGGING_WASM pointing to prebuilt WASM library tests"]
    fn runtime_health_policy_and_provider_faults_execute_in_the_guest() {
        use std::{
            env, fs,
            process::{Command, id},
        };
        let wasm = env::var_os("AUCTIONEER__TEST__FASTLY_LOGGING_WASM").expect("WASM test path");
        for case in [
            "token",
            "wrong-bearer",
            "absent-bearer",
            "renamed-entry",
            "anonymous",
            "omitted",
            "malformed",
            "missing-config",
            "missing-secret-store",
            "empty-secret",
            "oversized-secret",
        ] {
            let (fixture, test) = runtime_fixture(case);
            let path = env::temp_dir().join(format!(
                "auctioneer-fastly-runtime-health-{}-{case}.toml",
                id()
            ));
            fs::write(&path, toml::to_string(&fixture).unwrap()).unwrap();
            let mut command = Command::new("viceroy");
            command
                .arg("run")
                .arg("-C")
                .arg(&path)
                .arg(&wasm)
                .args(["--exact", test, "--ignored", "--nocapture"])
                .env_remove("AUCTIONEER__RUNTIME__HEALTH_GVL_TOKEN")
                .env_remove("AUCTIONEER__USERSYNC__COOKIE_KEYRING");
            let token = match case {
                "empty-secret" => String::new(),
                "oversized-secret" => "x".repeat(4097),
                _ => TOKEN.to_owned(),
            };
            command.env("AUCTIONEER__RUNTIME__HEALTH_GVL_TOKEN", token);
            if cfg!(target_os = "macos") && env::var_os("SSL_CERT_FILE").is_none() {
                command.env("SSL_CERT_FILE", "/etc/ssl/cert.pem");
            }
            let attempt = command.output();
            fs::remove_file(path).unwrap();
            let output = attempt.expect("run Viceroy");
            let logs = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.status.success(), "{case}: {logs}");
            assert!(logs.contains("1 passed"), "guest case must execute: {logs}");
            assert!(!logs.contains(TOKEN), "credentials must not appear in logs");
            assert!(
                !logs.contains("health policy unresolved"),
                "boot has no provider health policy: {logs}"
            );
        }
    }
}
