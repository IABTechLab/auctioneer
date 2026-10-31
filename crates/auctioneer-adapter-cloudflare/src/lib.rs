#![cfg_attr(target_arch = "wasm32", no_main)]

#[cfg(target_arch = "wasm32")]
use crate::{
    clock::CloudflareClock, cooperator::CloudflareDecodeCooperator, delay::WorkerDelaySource,
    rng::CloudflareRng, timer::CloudflareTimer,
};
#[cfg(target_arch = "wasm32")]
use auctioneer_core::{
    App as AuctioneerApp,
    host_primitives::{DelegateToDevStrategy, bootstrap_adapter_with_envelope},
};
#[cfg(target_arch = "wasm32")]
use edgezero_adapter_cloudflare::run_app;
#[cfg(target_arch = "wasm32")]
use std::sync::Arc;
#[cfg(target_arch = "wasm32")]
use worker::{Context, Env, Error, Request, Response, Result, event};

#[cfg(target_arch = "wasm32")]
mod clock;
#[cfg(target_arch = "wasm32")]
mod rng;

// Host-visible: the outbound profile is plain data and the timer and
// cooperator are generic over `DelaySource`, so all three compile and
// are testable off the deployment target. Only the `worker::Delay`
// implementation inside `delay` is wasm-gated.
pub mod boot_log;
pub mod cooperator;
pub mod delay;
pub mod envelope;
pub mod limits;
pub mod timer;

#[cfg(any(test, target_arch = "wasm32"))]
mod health;

#[cfg(test)]
mod tests {
    #[test]
    fn deployment_enables_config_reader_finalization() {
        let config: toml::Value = toml::from_str(include_str!("../wrangler.toml")).unwrap();
        let flags = config
            .get("compatibility_flags")
            .and_then(toml::Value::as_array);
        assert!(flags.is_some_and(|entries| {
            entries
                .iter()
                .any(|flag| flag.as_str() == Some("enable_weak_ref"))
        }));
    }
}

// EdgeZero feature/extensible-cli (post-2277072): `run_app` loads the
// manifest internally via the `#[app]` proc-macro plumbing, so the
// wrapper just passes the worker request triple.
/// Boot the Worker isolate and dispatch a request through the application.
///
/// # Errors
/// Returns an error for invalid boot limits, conflicting health-policy
/// installation or request/response dispatch. The SDK delivers application
/// assembly failures as typed HTTP error responses through its owned egress path.
/// Health configuration resolution errors degrade to a closed health policy.
#[cfg(target_arch = "wasm32")]
#[event(fetch)]
pub async fn main(req: Request, env: Env, ctx: Context) -> Result<Response> {
    let _installed =
        boot_log::install_logging(|key| env.var(key).ok().map(|value| value.to_string()));
    // Idempotent — only the first fetch in this isolate constructs
    // the clock / rng. Workers require adapter-local shims because
    // std::time::{Instant, SystemTime} + getrandom are unavailable on
    // wasm32-unknown-unknown.
    //
    // The timer and the decode cooperator are both real host
    // primitives now, backed by the same `setTimeout` future: the wave
    // scheduler awaits the timer in its acquisition race, and auctioneer's
    // own decode awaits the cooperator to refresh the isolate's frozen
    // clock before the one recheck it owns.
    //
    // With configured logging installed, the envelope is validated
    // before any host primitive, and this row is OVER ITS
    // CEILING: `install_boot_envelope` warns and installs rather than
    // refusing, so the isolate serves. The warning names the required
    // total, the ceiling, the overshoot and the terms that dominate it,
    // which is the entire safety margin on that path — an operator who
    // reads only that line must still be able to act on it.
    //
    // Every OTHER validation failure is still terminal and installs no
    // gate and no host primitive. A cap below a measured floor above
    // all: that under-reserves memory the code needs on every request,
    // so no amount of traffic tuning makes booting past it safe.
    // Bootstrap and inspection consume the same adapter-owned declarations.
    let profile = envelope::resource_profile();
    bootstrap_adapter_with_envelope(
        profile.boot_envelope(),
        || Arc::new(CloudflareClock::new()),
        || Arc::new(CloudflareRng::new()),
        || Arc::new(CloudflareTimer::new(WorkerDelaySource)),
        || Arc::new(CloudflareDecodeCooperator::new(WorkerDelaySource)),
        || Arc::new(DelegateToDevStrategy),
    )
    .map_err(|e| Error::RustError(format!("auctioneer boot envelope rejected: {e}")))?;
    install_health(&env)?;
    run_app::<AuctioneerApp>(req, env, ctx).await
}

/// Resolve the health-endpoint access policy from the Worker
/// environment and install it. Runs on every fetch; the install is
/// idempotent for identical inputs, and the first fetch freezes the
/// policy `OnceLock` — including the fail-closed `Closed` policy when
/// neither input resolves — so a later fetch cannot flip the endpoint
/// open.
///
/// Invalid health configuration installs `Closed` without stopping
/// `/status` or auctions, matching Fastly. An installation conflict
/// still fails the fetch: a frozen policy must never be ignored.
#[cfg(target_arch = "wasm32")]
fn install_health(env: &Env) -> Result<()> {
    const TOKEN_SECRET: &str = "AUCTIONEER__RUNTIME__HEALTH_GVL_TOKEN";
    // A missing secret / var reads as absent; the decision table
    // fail-closes on (no token, no opt-in).
    let raw_flag = env
        .var(health::FLAG_VARIABLE)
        .ok()
        .map(|value| value.to_string());
    let raw_token = env.secret(TOKEN_SECRET).ok().map(|value| value.to_string());
    health::install(raw_flag.as_deref(), raw_token)
        .map_err(|err| Error::RustError(format!("health policy install conflicted: {err}")))
}
