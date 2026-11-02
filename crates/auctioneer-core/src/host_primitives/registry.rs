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
/// token is resolved BY THE ADAPTER from its own host surface and
/// installed as a typed primitive — portable core never reads
/// `std::env` (empty on Workers anyway).
pub trait HealthAuth: Send + Sync {
    /// Constant-time compare against the configured token.
    /// Implementations MUST NOT short-circuit on first byte mismatch.
    fn verify(&self, presented: &str) -> bool;
    /// Stable identity for the idempotent-reinstall check (the 32-byte
    /// digest for [`BearerToken`]). Compared instead of the `Arc`
    /// pointer so two authenticators built from the same secret are
    /// equal.
    fn identity(&self) -> [u8; 32];
}

/// An EXPLICIT health-endpoint access policy — there is no implicit
/// "open" state. Absence of an installed policy is fail-closed (401).
pub enum HealthAccessPolicy {
    /// Bearer token required (the common production case).
    Token(Arc<dyn HealthAuth>),
    /// Endpoint served WITHOUT auth — installed only when the deploy
    /// set `allow_unauthenticated_health = true` and is relying on an
    /// ingress guard. The explicit variant makes "open" a deliberate,
    /// greppable choice.
    UnauthenticatedAcknowledged,
    /// FROZEN fail-closed. Installed at boot when the adapter resolved
    /// no token and no opt-in, so a LATER request (Cloudflare / Fastly
    /// / Spin install from request entrypoints) cannot flip the
    /// endpoint open — the `OnceLock` is already set, so any
    /// conflicting reinstall is a `ConflictingPolicy` error. `Closed`
    /// authorizes as `Deny`, same as never-set.
    Closed,
}

/// Hand-written because a `dyn` trait object can't derive it; `Token`
/// compares by `identity()`, not the `Arc` pointer, so two
/// authenticators built from the same secret compare equal.
#[expect(
    clippy::missing_trait_methods,
    reason = "PartialEq::ne's provided default is correct; only eq carries the identity comparison"
)]
impl PartialEq for HealthAccessPolicy {
    fn eq(&self, other: &Self) -> bool {
        use HealthAccessPolicy::{Closed, Token, UnauthenticatedAcknowledged};
        match (self, other) {
            (UnauthenticatedAcknowledged, UnauthenticatedAcknowledged) | (Closed, Closed) => true,
            (Token(a), Token(b)) => a.identity() == b.identity(),
            _ => false,
        }
    }
}

static HEALTH_POLICY: OnceLock<HealthAccessPolicy> = OnceLock::new();

/// `None` (never installed) OR `Some(Closed)` → handler fail-closes to
/// 401. The two differ only in mutability: once `Closed` is installed
/// the cell is frozen.
#[must_use]
pub fn health_policy_handle() -> Option<&'static HealthAccessPolicy> {
    HEALTH_POLICY.get()
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

/// Dedicated health-boot error type — the admission stack's boot error
/// belongs to a later milestone and this path must not depend on it.
#[derive(Debug, thiserror::Error)]
pub enum HealthBootError {
    /// A different policy or counter scope was already installed. A
    /// lost `OnceLock::set` race with an IDENTICAL policy is success
    /// (idempotent); only a genuinely different installed value is a
    /// conflict.
    #[error("a conflicting health policy or counter scope was already installed")]
    ConflictingPolicy,
    /// An adapter-resolved health flag was present but not a strict
    /// `"true"` / `"false"`.
    #[error("health config value {key} is not a strict boolean")]
    NonBooleanFlag {
        /// The static adapter-side key. Rejected values may be credentials.
        key: &'static str,
    },
}

/// Strict boolean parse of an adapter-resolved health flag. Absent →
/// `false`. Present-but-not-`true`/`false` → boot error (never a
/// silent `false`).
///
/// # Errors
/// [`HealthBootError::NonBooleanFlag`] when the trimmed value is
/// anything other than `"true"` or `"false"`.
pub fn read_bool_flag(key: &'static str, raw: Option<&str>) -> Result<bool, HealthBootError> {
    match raw.map(str::trim) {
        Some("true") => Ok(true),
        None | Some("false") => Ok(false),
        Some(_) => Err(HealthBootError::NonBooleanFlag { key }),
    }
}

/// The boot decision table every adapter runs over its resolved
/// `(token, allow_unauthenticated_health)` pair. Centralised here so
/// the four adapter boots share one implementation and the adapter
/// test suites can exercise the table directly.
///
/// # Errors
/// [`HealthBootError::ConflictingPolicy`] when BOTH a token and the
/// unauthenticated opt-in were resolved — conflicting intent fails
/// boot rather than guessing.
pub fn decide_health_policy(
    token: Option<BearerToken>,
    allow_unauthenticated: bool,
) -> Result<HealthAccessPolicy, HealthBootError> {
    match (token, allow_unauthenticated) {
        // Conflicting intent — fail boot, don't guess.
        (Some(_), true) => Err(HealthBootError::ConflictingPolicy),
        (Some(tok), false) => Ok(HealthAccessPolicy::Token(Arc::new(tok))),
        (None, true) => {
            log::warn!(
                target: "auctioneer::health",
                "health_gvl endpoint is OPEN - relying on ingress restriction"
            );
            Ok(HealthAccessPolicy::UnauthenticatedAcknowledged)
        }
        // No token, no opt-in → the frozen fail-closed policy;
        // `health_authorize` answers 401 (attributable, not a hidden
        // bad token).
        (None, false) => Ok(HealthAccessPolicy::Closed),
    }
}

