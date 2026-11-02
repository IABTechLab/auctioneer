//! Spin variable resolution for the configured stderr backend.

use auctioneer_core::host_primitives::install_stderr_boot_logger_with_level;
use edgezero_core::env_config::EnvConfig;
use std::future::Future;

/// Spin-compatible variable mapped to the canonical `EdgeZero` environment key.
pub const LEVEL_VARIABLE: &str = "edgezero_logging_level";

/// Read the declared component variable and install independent boot and
/// ordinary filters before bootstrap. Missing/invalid values use `EdgeZero`'s
/// default; repeated installs leave the original logger owner unchanged.
pub async fn install_logging<F, Fut>(lookup: F) -> bool
where
    F: FnOnce(&'static str) -> Fut,
    Fut: Future<Output = Option<String>>,
{
    let env = EnvConfig::from_vars(
        lookup(LEVEL_VARIABLE)
            .await
            .map(|value| ("EDGEZERO__LOGGING__LEVEL", value)),
    );
    install_stderr_boot_logger_with_level(edgezero_core::resolve_logging_level(&env))
}
