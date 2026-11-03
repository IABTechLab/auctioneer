//! The single adapter boot sequence.
//!
//! Every adapter entrypoint calls [`bootstrap_adapter`] rather than
//! installing host primitives itself, so a new entrypoint cannot
//! silently skip a boot step. That is compile-enforced rather than a
//! convention: `install_host_primitives` is crate-private, so an
//! adapter crate cannot name it at all.
//!
//! # Boot ordering
//!
//! [`bootstrap_adapter_with_envelope`] fixes the order an adapter must
//! observe, so no request can reach a gate that does not exist yet, and
//! no boot diagnostic is emitted into a process with nowhere to put it:
//!
//! 1. The target's `log` sink is installed, so step 2 has somewhere to
//!    report to.
//! 2. The boot envelope is VALIDATED. A rejected envelope installs
//!    nothing and returns an error; an over-ceiling one warns.
//! 3. The concurrency gates it sizes are installed, all together.
//! 4. The host primitives are installed and the baked GCL is lowered.
//! 5. Only after all of that returns `Ok` may the adapter hand its
//!    router to the host.
//!
//! Step 5 is the adapter's obligation and cannot be enforced from here
//! — an adapter physically calls `run_app` on its own line — but steps 1
//! to 4 are ordered inside one function, and the gate accessors answer
//! `None` until step 3 completes. A request arriving before then finds
//! no gate and is refused, rather than admitted ungated.
//!
//! Step 1 preceding step 2 is why the sink is a PARAMETER rather than
//! something an adapter installs on its own line before calling in.
//! The envelope's over-ceiling diagnostic replaced a boot refusal, so
//! it is the only safety signal on that path, and it fires once per
//! process behind a `OnceLock`: a cold start that emits it into no sink
//! suppresses it permanently. Making it a parameter means an adapter
//! cannot forget the sink and silently lose that signal — which is what
//! every target did before, since the pinned `EdgeZero` installs its
//! logger inside `run_app`, after this returns, and its Cloudflare and
//! Spin initialisers are no-ops regardless.

use crate::host_primitives::boot_gates;
use crate::host_primitives::boot_log::install_boot_logger;
use crate::host_primitives::gates::install_boot_envelope;
use crate::host_primitives::registry::install_host_primitives;
use crate::host_primitives::strategy::FanoutStrategy;
use auctioneer_bidder::schema::initialize_native_schemas;
use auctioneer_privacy::gcl::Gcl;
use auctioneer_runtime::{
    BudgetError, Clock, DecodeCooperator, RequestConstructionCosts, ResolvedIsolateLimits, Rng,
    TargetPlatformConstants, Timer,
};
use std::sync::Arc;

/// The inputs that size an isolate's gates, and the sink the validation
/// reports to.
///
/// The resource inputs are bundled because they are validated together and
/// are meaningless apart: a limit set is only checkable against the
/// platform constants it will run on, and the outbound metadata
/// reservation is charged out of the same envelope.
///
/// `log_sink` rides along rather than being a separate argument because
/// it is the same shape of fact — something only the TARGET knows — and
/// because carrying it here is what makes it impossible to validate an
/// envelope without first saying where the verdict goes. See the module
/// docs for why that matters more than it looks.
#[derive(Clone, Copy)]
pub struct BootEnvelope<'a> {
    /// The per-isolate budget this deployment resolved.
    pub limits: &'a ResolvedIsolateLimits,
    /// What the target platform itself fixes.
    pub target: &'a TargetPlatformConstants,
    /// Bytes reserved for outbound request metadata.
    pub outbound_meta_bytes: u32,
    /// Actual exchange/core construction ownership and reusable scratch costs.
    pub construction_costs: RequestConstructionCosts,
    /// Where this target's boot diagnostics go.
    ///
    /// `auctioneer-core` links no platform SDK, so it cannot write to a
    /// Worker console or a Fastly endpoint itself; the adapter that can
    /// supplies the sink. Every target whose platform surfaces process
    /// stderr passes
    /// [`StderrBootLog`](crate::host_primitives::boot_log::StderrBootLog).
    ///
    /// Installed best-effort: `log` allows one global logger, and
    /// losing that race to a test harness or to a host that installs
    /// its own is expected and is not a boot failure. What the boot
    /// sequence guarantees is that the attempt happens BEFORE
    /// validation, not that this particular sink wins.
    pub log_sink: &'static (dyn log::Log + 'static),
    /// Catalog increments over the matched boot baseline and auction phases.
    pub catalog_costs: auctioneer_runtime::CatalogResourceCosts,
}