/// Install the health access policy + counter scope. Returns a
/// `Result` and does NOT silently discard `OnceLock::set` errors: a
/// second install is idempotent ONLY when the policy is
/// value-identical; a CONFLICTING second policy fails boot.
///
/// Installation is not transactional: the policy is installed before the
/// counter scope. A scope conflict does not roll back that policy. Startup
/// callers must treat a conflict as fatal; this is not a reconfiguration API.
///
/// # Errors
/// [`HealthBootError::ConflictingPolicy`] when a different policy or
/// scope was already installed.
pub fn install_health_primitives(
    policy: HealthAccessPolicy,
    counter_scope: &'static str,
) -> Result<(), HealthBootError> {
    set_policy(policy)?;
    set_counter_scope(counter_scope)
}

/// Race-IDEMPOTENT install. `OnceLock::set` returns the rejected value
/// on a lost race; two THREADS installing the identical policy must
/// BOTH succeed, so on `set` failure the value that actually won is
/// compared against ours. Only a genuinely different installed policy
/// is `ConflictingPolicy`.
fn set_policy(policy: HealthAccessPolicy) -> Result<(), HealthBootError> {
    match HEALTH_POLICY.get() {
        Some(existing) if *existing == policy => Ok(()),
        Some(_) => Err(HealthBootError::ConflictingPolicy),
        None => match HEALTH_POLICY.set(policy) {
            Ok(()) => Ok(()),
            // Lost the race: whoever won is now present. Identical →
            // fine (idempotent); different → conflict. `set` hands the
            // rejected value back; compare against the winner.
            Err(rejected) => match HEALTH_POLICY.get() {
                Some(winner) if *winner == rejected => Ok(()),
                _ => Err(HealthBootError::ConflictingPolicy),
            },
        },
    }
}

/// Fail-loud AND race-idempotent — same pattern as `set_policy`.
fn set_counter_scope(counter_scope: &'static str) -> Result<(), HealthBootError> {
    match COUNTER_SCOPE.get() {
        Some(existing) if *existing == counter_scope => Ok(()),
        Some(_) => Err(HealthBootError::ConflictingPolicy),
        None => match COUNTER_SCOPE.set(counter_scope) {
            Ok(()) => Ok(()),
            Err(rejected) => match COUNTER_SCOPE.get() {
                Some(winner) if *winner == rejected => Ok(()),
                _ => Err(HealthBootError::ConflictingPolicy),
            },
        },
    }
}

/// Install the FROZEN `Closed` policy — the fail-closed default. Used
/// by an adapter that resolved no token and no opt-in. Leaving
/// `HEALTH_POLICY` unset instead would let a later request-entrypoint
/// install open the endpoint; SETTING the cell to `Closed` freezes it,
/// so any later conflicting install fails boot.
///
/// # Errors
/// [`HealthBootError::ConflictingPolicy`] when a different policy or
/// scope was already installed.
pub fn install_health_primitives_without_policy(
    counter_scope: &'static str,
) -> Result<(), HealthBootError> {
    install_health_primitives(HealthAccessPolicy::Closed, counter_scope)
}

/// The handler's authorization decision, computed from the INSTALLED
/// policy. Not installed OR `Closed` → 401 (fail-closed). The handler
/// returns early on `Deny` before touching the counter or building the
/// payload.
#[must_use]
pub fn health_authorize(presented_bearer: Option<&str>) -> HealthAuthz {
    match health_policy_handle() {
        // Fail-closed: a missing install is a misconfiguration, and
        // 401 is the safe reading.
        None | Some(HealthAccessPolicy::Closed) => HealthAuthz::Deny,
        Some(HealthAccessPolicy::UnauthenticatedAcknowledged) => HealthAuthz::Allow,
        Some(HealthAccessPolicy::Token(auth)) => match presented_bearer {
            Some(tok) if auth.verify(tok) => HealthAuthz::Allow,
            // Missing or wrong bearer → 401.
            _ => HealthAuthz::Deny,
        },
    }
}

/// Outcome of [`health_authorize`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HealthAuthz {
    /// Serve the health payload.
    Allow,
    /// Answer 401 with a `WWW-Authenticate: Bearer` challenge.
    Deny,
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
    /// adapter maps `None` here to "no token configured".
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
    use super::{read_bool_flag, strategy_handle, timer_handle};

    #[test]
    fn rejected_health_flags_never_retain_credentials_in_error_formatting() {
        let secret = "misplaced-health-token-never-log-this";
        let key = "allow_unauthenticated_health";
        let error = read_bool_flag(key, Some(secret)).expect_err("a token is not a boolean");
        for rendered in [error.to_string(), format!("{error:?}")] {
            assert!(rendered.contains(key), "the key is actionable diagnostics");
            assert!(
                !rendered.contains(secret),
                "rejected values are confidential"
            );
        }
    }

    #[test]
    fn health_flags_only_accept_trimmed_lowercase_booleans() {
        for (input, expected) in [
            (None, false),
            (Some(" false "), false),
            (Some(" true "), true),
        ] {
            assert_eq!(read_bool_flag("flag", input).unwrap(), expected);
        }
        for invalid in ["TRUE", "False", "yes", "", " "] {
            assert!(matches!(
                read_bool_flag("flag", Some(invalid)).unwrap_err(),
                super::HealthBootError::NonBooleanFlag { key: "flag" }
            ));
        }
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
