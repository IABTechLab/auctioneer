//! Real Fastly logger regressions, driven in fresh Viceroy processes.
//! Build the WASM library tests first and pass their artifact path in
//! `AUCTIONEER__TEST__FASTLY_LOGGING_WASM`, then run this test with `--ignored`.
//! Levels come from test-only Hooks, not runtime fixture overrides.

#![cfg(not(target_arch = "wasm32"))]

#[cfg(test)]
mod tests {
    use log::{Level, LevelFilter};
    use std::env;
    use std::path::Path;
    use std::process::{self, Command};

    #[cfg(unix)]
    #[test]
    fn logging_fixture_child_does_not_inherit_cookie_signing_keys() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt as _;

        let directory =
            env::temp_dir().join(format!("auctioneer-fastly-logging-env-{}", process::id()));
        fs::create_dir_all(&directory).unwrap();
        let probe = directory.join("viceroy");
        // Fail deliberately after observing the real fixture child's environment.
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
        let output = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::baked_logging_dispatches_without_runtime_environment_or_legacy_overrides",
                "--ignored",
                "--nocapture",
            ])
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
        assert!(
            !output.status.success(),
            "the probe must stop the guest fixture"
        );
        assert!(
            logs.contains("fixture-child-env: keyring absent; control retained"),
            "the logging child must remove inherited signing keys: {logs}"
        );
        assert!(
            !logs.contains("fixture-child-env: keyring inherited"),
            "{logs}"
        );
    }

    fn run_guest(fixture_name: &str, test: &str) -> String {
        let wasm =
            env::var_os("AUCTIONEER__TEST__FASTLY_LOGGING_WASM").expect("WASM test artifact path");
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture_name);
        let mut command = Command::new("viceroy");
        command.arg("run").arg("-C").arg(fixture).arg(wasm).args([
            "--exact",
            test,
            "--ignored",
            "--nocapture",
        ]);
        command.env_remove("AUCTIONEER__USERSYNC__COOKIE_KEYRING");
        if cfg!(target_os = "macos") && env::var_os("SSL_CERT_FILE").is_none() {
            command.env("SSL_CERT_FILE", "/etc/ssl/cert.pem");
        }
        let output = command.output().expect("run Viceroy");
        let logs = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.status.success(), "{test}: {logs}");
        assert!(
            logs.contains("1 passed"),
            "the WASM case must execute: {logs}"
        );
        assert!(
            !logs.contains("edgezero_runtime_env"),
            "boot must not read or diagnose the obsolete runtime environment: {logs}"
        );
        logs
    }

    #[test]
    #[ignore = "requires Viceroy and AUCTIONEER__TEST__FASTLY_LOGGING_WASM pointing to prebuilt WASM library tests"]
    fn baked_logging_dispatches_without_runtime_environment_or_legacy_overrides() {
        for (fixture, test) in [
            (
                "boot-no-runtime-env.toml",
                "logging_tests::baked_logging_dispatches_without_runtime_environment",
            ),
            (
                "boot-legacy-runtime-env.toml",
                "logging_tests::legacy_runtime_logging_settings_cannot_override_baked_hooks",
            ),
        ] {
            let logs = run_guest(fixture, test);
            assert!(
                logs.contains("production baked logging reached sink"),
                "{logs}"
            );
            assert!(
                logs.contains("boot envelope over ceiling: requires"),
                "{logs}"
            );
            assert!(logs.contains("health policy unresolved"), "{logs}");
        }
    }

    #[test]
    #[ignore = "requires Viceroy and AUCTIONEER__TEST__FASTLY_LOGGING_WASM pointing to prebuilt WASM library tests"]
    fn boot_warnings_and_dispatch_filters_survive_restrictive_baked_levels() {
        for (case, configured) in [
            ("error", LevelFilter::Error),
            ("off", LevelFilter::Off),
            ("debug", LevelFilter::Debug),
        ] {
            for mode in [
                "endpoint_boot",
                "endpoint_dispatch",
                "stderr_dispatch",
                "rejected_endpoint_dispatch",
            ] {
                let test = format!("logging_tests::{mode}_{case}");
                let logs = run_guest("boot-no-runtime-env.toml", &test);
                let endpoint = mode.starts_with("endpoint");
                assert_eq!(
                    logs.contains("[auctioneer]"),
                    !endpoint,
                    "named endpoint rejection or absence must select stderr: {logs}"
                );
                let sink_logs = logs
                    .lines()
                    .filter(|line| {
                        if endpoint {
                            line.contains("auctioneer-test :: ")
                        } else {
                            line.contains("[auctioneer]")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(!sink_logs.is_empty(), "the selected sink must emit: {logs}");
                if mode == "endpoint_boot" {
                    assert_eq!(
                        sink_logs
                            .matches("mandatory boot warning reached endpoint")
                            .count(),
                        1,
                        "{logs}"
                    );
                } else {
                    // The health stores are absent. OFF must suppress their ERROR
                    // before the SDK receives the request lifecycle.
                    assert_eq!(
                        logs.contains("health policy unresolved"),
                        configured >= LevelFilter::Error,
                        "health setup respects {configured}: {logs}"
                    );
                    assert_eq!(
                        sink_logs
                            .matches("boot envelope over ceiling: requires")
                            .count(),
                        1,
                        "{logs}"
                    );
                    for level in [
                        Level::Error,
                        Level::Warn,
                        Level::Info,
                        Level::Debug,
                        Level::Trace,
                    ] {
                        assert_eq!(
                            logs.contains(&format!("ordinary {level}")),
                            level <= configured,
                            "ordinary {level} respects {configured}: {logs}"
                        );
                    }
                }
            }
        }
    }
}