/// Install the host primitives, then lower the baked GCL into its
/// process static.
///
/// Idempotent, so it is safe to call from a per-request entrypoint:
/// both steps are `OnceLock`-backed, and Axum's call from `main` and
/// the `EdgeZero` targets' call from their fetch path behave
/// identically. Lowering the GCL here rather than on the first TCF
/// request keeps its parse cost and its heap inside the boot baseline
/// instead of charging them to a request.
///
/// This installs no concurrency gates. An adapter that bounds its load
/// calls [`bootstrap_adapter_with_envelope`] instead, which runs this
/// sequence after the envelope and its gates are in place.
pub fn bootstrap_adapter<C, R, T, D, S>(
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
    install_host_primitives(
        make_clock,
        make_rng,
        make_timer,
        make_decode_cooperator,
        make_strategy,
    );
    Gcl::baked();
}

/// Validate the boot envelope, install the gates it sizes, then run the
/// rest of the boot sequence.
///
/// The envelope is validated and its gates installed BEFORE the host
/// primitives, and the whole sequence completes before the caller is
/// permitted to serve routes. Idempotent for the same reason
/// [`bootstrap_adapter`] is: every step is `OnceLock`-backed, so a
/// per-request entrypoint may call it on each request and concurrent
/// callers install exactly one set of gates between them.
///
/// An envelope whose worst-case total exceeds the platform ceiling
/// WARNS and boots. Its caps state what production traffic is expected
/// to need, and tuning them requires separate workload evidence;
/// the warning carries the overshoot and the dominating terms, and
/// is the whole safety margin on that path.
///
/// # Errors
/// Returns the envelope's own [`BudgetError`] when validation fails for
/// any reason other than the ceiling — a cap under a measured floor
/// above all. On that path NO gates are installed and the host
/// primitives are left alone, so the adapter must fail boot rather than
/// serve: a running isolate with no gates refuses every request, and
/// failing loudly names the bad limit instead of looking like an
/// outage.
pub fn bootstrap_adapter_with_envelope<C, R, T, D, S>(
    envelope: BootEnvelope<'_>,
    make_clock: C,
    make_rng: R,
    make_timer: T,
    make_decode_cooperator: D,
    make_strategy: S,
) -> Result<(), BudgetError>
where
    C: FnOnce() -> Arc<dyn Clock + Send + Sync>,
    R: FnOnce() -> Arc<dyn Rng + Send + Sync>,
    T: FnOnce() -> Arc<dyn Timer + Send + Sync>,
    D: FnOnce() -> Arc<dyn DecodeCooperator + Send + Sync>,
    S: FnOnce() -> Arc<dyn FanoutStrategy + Send + Sync>,
{
    // The sink BEFORE the validation that reports into it. The
    // over-ceiling warning fires once per process and is the only
    // safety signal on the warn-and-boot path, so emitting it into a
    // process with no sink suppresses it for the isolate's whole life.
    // The return value is deliberately unused: a lost race means
    // something else is already listening, which is the outcome this
    // call wants.
    let _sink_is_ours = install_boot_logger(|| envelope.log_sink);
    // Envelope next, and a fatal failure is terminal: `?` returns
    // before a single gate or primitive is installed, so an
    // inconsistent envelope cannot leave the isolate half-booted. An
    // over-ceiling envelope is warned about inside and boots on.
    install_boot_envelope(
        envelope.limits,
        envelope.target,
        envelope.outbound_meta_bytes,
        envelope.construction_costs,
        envelope.catalog_costs,
    )?;
    // Only the installed envelope can authorize initialization. A cached
    // failure leaves new native flows closed while bounded liveness survives.
    if let Some(gates) = boot_gates().filter(|gates| gates.native_schema_scratch_bytes() > 0) {
        let _initialized = initialize_native_schemas(gates.catalog_costs);
    }
    bootstrap_adapter(
        make_clock,
        make_rng,
        make_timer,
        make_decode_cooperator,
        make_strategy,
    );
    Ok(())
}
