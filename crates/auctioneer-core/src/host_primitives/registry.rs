//! Host-primitive install registry. Adapters store their concrete
//! `Clock`, `Rng`, `Timer`, `DecodeCooperator` and `FanoutStrategy`
//! impls into a `OnceLock` at startup; handlers read from these locks.
//!
//! The install call is idempotent — Cloudflare's per-fetch entrypoint
//! calls it on every request; Axum/Spin/Fastly call it once from main.

use crate::host_primitives::strategy::FanoutStrategy;
use auctioneer_runtime::{Clock, DecodeCooperator, Rng, Timer};
use edgezero_core::time::MonotonicClock;
use sha2::{Digest as _, Sha256};
use std::sync::{Arc, OnceLock};

static CLOCK: OnceLock<Arc<dyn Clock + Send + Sync>> = OnceLock::new();
static RNG: OnceLock<Arc<dyn Rng + Send + Sync>> = OnceLock::new();
static TIMER: OnceLock<Arc<dyn Timer + Send + Sync>> = OnceLock::new();
static DECODE_COOPERATOR: OnceLock<Arc<dyn DecodeCooperator + Send + Sync>> = OnceLock::new();
static STRATEGY: OnceLock<Arc<dyn FanoutStrategy + Send + Sync>> = OnceLock::new();

/// Idempotent install — safe to call from inside a per-request
/// handler. The first call constructs and stores the impls; every
/// subsequent call is a cheap no-op.
///
/// Crate-private on purpose: adapters call
/// [`bootstrap_adapter`](crate::host_primitives::bootstrap_adapter),
/// which composes this with the rest of the boot sequence. Keeping the
/// installer unnameable outside auctioneer-core makes "every entrypoint
/// runs the whole sequence" a compile error to violate rather than a
/// convention a reviewer has to catch.
pub(crate) fn install_host_primitives<C, R, T, D, S>(
    make_clock: C,
    make_rng: R,
    make_timer: T,
    make_decode_cooperator: D,
    make_strategy: S,
) where
    C: FnOnce() -> Arc<dyn Clock + Send + Sync>,
    R: FnOnce() -> Arc<dyn Rng + Send + Sync>,
    T: FnOnce() -> Arc<dyn Timer + Send + Sync>,
    D: FnOnce() -> Arc<dyn DecodeCooperator + Send + Sync>,
    S: FnOnce() -> Arc<dyn FanoutStrategy + Send + Sync>,
{
    CLOCK.get_or_init(make_clock);
    RNG.get_or_init(make_rng);
    TIMER.get_or_init(make_timer);
    DECODE_COOPERATOR.get_or_init(make_decode_cooperator);
    STRATEGY.get_or_init(make_strategy);
}

/// Read an installed primitive, or panic with the shared
/// deployment-bug message. Every `*_handle` accessor routes through
/// here so the install contract is stated — and the panic spelled —
/// exactly once.
///
/// # Panics
/// Panics if no adapter has called `bootstrap_adapter`.
#[expect(
    clippy::expect_used,
    reason = "adapter init contract: every adapter calls bootstrap_adapter before run_app"
)]
fn installed<P>(cell: &OnceLock<Arc<P>>) -> Arc<P>
where
    P: ?Sized,
{
    Arc::clone(
        cell.get()
            .expect("host primitives not installed; adapter must call bootstrap_adapter"),
    )
}

/// Retrieve the installed clock.
///
/// # Panics
/// Panics if no adapter has called `bootstrap_adapter` — that is a
/// deployment bug, not a runtime branch.
#[must_use]
pub fn clock_handle() -> Arc<dyn Clock + Send + Sync> {
    installed(&CLOCK)
}

/// Retrieve the installed RNG.
///
/// # Panics
/// Panics if no adapter has called `bootstrap_adapter`.
#[must_use]
pub fn rng_handle() -> Arc<dyn Rng + Send + Sync> {
    installed(&RNG)
}

