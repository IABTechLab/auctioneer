use auctioneer_adapter_axum::{envelope, timer::TokioTimer};
#[cfg(test)]
mod limits;
mod resource_profile;

use auctioneer_core::host_primitives::{
    DevRng, SystemClock, axum_production_strategy, bootstrap_adapter_with_envelope,
    install_health_counter_scope,
};
use auctioneer_runtime::NoopDecodeCooperator;
use edgezero_adapter_axum::dev_server::run_app_with_preflight;
use edgezero_core::env_config::EnvConfig;
use edgezero_core::logging::{BOOT_LOG_LEVEL, BOOT_LOG_TARGET, resolve_logging_level};
use std::env;
use std::sync::Arc;

fn main() -> anyhow::Result<()> {
    let mut args = env::args_os().skip(1);
    if let Some(arg) = args.next() {
        if arg != "--resource-profile" || args.next().is_some() {
            anyhow::bail!("unsupported arguments");
        }
        return resource_profile::write()
            .map_err(|_error| anyhow::anyhow!("resource profile unavailable"));
    }
    serve()
}

fn serve() -> anyhow::Result<()> {
    // App owns the backend and both filters before any startup diagnostic.
    simple_logger::SimpleLogger::new()
        .with_level(resolve_logging_level(&EnvConfig::from_env()))
        .with_module_level(BOOT_LOG_TARGET, BOOT_LOG_LEVEL)
        .init()?;
    // The per-isolate memory envelope, validated before any gate or
    // primitive is installed. A rejected envelope installs nothing and
    // returns here, so boot fails naming the bad limit rather than
    // serving with absent gates and host-default byte caps.
    let profile = envelope::resource_profile();

    // The racing strategy needs a real host timer, and tokio's is
    // installed on the same line. `axum_production_strategy` is passed
    // as a function item, not invoked: the installer takes a factory
    // closure so the strategy is built only on the first install.
    let boot_result = bootstrap_adapter_with_envelope(
        profile.boot_envelope(),
        || Arc::new(SystemClock::new()),
        || Arc::new(DevRng::new()),
        || Arc::new(TokioTimer),
        // Nothing to cooperate with: a native host reads a fresh
        // instant on every call, so auctioneer's decode recheck already
        // sees the CPU time it spent. The yield is only load-bearing on
        // a runtime whose clock is frozen for the duration of a turn.
        || Arc::new(NoopDecodeCooperator),
        axum_production_strategy,
    );
    // Boot stops here on a rejected envelope: no gate and no primitive
    // has been installed, so there is nothing to serve with. The error
    // names the limit that failed.
    boot_result.map_err(|error| anyhow::anyhow!("boot envelope rejected: {error}"))?;
    // Policy and secret resolution belongs to the shared health handler.
    install_health_counter_scope("process")?;
    run_app_with_preflight::<auctioneer_core::App, _>(|| {
        envelope::validate_sdk_settings(&EnvConfig::from_env())
    })
}

#[cfg(test)]
mod logging_tests {
    use auctioneer_core::host_primitives::{boot_gates, install_boot_envelope};
    use auctioneer_runtime::{BudgetError, validate_isolate_limits};
    use log::LevelFilter;
    use std::env;
    use std::process::Command;

    #[test]
    fn configured_level_survives_fitting_boot_without_budget_warning() {
        assert_configured_levels(
            "logging_tests::configured_level_survives_fitting_boot_without_budget_warning",
            false,
        );
    }

    #[test]
    fn oversized_budget_warning_survives_configured_error_and_off() {
        assert_configured_levels(
            "logging_tests::oversized_budget_warning_survives_configured_error_and_off",
            true,
        );
    }

    fn install_oversized_test_row() {
        let mut row = super::envelope::limits();
        row.max_concurrent_auctions = 64;
        row.egress_permits = 64;
        let target = super::envelope::target();
        let boot = super::envelope::envelope(&row, &target);
        assert!(matches!(
            validate_isolate_limits(
                &row,
                &target,
                boot.outbound_meta_bytes,
                boot.construction_costs,
                auctioneer_runtime::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::BudgetOverCeiling { .. }),
        ));
        for _ in 0_u32..2 {
            install_boot_envelope(
                &row,
                &target,
                boot.outbound_meta_bytes,
                boot.construction_costs,
                boot.catalog_costs,
            )
            .expect("an oversized test-only row warns once and still boots");
        }
    }

    fn assert_configured_levels(test_name: &str, oversized: bool) {
        const CHILD_LEVEL: &str = "AUCTIONEER__TEST__LOG_LEVEL";
        if let Ok(expected) = env::var(CHILD_LEVEL) {
            let error = super::serve().expect_err("invalid SDK settings stop before serving");
            assert!(error.to_string().contains("max_connections"));
            let gates = boot_gates().expect("the actual shipped row bootstrapped");
            assert!(gates.derived.total_bytes <= gates.limits.isolate_total_bytes);
            if oversized {
                install_oversized_test_row();
            }
            let level = expected.parse::<LevelFilter>().expect("expected log level");
            assert_eq!(log::max_level(), level.max(edgezero_core::BOOT_LOG_LEVEL));
            assert!(log::log_enabled!(target: edgezero_core::BOOT_LOG_TARGET, log::Level::Warn));
            assert!(!log::log_enabled!(target: edgezero_core::BOOT_LOG_TARGET, log::Level::Info));
            for record_level in [
                log::Level::Error,
                log::Level::Warn,
                log::Level::Info,
                log::Level::Debug,
                log::Level::Trace,
            ] {
                assert_eq!(
                    log::log_enabled!(target: "auctioneer::logging_test", record_level),
                    record_level <= level,
                );
                log::log!(target: "auctioneer::logging_test", record_level, "ordinary {record_level}");
            }
            return;
        }

        for (configured, expected) in [
            (Some("debug"), LevelFilter::Debug),
            (Some("trace"), LevelFilter::Trace),
            (Some("error"), LevelFilter::Error),
            (Some("off"), LevelFilter::Off),
            (Some("invalid"), LevelFilter::Info),
            (None, LevelFilter::Info),
        ] {
            let mut command = Command::new(env::current_exe().expect("test executable"));
            command
                .args(["--exact", test_name, "--nocapture"])
                .env(CHILD_LEVEL, expected.to_string())
                .env("EDGEZERO__ADAPTER__INGRESS__MAX_CONNECTIONS", "385")
                .env_remove("EDGEZERO__LOGGING__LEVEL");
            if let Some(level) = configured {
                command.env("EDGEZERO__LOGGING__LEVEL", level);
            }
            let output = command.output().expect("run fresh logging process");
            let stderr = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
            assert!(
                output.status.success(),
                "configured {configured:?}: {stderr}"
            );
            assert_eq!(
                stderr
                    .matches("boot envelope over ceiling: requires")
                    .count(),
                usize::from(oversized),
                "budget warning matches oversized={oversized} at {configured:?}: {stderr}",
            );
            for level in [
                log::Level::Error,
                log::Level::Warn,
                log::Level::Info,
                log::Level::Debug,
                log::Level::Trace,
            ] {
                assert_eq!(
                    stderr.contains(&format!("ordinary {level}")),
                    level <= expected,
                    "ordinary {level} respects {configured:?}: {stderr}",
                );
            }
        }
    }
}
