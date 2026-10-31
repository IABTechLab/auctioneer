//! Local health provisioning, including optional real SDK reads in Viceroy.

#[cfg(test)]
mod tests {
    use auctioneer_core::App;
    use auctioneer_core::host_primitives::{
        BearerToken, HealthAccessPolicy, HealthBootError, decide_health_policy, read_bool_flag,
    };
    use edgezero_core::app::Hooks;
    #[cfg(not(target_arch = "wasm32"))]
    use std::process::Output;
    use toml::{Value, from_str};

    const FLAG_KEY: &str = "allow_unauthenticated_health";
    const TOKEN: &str = "auctioneer-development-only-health-test-token";
    const FLAG_TOKEN: &str = "flagHeadF8v3.flagBodyJ7m2.flagSigQ5n9";
    const PADDED_FLAG_TOKEN: &str = " \tflagHeadF8v3.flagBodyJ7m2.flagSigQ5n9\n";

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
        let mut results = Vec::new();
        for test in [
            "tests::the_local_manifest_resolves_health_with_and_without_a_development_token",
            "tests::logical_store_aliases_and_fail_closed_faults_execute_in_the_guest",
        ] {
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
            results.push((test, output));
        }
        fs::remove_dir_all(directory).unwrap();
        for (test, output) in results {
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
    }

    fn assert_no_health_credentials(rendered: &str) {
        for sensitive in [
            TOKEN,
            "auctioneer-development",
            "health-test-token",
            FLAG_TOKEN,
            "flagHeadF8v3",
            "flagBodyJ7m2",
            "flagSigQ5n9",
            "F8v3",
            "J7m2",
            "Q5n9",
        ] {
            assert!(
                !rendered.contains(sensitive),
                "health credentials and their segments must not be exposed"
            );
        }
    }

    #[test]
    fn rejected_health_flags_preserve_key_and_type_without_raw_values() {
        for raw_flag in [
            FLAG_TOKEN,
            PADDED_FLAG_TOKEN,
            TOKEN,
            "not-a-boolean",
            "TRUE",
            "",
        ] {
            let error = read_bool_flag(FLAG_KEY, Some(raw_flag))
                .expect_err("a non-boolean flag must be rejected");
            assert!(matches!(
                &error,
                HealthBootError::NonBooleanFlag { key } if *key == FLAG_KEY
            ));
            let display = error.to_string();
            let debug = format!("{error:?}");
            assert!(display.contains("not a strict boolean"));
            assert!(debug.contains("NonBooleanFlag"));
            for rendered in [display, debug] {
                assert!(rendered.contains(FLAG_KEY));
                assert_no_health_credentials(&rendered);
                if !raw_flag.trim().is_empty() {
                    assert!(
                        !rendered.contains(raw_flag.trim()),
                        "neither Display nor Debug may expose the rejected flag"
                    );
                }
            }
        }
    }

    fn manifest() -> Value {
        from_str(include_str!("../../../fastly.toml")).expect("the local Fastly manifest parses")
    }

    fn local_policy(token: Option<&str>) -> HealthAccessPolicy {
        let manifest = manifest();
        let config_id = <App as Hooks>::stores()
            .config
            .expect("config store")
            .default;
        let raw_flag = manifest["local_server"]["config_stores"][config_id]["contents"]
            ["allow_unauthenticated_health"]
            .as_str();
        let allow = read_bool_flag("allow_unauthenticated_health", raw_flag)
            .expect("the local health flag is valid");
        decide_health_policy(token.and_then(BearerToken::new), allow)
            .expect("local health inputs resolve without fallback")
    }

    #[test]
    fn an_absent_or_empty_local_token_keeps_health_closed() {
        for token in [None, Some("")] {
            assert!(matches!(local_policy(token), HealthAccessPolicy::Closed));
        }
    }

    #[test]
    fn a_development_only_local_token_requires_the_matching_bearer() {
        let HealthAccessPolicy::Token(auth) =
            local_policy(Some("auctioneer-development-only-health-test-token"))
        else {
            panic!("a configured local token must select bearer authentication");
        };
        assert!(auth.verify("auctioneer-development-only-health-test-token"));
        assert!(!auth.verify("wrong-development-token"));
        assert!(!auth.verify(""));
    }

    #[test]
    fn the_local_health_flag_is_a_string_in_the_declared_config_store() {
        let manifest = manifest();
        let config_id = <App as Hooks>::stores()
            .config
            .expect("config store")
            .default;
        let config = &manifest["local_server"]["config_stores"][config_id];
        assert_eq!(config["format"].as_str(), Some("inline-toml"));
        let contents = config["contents"].as_table().expect("inline config values");
        let flag = contents
            .get("allow_unauthenticated_health")
            .and_then(Value::as_str);
        assert_eq!(flag, Some("false"));
        assert!(!read_bool_flag("allow_unauthenticated_health", flag).unwrap());
        assert!(
            contents.get(config_id).and_then(Value::as_str).is_some(),
            "health provisioning must preserve the existing runtime config envelope"
        );
        assert!(
            !contents.contains_key("health_gvl_token"),
            "bearer credentials do not belong in the Config Store"
        );
    }