/// Retrieve the installed timer.
///
/// # Panics
/// Panics if no adapter has called `bootstrap_adapter`. Silently
/// substituting an immediate-resolve stub would be worse than the
/// panic: under a strategy that races timers every sleep resolves on
/// its first poll, so every bidder call times out before the proxy is
/// ever touched — a wrong answer that looks like a real timeout.
#[must_use]
pub fn timer_handle() -> Arc<dyn Timer + Send + Sync> {
    installed(&TIMER)
}

/// Retrieve the installed decode cooperator.
///
/// # Panics
/// Panics if no adapter has called `bootstrap_adapter`. The cell is
/// independent of [`timer_handle`]'s: a target can have a real sleep
/// primitive and no host yield point, or the reverse, so neither one
/// substitutes for the other.
#[must_use]
pub fn decode_cooperator_handle() -> Arc<dyn DecodeCooperator + Send + Sync> {
    installed(&DECODE_COOPERATOR)
}

/// Retrieve the installed fanout strategy.
///
/// # Panics
/// Panics if no adapter has called `bootstrap_adapter` — same
/// deployment-bug reasoning as [`clock_handle`]. The cells are
/// independent: a test may inject a `TestStrategyHandle` without a
/// `TestTimerHandle`, so defaulting either one silently pairs a real
/// strategy with a primitive it was never meant to run against.
#[must_use]
pub fn strategy_handle() -> Arc<dyn FanoutStrategy + Send + Sync> {
    installed(&STRATEGY)
}

/// Direct development calls may lack an envelope, but still need real
/// installed primitives before selecting the per-call fanout bridge.
pub(crate) fn fanout_primitives_installed() -> bool {
    TIMER.get().is_some() && STRATEGY.get().is_some()
}

/// Per-request test override for `Clock`. Handlers check the request
/// extensions for this handle FIRST and fall back to the global
/// `OnceLock` only when absent.
#[derive(Clone)]
pub struct TestClockHandle(pub Arc<dyn Clock + Send + Sync>);

/// Per-request test override for the MONOTONIC clock the auction
/// path reads.
///
/// The production source is `ctx.monotonic_clock()` — `EdgeZero`'s own
/// per-request clock — which at this `EdgeZero` pin has no public
/// setter on `RequestContext`. This handle is the equivalent seam on
/// auctioneer's side: installing it makes one source drive
/// `request_start`, the exchange's deadline composition and the
/// fanout's timing together, which is the whole point of the single
/// clock domain. Absent, the auction path uses
/// `ctx.monotonic_clock()` and never consults `TestClockHandle` for
/// monotonic time.
#[derive(Clone)]
pub struct TestMonotonicClockHandle(pub MonotonicClock);

/// Per-request test override for `Rng`. Same pattern as
/// `TestClockHandle`.
#[derive(Clone)]
pub struct TestRngHandle(pub Arc<dyn Rng + Send + Sync>);

/// Per-request test override for `Timer`. Same pattern as
/// `TestClockHandle`.
#[derive(Clone)]
pub struct TestTimerHandle(pub Arc<dyn Timer + Send + Sync>);

/// Per-request test override for `FanoutStrategy`. Same pattern as
/// `TestClockHandle`.
#[derive(Clone)]
pub struct TestStrategyHandle(pub Arc<dyn FanoutStrategy + Send + Sync>);

/// Authenticator for the `/_auctioneer/health/gvl` bearer token. The
/// token is resolved from the request-bound default Secret Store; portable
/// core never reads provider environment variables.
pub trait HealthAuth: Send + Sync {
    /// Constant-time compare against the configured token.
    /// Implementations MUST NOT short-circuit on first byte mismatch.
    fn verify(&self, presented: &str) -> bool;
    /// Stable identity of the configured credential (the 32-byte digest for
    /// [`BearerToken`]). Never expose it through diagnostics.
    fn identity(&self) -> [u8; 32];
}

/// The adapter's declared counter scope. The rolling
/// unavailable-lookup counter is process-local, and "process" means
/// something different per platform (Cloudflare `"isolate"`, Fastly
/// `"request"`, Spin `"instance"`, Axum `"process"`); the adapter
/// declares the truth so operators can't misread the counter as
/// deployment-wide.
static COUNTER_SCOPE: OnceLock<&'static str> = OnceLock::new();

