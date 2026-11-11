// EdgeZero feature/extensible-cli (post-2277072): the wrapper uses
// Spin's `#[http_service]` macro with the `Request` type — its
// expansion contains an `unsafe extern "C"` block (component-model
// entrypoint) that collides with the workspace-wide
// `unsafe_code = "deny"` lint, so we relax it for this file only.
#![cfg_attr(
    target_arch = "wasm32",
    allow(
        unsafe_code,
        reason = "spin's #[http_service] macro generates the unsafe wasm export"
    )
)]

#[cfg(target_arch = "wasm32")]
use crate::{cooperator::SpinDecodeCooperator, delay::SpinDelaySource, timer::SpinTimer};
#[cfg(target_arch = "wasm32")]
use auctioneer_core::{
    App as AuctioneerApp,
    host_primitives::{
        DelegateToDevStrategy, DevRng, SystemClock, bootstrap_adapter_with_envelope,
    },
};
#[cfg(target_arch = "wasm32")]
use edgezero_adapter_spin::{SpinResponse, run_app_with_platform};
#[cfg(target_arch = "wasm32")]
use spin_sdk::http::Request;
#[cfg(target_arch = "wasm32")]
use spin_sdk::{http_service, variables};
#[cfg(target_arch = "wasm32")]
use std::sync::Arc;

// Host-visible: the outbound profile is plain data, the timer is
// generic over `DelaySource`, and the cooperator is a bare executor
// yield with no host dependency at all, so all four compile and are
// testable off the deployment target. Only the `spin_sdk::time::sleep`
// implementation inside `delay` is wasm-gated.
pub mod boot_log;
pub mod cooperator;
pub mod delay;
pub mod envelope;
pub mod limits;
pub mod timer;

#[cfg(any(test, target_arch = "wasm32"))]
mod health;

#[cfg(target_arch = "wasm32")]
#[http_service]
async fn handle(req: Request) -> anyhow::Result<SpinResponse> {
    let _installed =
        boot_log::install_logging(|key| async move { variables::get(key).await.ok() }).await;

    // The timer is a real host primitive now: the wave scheduler awaits
    // it in the acquisition race, and it sleeps on the WASI
    // monotonic-clock pollable that `EdgeZero`'s own Spin adapter waits
    // on. The cooperator yields to the executor rather than to the host
    // — WASI's clock keeps running across guest CPU work, so the
    // deadline recheck already reads a fresh instant and the yield is
    // there to bound poll monopolization.
    //
    // With configured logging installed, the envelope is validated
    // before any host primitive, and this row is OVER ITS
    // CEILING: `install_boot_envelope` warns and installs rather than
    // refusing, so the guest serves. The warning names the required
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
        || Arc::new(SystemClock::new()),
        || Arc::new(DevRng::new()),
        || Arc::new(SpinTimer::new(SpinDelaySource)),
        || Arc::new(SpinDecodeCooperator),
        || Arc::new(DelegateToDevStrategy),
    )
    .map_err(|e| anyhow::anyhow!("auctioneer boot envelope rejected: {e}"))?;
    install_health()?;
    run_app_with_platform::<AuctioneerApp>(req, profile.dispatch_platform).await
}

/// Install the instance's immutable health observation scope.
#[cfg(target_arch = "wasm32")]
fn install_health() -> anyhow::Result<()> {
    health::install()
        .map_err(|err| anyhow::anyhow!("health counter scope install conflicted: {err}"))
}
