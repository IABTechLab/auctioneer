//! Fresh-process coverage of configured logging through the real boot envelope.
#![cfg(not(target_arch = "wasm32"))]

#[cfg(test)]
mod tests {
    use auctioneer_adapter_spin::{boot_log, envelope};
    use auctioneer_core::host_primitives::{
        DelegateToDevStrategy, DevRng, SystemClock, bootstrap_adapter_with_envelope,
    };
    use auctioneer_runtime::{ImmediateResolveTimer, NoopDecodeCooperator};
    use edgezero_core::env_config::EnvConfig;
    use futures::executor::block_on;
    use log::{Level, LevelFilter};
    use std::env;
    use std::future::ready;
    use std::process::Command;
    use std::sync::Arc;

    const CHILD: &str = "AUCTIONEER__TEST__LOGGING_CHILD";
    const LEVEL_ENV: &str = "edgezero_logging_level";

    #[test]
    fn logging_variable_is_declared_and_mapped() {
        assert_eq!(
            boot_log::LEVEL_VARIABLE,
            "edgezero_logging_level",
            "Spin rejects empty words between underscores; EnvConfig uses a separate mapping"
        );
        let manifest: toml::Value =
            toml::from_str(include_str!("../spin.toml")).expect("valid Spin manifest");
        let declaration = manifest
            .get("variables")
            .and_then(|variables| variables.get(LEVEL_ENV))
            .expect("the logging variable must be declared");
        assert_eq!(
            declaration.get("default").and_then(toml::Value::as_str),
            Some("INFO")
        );
        let mapping = manifest
            .get("component")
            .and_then(|components| components.get("auctioneer-adapter-spin"))
            .and_then(|component| component.get("variables"))
            .and_then(|variables| variables.get(LEVEL_ENV))
            .and_then(toml::Value::as_str);
        assert_eq!(mapping, Some("{{ edgezero_logging_level }}"));
    }

    fn boot() -> bool {
        let installed = block_on(boot_log::install_logging(|key| {
            assert_eq!(key, LEVEL_ENV);
            ready(env::var(key).ok())
        }));
        let limits = envelope::limits();
        let target = envelope::target();
        bootstrap_adapter_with_envelope(
            envelope::envelope(&limits, &target),
            || Arc::new(SystemClock::new()),
            || Arc::new(DevRng::new()),
            || Arc::new(ImmediateResolveTimer),
            || Arc::new(NoopDecodeCooperator),
            || Arc::new(DelegateToDevStrategy),
        )
        .expect("the shipped over-ceiling envelope warns and boots");
        installed
    }

    #[test]
    fn configured_logging_child() {
        if env::var_os(CHILD).is_none() {
            return;
        }
        assert_eq!(log::max_level(), LevelFilter::Off);
        assert!(boot(), "the first boot installs the provider backend");
        let env_config = EnvConfig::from_vars(
            env::var(LEVEL_ENV)
                .ok()
                .map(|value| ("EDGEZERO__LOGGING__LEVEL", value)),
        );
        let facade_level =
            edgezero_core::resolve_logging_level(&env_config).max(edgezero_core::BOOT_LOG_LEVEL);
        assert_eq!(log::max_level(), facade_level);
        assert!(!boot(), "repeated bootstrap must preserve the backend");
        assert_eq!(log::max_level(), facade_level);
        for level in [
            Level::Error,
            Level::Warn,
            Level::Info,
            Level::Debug,
            Level::Trace,
        ] {
            log::log!(target: "auctioneer::request", level, "ordinary {level}");
            log::log!(target: edgezero_core::BOOT_LOG_TARGET, level, "boot {level}");
            // A namespace prefix is not the independent boot target.
            log::log!(target: "edgezero::boot::request", level, "ordinary-prefix {level}");
        }
    }

    #[test]
    fn target_environment_levels_survive_bootstrap() {
        let mut failures = Vec::new();
        for (raw, expected) in [
            (Some("OFF"), LevelFilter::Off),
            (Some("ERROR"), LevelFilter::Error),
            (Some("DEBUG"), LevelFilter::Debug),
            (Some("DeBuG"), LevelFilter::Debug),
            (Some("TRACE"), LevelFilter::Trace),
            (Some("invalid"), LevelFilter::Info),
            (None, LevelFilter::Info),
        ] {
            let mut command = Command::new(env::current_exe().expect("test binary"));
            command.args(["--exact", "tests::configured_logging_child", "--nocapture"]);
            command.env(CHILD, "1").env_remove(LEVEL_ENV);
            if let Some(value) = raw {
                command.env(LEVEL_ENV, value);
            }
            let output = command.output().expect("fresh logging process");
            let logs = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.status.success(), "{raw:?}: {logs}");
            assert!(logs.contains("1 passed"), "the child must execute: {logs}");
            assert_eq!(
                logs.matches("boot envelope over ceiling: requires").count(),
                1,
                "the actual cold-boot warning must land exactly once: {logs}"
            );
            for level in [
                Level::Error,
                Level::Warn,
                Level::Info,
                Level::Debug,
                Level::Trace,
            ] {
                for (marker, admitted) in [
                    ("ordinary", level <= expected),
                    ("ordinary-prefix", level <= expected),
                    ("boot", level <= edgezero_core::BOOT_LOG_LEVEL),
                ] {
                    if logs.contains(&format!(": {marker} {level}")) != admitted {
                        failures.push(format!(
                            "{raw:?}: {marker} {level} must be admitted={admitted}"
                        ));
                    }
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