/// The installed counter scope, `"unknown"` when no adapter installed
/// one.
#[must_use]
pub fn counter_scope_handle() -> &'static str {
    COUNTER_SCOPE.get().copied().unwrap_or("unknown")
}

/// Counter-scope installation failed because another adapter already owns it.
#[derive(Debug, thiserror::Error)]
pub enum HealthBootError {
    #[error("a conflicting health counter scope was already installed")]
    ConflictingScope,
}

/// Install only the adapter's immutable observation scope.
/// Authentication policy is resolved from operator configuration per request.
///
/// # Errors
/// Returns [`HealthBootError::ConflictingScope`] for a different installed scope.
pub fn install_health_counter_scope(counter_scope: &'static str) -> Result<(), HealthBootError> {
    set_counter_scope(counter_scope)
}

/// Repeated identical installs, including lost races, succeed.
fn set_counter_scope(counter_scope: &'static str) -> Result<(), HealthBootError> {
    match COUNTER_SCOPE.get() {
        Some(existing) if *existing == counter_scope => Ok(()),
        Some(_) => Err(HealthBootError::ConflictingScope),
        None => match COUNTER_SCOPE.set(counter_scope) {
            Ok(()) => Ok(()),
            Err(rejected) => match COUNTER_SCOPE.get() {
                Some(winner) if *winner == rejected => Ok(()),
                _ => Err(HealthBootError::ConflictingScope),
            },
        },
    }
}

/// The CONCRETE production authenticator. Constant-time so a timing
/// side-channel can't recover the token byte by byte; hashing both
/// sides to a fixed 32 bytes before the constant-time compare removes
/// length as a side channel too.
pub struct BearerToken {
    digest: [u8; 32],
}

impl BearerToken {
    /// Digest a configured secret, rejecting an EMPTY one. An empty
    /// configured secret is a misconfiguration (a caller sending
    /// `Bearer ` or an empty header would otherwise authenticate). The
    /// health handler maps `None` here to an unavailable credential.
    #[must_use]
    pub fn new<T>(token: T) -> Option<Self>
    where
        T: AsRef<[u8]>,
    {
        if token.as_ref().is_empty() {
            return None;
        }
        Some(Self {
            digest: Sha256::digest(token.as_ref()).into(),
        })
    }
}

impl HealthAuth for BearerToken {
    fn verify(&self, presented: &str) -> bool {
        use subtle::ConstantTimeEq as _;
        let presented_digest: [u8; 32] = Sha256::digest(presented.as_bytes()).into();
        presented_digest.ct_eq(&self.digest).into()
    }

    // The digest IS the stable identity.
    fn identity(&self) -> [u8; 32] {
        self.digest
    }
}

#[cfg(test)]
mod tests {
    use super::{strategy_handle, timer_handle};

    #[test]
    fn health_counter_scope_reinstalls_are_idempotent_and_conflicts_fail() {
        super::install_health_counter_scope("process").unwrap();
        super::install_health_counter_scope("process").unwrap();
        assert!(matches!(
            super::install_health_counter_scope("isolate"),
            Err(super::HealthBootError::ConflictingScope)
        ));
        assert_eq!(super::counter_scope_handle(), "process");
    }

    /// A missing install is a deployment bug and must be loud. The
    /// assertion rests on nothing in this test binary calling
    /// `bootstrap_adapter`: it is an adapter-boot entry point, and a
    /// `OnceLock` written by one test would leak into every other.
    #[test]
    #[should_panic(expected = "host primitives not installed")]
    fn timer_handle_panics_when_no_adapter_installed() {
        drop(timer_handle());
    }

    /// Same contract for the strategy cell. A silent delegate default
    /// here would let a caller that injected only a strategy run it
    /// against primitives it never asked for.
    #[test]
    #[should_panic(expected = "host primitives not installed")]
    fn strategy_handle_panics_when_no_adapter_installed() {
        drop(strategy_handle());
    }
}