    #[test]
    fn the_local_health_secret_is_env_backed_in_the_declared_secret_store() {
        let manifest = manifest();
        let secrets_id = <App as Hooks>::stores()
            .secrets
            .expect("secret store")
            .default;
        let entries = manifest["local_server"]["secret_stores"][secrets_id]
            .as_array()
            .expect("Viceroy secret stores use arrays of key/value entries");
        let token = entries
            .iter()
            .find(|entry| entry["key"].as_str() == Some("health_gvl_token"))
            .expect("the health token key is materialized even when its env var is unset")
            .as_table()
            .expect("a secret entry is a table");
        assert_eq!(
            token.get("env").and_then(Value::as_str),
            Some("AUCTIONEER__RUNTIME__HEALTH_GVL_TOKEN")
        );
        assert!(
            !token.contains_key("data") && !token.contains_key("file"),
            "Viceroy requires exactly one source; no operator token is checked in"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn assert_guest_diagnostics(
        test: &str,
        case_index: usize,
        raw_flag: Option<&str>,
        output: &Output,
    ) {
        let logs = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.status.success(), "{test}: {logs}");
        assert!(
            logs.contains("1 passed"),
            "the SDK case must execute: {logs}"
        );
        assert_no_health_credentials(&logs);
        if let Some(flag) = raw_flag {
            assert!(
                !logs.contains(flag.trim()),
                "the process logger must not expose the rejected flag"
            );
            let diagnostics = logs
                .lines()
                .filter(|line| {
                    line.contains("health policy unresolved")
                        && (line.contains("[auctioneer]") || line.contains("auctioneer_log :: "))
                })
                .collect::<Vec<_>>();
            assert_eq!(
                diagnostics.len(),
                1,
                "case {case_index}: the real installer must diagnose the flag: {logs}"
            );
            for diagnostic in diagnostics {
                assert!(
                    diagnostic.contains("[auctioneer] ERROR auctioneer::health:")
                        || (diagnostic.contains("auctioneer_log :: ")
                            && diagnostic.contains(" ERROR health policy unresolved")),
                    "the selected sink must carry the ERROR diagnostic: {logs}"
                );
                assert!(
                    diagnostic.contains(FLAG_KEY),
                    "case {case_index}: the flag key must remain diagnosable: {logs}"
                );
                assert!(
                    diagnostic.contains("not a strict boolean"),
                    "case {case_index}: the wrong-type reason must remain diagnosable: {logs}"
                );
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "requires Viceroy and AUCTIONEER__TEST__FASTLY_LOGGING_WASM pointing to prebuilt WASM library tests"]
    fn the_local_manifest_resolves_health_with_and_without_a_development_token() {
        use std::env;
        use std::fs::{remove_file, write};
        use std::path::Path;
        use std::process::{Command, id};

        let wasm =
            env::var_os("AUCTIONEER__TEST__FASTLY_LOGGING_WASM").expect("WASM test artifact path");
        let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fastly.toml");
        for (case_index, (test, token, raw_flag)) in [
            (
                "health_setup_tests::unset_local_token_resolves_closed",
                None,
                None,
            ),
            (
                "health_setup_tests::local_development_token_resolves_authenticated",
                Some(TOKEN),
                None,
            ),
            (
                "health_setup_tests::misplaced_health_flag_is_diagnosed_and_installs_closed",
                None,
                Some(FLAG_TOKEN),
            ),
            (
                "health_setup_tests::misplaced_health_flag_is_diagnosed_and_installs_closed",
                None,
                Some(PADDED_FLAG_TOKEN),
            ),
            (
                "health_setup_tests::misplaced_health_flag_is_diagnosed_and_installs_closed",
                Some(TOKEN),
                Some(FLAG_TOKEN),
            ),
            (
                "health_setup_tests::misplaced_health_flag_is_diagnosed_and_installs_closed",
                Some(TOKEN),
                Some(PADDED_FLAG_TOKEN),
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let fixture_path = raw_flag.map(|flag| {
                let mut fixture = manifest();
                let config_id = <App as Hooks>::stores()
                    .config
                    .expect("config store")
                    .default;
                fixture["local_server"]["config_stores"][config_id]["contents"][FLAG_KEY] =
                    Value::String(flag.to_owned());
                let path = env::temp_dir().join(format!(
                    "auctioneer-fastly-health-{}-{case_index}.toml",
                    id()
                ));
                write(
                    &path,
                    toml::to_string(&fixture).expect("health fixture TOML"),
                )
                .expect("write health fixture");
                path
            });
            let mut command = Command::new("viceroy");
            command
                .arg("run")
                .arg("-C")
                .arg(fixture_path.as_deref().unwrap_or(&manifest_path))
                .arg(&wasm)
                .args(["--exact", test, "--ignored", "--nocapture"])
                .env_remove("AUCTIONEER__RUNTIME__HEALTH_GVL_TOKEN")
                .env_remove("AUCTIONEER__USERSYNC__COOKIE_KEYRING");
            if let Some(value) = token {
                command.env("AUCTIONEER__RUNTIME__HEALTH_GVL_TOKEN", value);
            }
            if cfg!(target_os = "macos") && env::var_os("SSL_CERT_FILE").is_none() {
                command.env("SSL_CERT_FILE", "/etc/ssl/cert.pem");
            }
            let attempt = command.output();
            if let Some(path) = fixture_path {
                remove_file(path).expect("remove health fixture");
            }
            let output = attempt.expect("run Viceroy");
            assert_guest_diagnostics(test, case_index, raw_flag, &output);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "requires Viceroy and AUCTIONEER__TEST__FASTLY_LOGGING_WASM pointing to prebuilt WASM library tests"]
    fn logical_store_aliases_and_fail_closed_faults_execute_in_the_guest() {
        use std::env;
        use std::fs::{remove_file, write};
        use std::process::{Command, id};
        use toml::map::Map;

        let wasm =
            env::var_os("AUCTIONEER__TEST__FASTLY_LOGGING_WASM").expect("WASM test artifact path");
        let stores = <App as Hooks>::stores();
        let config_id = stores.config.expect("config alias").default;
        let secrets_id = stores.secrets.expect("secret alias").default;
        for case in ["legacy-selectors", "missing-config", "missing-secrets"] {
            let mut fixture = manifest();
            let local = fixture["local_server"]
                .as_table_mut()
                .expect("local manifest");
            let test = if case == "legacy-selectors" {
                let mut legacy = Map::new();
                for (kind, logical, physical) in [
                    ("CONFIG", config_id, "auctioneer_runtime_physical"),
                    ("SECRETS", secrets_id, "auctioneer_secrets_physical"),
                ] {
                    for prefix in ["EDGEZERO", "EDGEZERO__SERVICES__0000000000000000000000"] {
                        legacy.insert(
                            format!("{prefix}__STORES__{kind}__{}__NAME", logical.to_uppercase()),
                            Value::String(physical.to_owned()),
                        );
                    }
                }
                let config = local["config_stores"]
                    .as_table_mut()
                    .expect("config stores");
                let mut physical = config[config_id].clone();
                physical["contents"][FLAG_KEY] = Value::String("true".to_owned());
                config.insert("auctioneer_runtime_physical".to_owned(), physical);
                config.insert(
                    "edgezero_runtime_env".to_owned(),
                    Value::Table(Map::from_iter([
                        ("format".to_owned(), Value::String("inline-toml".to_owned())),
                        ("contents".to_owned(), Value::Table(legacy)),
                    ])),
                );
                local["secret_stores"]
                    .as_table_mut()
                    .expect("secret stores")
                    .insert(
                        "auctioneer_secrets_physical".to_owned(),
                        Value::Array(vec![Value::Table(Map::from_iter([
                            (
                                "key".to_owned(),
                                Value::String("health_gvl_token".to_owned()),
                            ),
                            (
                                "data".to_owned(),
                                Value::String("wrong-physical-store-token".to_owned()),
                            ),
                        ]))]),
                    );
                "health_setup_tests::health_and_sdk_reads_use_logical_aliases_despite_legacy_selectors"
            } else {
                let (kind, alias) = if case == "missing-config" {
                    ("config_stores", config_id)
                } else {
                    ("secret_stores", secrets_id)
                };
                local[kind]
                    .as_table_mut()
                    .expect("store table")
                    .remove(alias);
                "health_setup_tests::a_missing_health_store_stays_closed_through_sdk_dispatch"
            };
            let path =
                env::temp_dir().join(format!("auctioneer-fastly-alias-{}-{case}.toml", id()));
            write(&path, toml::to_string(&fixture).expect("fixture TOML")).expect("write fixture");
            let mut command = Command::new("viceroy");
            command
                .arg("run")
                .arg("-C")
                .arg(&path)
                .arg(&wasm)
                .args(["--exact", test, "--ignored", "--nocapture"])
                .env("AUCTIONEER__RUNTIME__HEALTH_GVL_TOKEN", TOKEN)
                .env_remove("AUCTIONEER__USERSYNC__COOKIE_KEYRING");
            if cfg!(target_os = "macos") && env::var_os("SSL_CERT_FILE").is_none() {
                command.env("SSL_CERT_FILE", "/etc/ssl/cert.pem");
            }
            let attempt = command.output();
            remove_file(path).expect("remove fixture");
            let output = attempt.expect("run Viceroy");
            assert_guest_diagnostics(test, 0, None, &output);
            let logs = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(!logs.contains("wrong-physical-store-token"));
            assert_eq!(
                logs.contains("health policy unresolved"),
                case != "legacy-selectors",
                "{case}: {logs}"
            );
        }
    }
}
