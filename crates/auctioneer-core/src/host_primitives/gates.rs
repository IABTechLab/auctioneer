//! The boot envelope and the concurrency gates it sizes.
//!
//! # Why this is process-global and not per-`App`
//!
//! The gates bound CONCURRENCY, so every request in the isolate must
//! contend for the same counters. `App` is the wrong home for them: the
//! `app!` macro's `configure` callback runs on every `App` build, which
//! on a serverless adapter can be once per request, so gates installed
//! there would hand each request its own private capacity and bound
//! nothing at all. They live in `OnceLock`s here instead, installed by
//! [`install_boot_envelope`] from the one boot sequence, and
//! `configure` READS what is installed rather than installing it.
//!
//! # Boot ordering
//!
//! The envelope is VALIDATED before any gate exists, and the gates are
//! installed before routes are served:
//!
//! 1. [`install_boot_envelope`] validates the envelope. A rejected
//!    envelope installs NOTHING and returns an error, so boot fails
//!    loudly rather than serving with absent or half-sized gates.
//! 2. On success it installs all three gates together.
//! 3. Only then does the adapter hand its router to the host.
//!
//! # The one refusal that warns instead
//!
//! A worst-case total over the platform ceiling
//! ([`BudgetError::BudgetOverCeiling`]) warns and boots. It says the
//! declared caps PERMIT a state that would not fit — an ambition about
//! load, not a contradiction — and the caps state what production
//! traffic is expected to need rather than what keeps a sum under a
//! ceiling. Tuning them against real traffic requires separate workload evidence,
//! and blocking every adapter on a worst-case model before any traffic
//! exists buys nothing.
//!
//! Every other variant stays fatal, and the two that matter most are
//! [`BudgetError::LimitBelowMinimum`] and
//! [`BudgetError::FinalResponseBelowAssemblyFloor`]: those are caps
//! under a MEASURED floor, so they under-reserve memory the code needs
//! on every request rather than in some worst case. No amount of
//! traffic tuning makes them safe, and booting past one means
//! overrunning at the first request that exercises it.
//!
//! Step 1 preceding step 2 is what makes "no request can reach a gate
//! that does not exist yet" true: the accessors
//! ([`egress_gate`](egress_gate) and friends) return `None` until the
//! install completes, and a request that finds `None` is refused rather
//! than admitted ungated. Reading an absent gate is therefore a
//! fail-CLOSED branch, which is why these return `Option` instead of
//! panicking the way the host-primitive accessors do: a gate is a load
//! bound, and the safe answer to "is there capacity?" before boot
//! finishes is "no".

use crate::configure::extraction_limits_for;
use auctioneer_bidder::schema::{SchemaArtifactState, native_schema_funded, schema_artifact_state};
use auctioneer_runtime::aux_work::{AuxWorkBudget, AuxWorkDerived};
use auctioneer_runtime::budget::{
    derive_aux_work_for_profile, derive_isolate_budget_with_baked_config,
};
use auctioneer_runtime::{
    AdmissionGate, AuxGate, BudgetError, CatalogResourceCosts, Derived, EgressGate, HeadroomClass,
    RequestConstructionCosts, ResolvedIsolateLimits, StaticEgressGate, StreamGate,
    TargetPlatformConstants, minimum_headroom_bytes, serialized_json_bytes,
};
use std::cmp::Reverse;
use std::num::NonZeroU64;
use std::sync::{Arc, OnceLock};

/// Everything the boot envelope sizes, installed as one unit.
///
/// One struct behind one `OnceLock` rather than a lock per gate: a
/// partial install — some gates present, others absent — is not a state
/// any caller should have to reason about, and separate locks would make
/// it representable.
#[derive(Debug)]
pub struct BootGates {
    /// Auction admission capacity (`max_concurrent_auctions`).
    pub admission: Arc<AdmissionGate>,
    /// Concurrent outbound bidder legs one auction's fanout wave may
    /// decode at once (`max_active_body_streams`).
    ///
    /// The field name is misleading and the limit's name is too: this
    /// gate counts no inbound request bodies. `outbound::wave`'s
    /// `drive_fanout` is its only acquirer, and it takes a whole wave
    /// of permits at a time, one per bidder call it is about to issue.
    /// The budget charges it as a bidder RESPONSE being decoded, which
    /// is what it is.
    pub streams: Arc<StreamGate>,
    /// Auction egress class (`egress_permits`).
    pub egress: Arc<EgressGate>,
    /// Aux-route class (`aux_route_permits`), separate from admission so
    /// an aux flood cannot consume auction capacity.
    pub aux: Arc<AuxGate>,
    /// Checked non-GVL peak and its process-wide reservation ledger.
    pub aux_work_derived: AuxWorkDerived,
    pub aux_work_budget: AuxWorkBudget,
    /// Static-response class (`terminal_response_permits`): one permit
    /// per terminal response, whatever route produced it. It bounds
    /// TRANSMISSION rather than a route's work, which is why responses
    /// that never pass a route gate — a pre-admission rejection, an
    /// admission refusal — still draw from it.
    pub static_egress: Arc<StaticEgressGate>,
    /// The validated effective envelope the capacities came from, retained so
    /// per-`App` configuration can read the byte/node caps without
    /// recomputing or re-validating them.
    pub limits: ResolvedIsolateLimits,
    /// Disjoint measured phase charges, retained even on an over-ceiling boot.
    pub derived: Derived,
    /// Actual owning-layout costs retained for installed and diagnostic derivation.
    pub construction_costs: RequestConstructionCosts,
    /// Catalog increments retained unchanged for diagnostic and request derivation.
    pub catalog_costs: auctioneer_runtime::CatalogResourceCosts,
    /// Per-call metadata charge retained for diagnostic re-derivation.
    pub outbound_meta_bytes: u32,
    /// Measured baked configuration size used during installation.
    pub baked_config_bytes: u64,
    /// The platform constants validated alongside the envelope.
    ///
    /// Retained for the same reason as `limits`: per-`App`
    /// configuration reads figures off it. The request-TARGET cap
    /// lives here rather than on the envelope row, and dropping this
    /// after validation is why that cap went uninstalled while the
    /// header caps did not.
    pub target: TargetPlatformConstants,
}

fn native_schema_scratch(costs: CatalogResourceCosts, total: u64, ceiling_bytes: u64) -> u64 {
    let Some(ceiling) = NonZeroU64::new(ceiling_bytes) else {
        return 0;
    };
    let headroom = minimum_headroom_bytes(ceiling, HeadroomClass::Native);
    if !cfg!(target_arch = "wasm32")
        && native_schema_funded(costs)
        && total
            .checked_add(headroom)
            .is_some_and(|required| required <= ceiling.get())
    {
        costs.schema_validation_per_auction_bytes
    } else {
        0
    }
}

impl BootGates {
    /// Native schema admission is stricter than legacy warn-and-boot: both the
    /// frozen reservations and the shared native headroom must actually fit.
    #[must_use]
    pub fn native_schema_scratch_bytes(&self) -> u64 {
        native_schema_scratch(
            self.catalog_costs,
            self.derived.total_bytes,
            self.limits.isolate_total_bytes,
        )
    }

    /// Recompute diagnostic terms from precisely the installed inputs, including
    /// normalized config caps, baked policy and owning construction layouts.
    ///
    /// # Errors
    /// Returns checked budget validation errors, without applying ceiling policy.
    pub fn rederive_budget(&self) -> Result<Derived, BudgetError> {
        derive_isolate_budget_with_baked_config(
            &self.limits,
            &self.target,
            self.outbound_meta_bytes,
            self.construction_costs,
            self.baked_config_bytes,
            self.catalog_costs,
        )
    }
}

static BOOT_GATES: OnceLock<BootGates> = OnceLock::new();
static BAKED_CONFIG_BYTES: OnceLock<Result<u64, BudgetError>> = OnceLock::new();

/// Effective config caps and the compiled defaults charged by budget derivation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BudgetInputs {
    /// Original row with only the three config-extraction caps resolved.
    pub limits: ResolvedIsolateLimits,
    /// Serialized bytes of the compiled host/account default pair, not metadata.
    pub baked_config_bytes: u64,
}

/// Resolve accounting inputs without installing admission or egress gates.
/// Nonzero config caps are preserved; zero uses the effective extractor's
/// default independently for each cap. Envelope validation remains the caller's
/// responsibility, so reports can inspect invalid and over-ceiling rows.
///
/// # Errors
/// Returns [`BudgetError::BudgetArithmeticOverflow`] if the compiled defaults'
/// serialized byte count cannot be represented.
pub fn resolve_budget_inputs(
    input_limits: &ResolvedIsolateLimits,
) -> Result<BudgetInputs, BudgetError> {
    if schema_artifact_state() == SchemaArtifactState::Unqualified {
        return Err(BudgetError::UnqualifiedSchemaArtifact);
    }
    let extraction = extraction_limits_for(Some(input_limits));
    let mut limits = *input_limits;
    limits.max_runtime_config_bytes = extraction.max_backend_bytes;
    limits.max_runtime_config_blob_bytes = extraction.max_blob_bytes;
    limits.max_config_json_nodes = extraction.max_json_nodes;
    let baked_config_bytes = (*BAKED_CONFIG_BYTES.get_or_init(|| {
        serialized_json_bytes(&(
            auctioneer_config_data::host_default(),
            auctioneer_config_data::account_default(),
        ))
        .map_err(|_error| BudgetError::BudgetArithmeticOverflow)
    }))?;
    Ok(BudgetInputs {
        limits,
        baked_config_bytes,
    })
}

static PROBE_EGRESS_GATE: OnceLock<Arc<EgressGate>> = OnceLock::new();

/// Fires the over-ceiling warning once per process rather than once per
/// call.
///
/// `install_boot_envelope` is called per REQUEST on the serverless
/// adapters, and it re-validates on every call by design. Without this
/// the warning would repeat on every fetch a warm isolate serves —
/// which is not more informative, and a warning an operator learns to
/// filter out is a warning that no longer carries the safety margin
/// this one is standing in for. A cold isolate still boots and warns
/// exactly once, which is the case that matters.
static OVER_CEILING_WARNED: OnceLock<()> = OnceLock::new();

/// Validate the boot envelope and install the gates it sizes.
///
/// Idempotent and safe to call from a per-request entrypoint: the first
/// successful call installs, and every later call validates the envelope
/// and then finds the gates already present. Concurrent callers race on
/// the `OnceLock`, and exactly one install wins — the losers observe the
/// winner's gates, so every request in the isolate contends for one set
/// of counters.
///
/// An envelope whose worst-case total exceeds the platform ceiling
/// WARNS and installs: see the module docs for why that one refusal is
/// tolerated and every other one is not.
///
/// # Errors
/// Returns the envelope's own [`BudgetError`] when validation fails for
/// any reason other than the ceiling, having installed NOTHING. A
/// caller that gets an error must not serve routes: with no gates
/// installed every gate accessor answers `None` and the isolate would
/// refuse every request anyway, but failing boot makes the
/// misconfiguration visible instead of looking like a total outage.
pub fn install_boot_envelope(
    input_limits: &ResolvedIsolateLimits,
    target: &TargetPlatformConstants,
    outbound_meta_bytes: u32,
    construction_costs: RequestConstructionCosts,
    catalog_costs: auctioneer_runtime::CatalogResourceCosts,
) -> Result<(), BudgetError> {
    // Validate FIRST, unconditionally, and on every call. Doing it
    // before the `OnceLock` is what orders "envelope validated" ahead of
    // "gates installed"; doing it on every call means a second adapter
    // entrypoint passing a bad envelope is still told so, rather than
    // silently inheriting the first caller's gates.
    //
    // A worst-case total over the platform ceiling is the one refusal
    // this boot tolerates: it is an ambition the declared caps PERMIT,
    // not an inconsistency the code will hit on every request, and the
    // caps are due to be tuned against production traffic. Every other
    // variant still returns before a gate exists.
    let BudgetInputs {
        limits: normalized,
        baked_config_bytes,
    } = resolve_budget_inputs(input_limits)?;
    let limits = &normalized;
    let derived = derive_isolate_budget_with_baked_config(
        limits,
        target,
        outbound_meta_bytes,
        construction_costs,
        baked_config_bytes,
        catalog_costs,
    )?;
    let aux_work_derived = derive_aux_work_for_profile(limits, target, baked_config_bytes)?;
    let aux_work_capacity = aux_work_derived
        .concurrent_peak(limits.aux_route_permits)
        .map_err(BudgetError::AuxWork)?;
    if derived.total_bytes > limits.isolate_total_bytes && OVER_CEILING_WARNED.set(()).is_ok() {
        warn_over_ceiling(limits, &derived);
    }

    // `get_or_init` rather than `set`: a repeated call is a no-op, not
    // an error, because per-request entrypoints call it every time.
    let _installed = BOOT_GATES.get_or_init(|| BootGates {
        admission: AdmissionGate::new(limits.max_concurrent_auctions),
        streams: StreamGate::new(limits.max_active_body_streams),
        egress: EgressGate::new(limits.egress_permits),
        aux: AuxGate::new(limits.aux_route_permits),
        aux_work_derived,
        aux_work_budget: AuxWorkBudget::new(aux_work_capacity),
        static_egress: StaticEgressGate::new(limits.terminal_response_permits),
        limits: *limits,
        derived,
        construction_costs,
        outbound_meta_bytes,
        catalog_costs,
        baked_config_bytes,
        target: *target,
    });
    Ok(())
}

/// Report an over-ceiling envelope and name the terms that dominate it.
///
/// This warning REPLACES a boot refusal, so it is the only thing
/// standing between a deployment and an isolate whose declared caps
/// permit more memory than the platform has. It therefore states the
/// overshoot in bytes, names the largest terms that produce it, and
/// says plainly that the process is booting anyway — an operator who
/// reads only this line must still be able to act on it.
///
/// Reports the exact normalized caps and baked-config allowance retained by
/// boot; a second derivation must not silently omit either charge.
fn warn_over_ceiling(limits: &ResolvedIsolateLimits, derived: &Derived) {
    let required = derived.total_bytes;
    let ceiling = limits.isolate_total_bytes;
    let overshoot = required.saturating_sub(ceiling);
    log::warn!(
        target: edgezero_core::BOOT_LOG_TARGET,
        "boot envelope over ceiling: requires {required} bytes against the \
         {ceiling}-byte platform ceiling, {overshoot} bytes over. Booting anyway: \
         the caps state what production traffic is expected to need and are due \
         to be tuned against real traffic, so this is a deliberate deferral and \
         not a transient fault. An isolate under real load may be killed by the \
         platform for exceeding its memory ceiling."
    );

    warn_dominant_terms(limits, derived);
}

/// Name the largest terms of an over-ceiling total, largest first.
///
/// "You are N bytes over" is not actionable on its own; which cap to
/// look at is the whole content of the warning. The terms are reported
/// as the isolate charges them — a population times its unit cost —
/// because the cap an operator would move is the population.
fn warn_dominant_terms(limits: &ResolvedIsolateLimits, derived: &Derived) {
    let auctions = u64::from(limits.max_concurrent_auctions);
    let streams = u64::from(limits.max_active_body_streams);
    let mut terms = [
        (
            "max_concurrent_auctions",
            auctions.saturating_mul(derived.per_auction_peak),
            auctions,
            derived.per_auction_peak,
        ),
        (
            "max_active_body_streams",
            streams.saturating_mul(derived.per_stream_peak),
            streams,
            derived.per_stream_peak,
        ),
        (
            "egress_permits",
            derived.egress_term,
            u64::from(limits.egress_permits),
            limits.final_response_bytes,
        ),
        (
            "aux_route_permits",
            derived.aux_term,
            u64::from(limits.aux_route_permits),
            limits.aux_route_peak_bytes,
        ),
        (
            "terminal_response_permits",
            derived.terminal_term,
            u64::from(limits.terminal_response_permits),
            u64::from(limits.refusal_body_bytes),
        ),
        (
            "platform_concurrent_ceiling",
            derived.pre_admission_parser_term,
            0,
            0,
        ),
        ("gvl_parsed_bytes", derived.gvl_terms, 0, 0),
        ("baseline_bytes", limits.baseline_bytes, 0, 0),
        ("catalog_resident_term", derived.catalog_resident_term, 0, 0),
        (
            "catalog_initialization_term",
            derived.catalog_initialization_term,
            0,
            0,
        ),
    ];
    // Descending by charged bytes, so the first line an operator reads
    // is the cap with the most to give back.
    terms.sort_unstable_by_key(|term| Reverse(term.1));

    for (name, charged, population, unit) in terms.iter().take(3) {
        if *population == 0 {
            log::warn!(
                target: edgezero_core::BOOT_LOG_TARGET,
                "boot envelope over ceiling: {name} charges {charged} bytes"
            );
        } else {
            // The per-unit figure is the cap itself, not `charged /
            // population`: the egress classes multiply their cap by the
            // handoff factor and add transmit state on top, so the two
            // do not divide evenly and printing the quotient would
            // invent a cap that is not in the row. What an operator
            // moves is the population and the cap, which is what this
            // names.
            log::warn!(
                target: edgezero_core::BOOT_LOG_TARGET,
                "boot envelope over ceiling: {name} charges {charged} bytes \
                 at a population of {population}, each against a {unit}-byte cap"
            );
        }
    }
}

/// The installed gates, or `None` before boot completes.
///
/// `None` is a fail-closed answer, not an error to paper over: a
/// request that reaches a gate accessor before the install must be
/// refused rather than admitted ungated.
#[must_use]
pub fn boot_gates() -> Option<&'static BootGates> {
    BOOT_GATES.get()
}

/// The auction admission gate, or `None` before boot completes.
#[must_use]
pub fn admission_gate() -> Option<Arc<AdmissionGate>> {
    boot_gates().map(|gates| Arc::clone(&gates.admission))
}

/// The fanout wave gate, or `None` before boot completes.
///
/// Named for inbound body streams, which it does not count: see
/// [`BootGates::streams`].
#[must_use]
pub fn stream_gate() -> Option<Arc<StreamGate>> {
    boot_gates().map(|gates| Arc::clone(&gates.streams))
}

/// The auction egress gate, or `None` before boot completes.
#[must_use]
pub fn egress_gate() -> Option<Arc<EgressGate>> {
    boot_gates().map(|gates| Arc::clone(&gates.egress))
}

/// Reserved probe output slots, available even before bootstrap.
///
/// Their fixed population is independently funded by the runtime budget.
/// Initializing this once prevents bootstrap from replacing a gate whose
/// pre-boot responses still hold permits.
#[must_use]
pub fn probe_egress_gate() -> Arc<EgressGate> {
    Arc::clone(
        PROBE_EGRESS_GATE
            .get_or_init(|| EgressGate::new(auctioneer_runtime::PROBE_RESPONSE_PERMITS)),
    )
}

/// The aux-route gate, or `None` before boot completes.
#[must_use]
pub fn aux_gate() -> Option<Arc<AuxGate>> {
    boot_gates().map(|gates| Arc::clone(&gates.aux))
}

/// The static-response class, or `None` before boot completes.
#[must_use]
pub fn static_egress_gate() -> Option<Arc<StaticEgressGate>> {
    boot_gates().map(|gates| Arc::clone(&gates.static_egress))
}

/// The validated effective envelope, or `None` before boot completes.
#[must_use]
pub fn boot_limits() -> Option<&'static ResolvedIsolateLimits> {
    boot_gates().map(|gates| &gates.limits)
}

/// The validated platform constants, for configuration that reads a
/// figure the envelope row does not carry.
#[must_use]
pub fn boot_target() -> Option<&'static TargetPlatformConstants> {
    boot_gates().map(|gates| &gates.target)
}

#[cfg(test)]
#[cfg(not(target_family = "wasm"))]
mod tests {
    use super::{
        boot_gates, boot_limits, derive_aux_work_for_profile, install_boot_envelope,
        resolve_budget_inputs, serialized_json_bytes, warn_dominant_terms,
    };
    use crate::configure::configure_app;
    use crate::configure::extraction_limits_for;
    use crate::handlers::request_disclosure_audit::construction_costs;
    use auctioneer_bidder::RegistryBuildError;
    use auctioneer_bidder::schema::{SchemaArtifactState, schema_artifact_state};
    use auctioneer_runtime::budget::{CONFIG_ENVELOPE_VALUE_FACTOR, CONFIG_PARSE_FACTOR};
    use auctioneer_runtime::{
        BudgetError, Derived, ResolvedIsolateLimits, TargetPlatformConstants,
        derive_isolate_budget_with_baked_config, min_aux_route_peak_bytes,
    };
    use edgezero_core::app::App;
    use edgezero_core::config_store::{ConfigExtractionLimits, DEFAULT_CONFIG_JSON_NODES};
    use edgezero_core::error::EdgeError;
    use edgezero_core::router::RouterService;
    use std::env::{VarError, current_exe, var};
    use std::process::Command;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_schema_funding_requires_costs_and_actual_headroom_independently() {
        let costs = auctioneer_runtime::CatalogResourceCosts {
            resident_bytes: 0x0100_0000,
            initialization_bytes: 0x0080_0000,
            schema_validation_per_auction_bytes: 0x0008_0000,
            context_per_auction_bytes: 17,
        };
        for (demand, expected) in [
            (4_026_531_839, 0x0008_0000),
            (0xf000_0000, 0x0008_0000),
            (4_026_531_841, 0),
            (u64::MAX, 0),
        ] {
            assert_eq!(
                super::native_schema_scratch(costs, demand, 0x0001_0000_0000),
                expected
            );
        }
        assert_eq!(super::native_schema_scratch(costs, 0, 0), 0);
        for unfunded in [
            auctioneer_runtime::CatalogResourceCosts {
                resident_bytes: 0x00ff_ffff,
                ..costs
            },
            auctioneer_runtime::CatalogResourceCosts {
                initialization_bytes: 0x007f_ffff,
                ..costs
            },
            auctioneer_runtime::CatalogResourceCosts {
                schema_validation_per_auction_bytes: 0x0007_ffff,
                ..costs
            },
        ] {
            assert_eq!(
                super::native_schema_scratch(unfunded, 1, 0x0001_0000_0000),
                0
            );
        }
    }

    fn expected_schema_artifact_state() -> SchemaArtifactState {
        match var("AUCTIONEER__TEST__EXPECT_SCHEMA_ARTIFACT") {
            Err(VarError::NotPresent) => SchemaArtifactState::Normal,
            Ok(value) if value == "unqualified" => SchemaArtifactState::Unqualified,
            _ => panic!("invalid schema-artifact test expectation"),
        }
    }

    #[test]
    fn schema_artifact_state_matches_the_independent_build_expectation() {
        assert_eq!(schema_artifact_state(), expected_schema_artifact_state());
    }

    #[test]
    fn schema_artifact_budget_refusal_precedes_baked_config_and_gates() {
        let result = resolve_budget_inputs(&limits());
        match expected_schema_artifact_state() {
            SchemaArtifactState::Normal => {
                result.unwrap();
            }
            SchemaArtifactState::Unqualified => {
                assert_eq!(result, Err(BudgetError::UnqualifiedSchemaArtifact));
                assert!(super::BAKED_CONFIG_BYTES.get().is_none());
                assert!(boot_gates().is_none());
                assert!(boot_limits().is_none());
            }
        }
    }

    #[test]
    fn schema_artifact_app_configuration_refuses_with_the_typed_cause() {
        let mut app = App::new(RouterService::builder().build());
        let result = configure_app(&mut app);
        match expected_schema_artifact_state() {
            SchemaArtifactState::Normal => result.unwrap(),
            SchemaArtifactState::Unqualified => {
                let EdgeError::Internal { source, .. } = result.unwrap_err() else {
                    panic!("expected typed internal startup refusal");
                };
                assert_eq!(
                    source.downcast_ref::<BudgetError>(),
                    Some(&BudgetError::UnqualifiedSchemaArtifact)
                );
                assert!(boot_gates().is_none());
            }
        }
    }

    #[test]
    fn schema_artifact_registry_refuses_before_published_construction() {
        let result = auctioneer_bidder::configured_registry();
        match expected_schema_artifact_state() {
            SchemaArtifactState::Normal => {
                result.unwrap();
            }
            SchemaArtifactState::Unqualified => {
                assert_eq!(
                    result.err(),
                    Some(RegistryBuildError::UnqualifiedSchemaArtifact)
                );
                assert!(boot_gates().is_none());
            }
        }
    }

    const FOUR_GIB: u64 = 4 << 30;
    const TEST_CONFIG_BYTE_CAP: u64 = 1 << 18;
    const OVERSIZED_CONFIG_NODE_CAP: u32 = 1 << 20;
    // Frozen independently of the runtime constant so undercharging cannot
    // change both the installed budget and its expected value together.
    const EXPECTED_CONFIG_NODE_RETAINED_BYTES: u64 = 623;

    #[test]
    fn budget_input_resolution_defaults_each_config_cap_independently() {
        let defaults = ConfigExtractionLimits::default();
        for mask in 0_u8..8 {
            let mut input = limits();
            input.max_runtime_config_bytes = if mask & 1 == 0 { 0 } else { 3 << 20_u32 };
            input.max_runtime_config_blob_bytes = if mask & 2 == 0 { 0 } else { 1 << 19_u32 };
            input.max_config_json_nodes = if mask & 4 == 0 { 0 } else { 17_123 };
            let mut expected = input;
            if mask & 1 == 0 {
                expected.max_runtime_config_bytes = defaults.max_backend_bytes;
            }
            if mask & 2 == 0 {
                expected.max_runtime_config_blob_bytes = defaults.max_blob_bytes;
            }
            if mask & 4 == 0 {
                expected.max_config_json_nodes = defaults.max_json_nodes;
            }
            assert_eq!(resolve_budget_inputs(&input).unwrap().limits, expected);
        }
    }

    #[test]
    fn budget_input_resolution_counts_only_the_compiled_default_pair() {
        let expected = serde_json::to_vec(&(
            auctioneer_config_data::host_default(),
            auctioneer_config_data::account_default(),
        ))
        .unwrap()
        .len();
        assert!(expected > 0);
        assert_eq!(
            resolve_budget_inputs(&limits()).unwrap().baked_config_bytes,
            u64::try_from(expected).unwrap(),
        );
    }

    #[test]
    fn budget_input_resolution_is_noninstalling_and_idempotent() {
        assert!(boot_gates().is_none());
        let mut input = limits();
        input.max_runtime_config_bytes = 0;
        input.max_runtime_config_blob_bytes = u64::MAX;
        input.max_config_json_nodes = u32::MAX;
        let first = resolve_budget_inputs(&input).unwrap();
        assert_eq!(first.limits.max_runtime_config_blob_bytes, u64::MAX);
        assert_eq!(first.limits.max_config_json_nodes, u32::MAX);
        assert_eq!(resolve_budget_inputs(&first.limits).unwrap(), first);
        assert!(boot_gates().is_none());
    }

    #[test]
    fn dominant_catalog_isolate_costs_are_reported_without_population_scaling() {
        testing_logger::setup();
        let mut limits = limits();
        limits.aux_route_peak_bytes = 768 * 1_024 * 1_024;
        let mut derived = derive_isolate_budget_with_baked_config(
            &limits,
            &target(),
            4_384,
            construction_costs(),
            0,
            auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        derived.catalog_resident_term = 1_u64 << 60_u32;
        derived.catalog_initialization_term = 1_u64 << 59_u32;
        warn_dominant_terms(&limits, &derived);
        testing_logger::validate(|captured| {
            assert_eq!(captured.len(), 3);
            for (label, bytes) in [
                ("catalog_resident_term", derived.catalog_resident_term),
                (
                    "catalog_initialization_term",
                    derived.catalog_initialization_term,
                ),
            ] {
                assert!(captured.iter().any(|line| line.body
                    == format!("boot envelope over ceiling: {label} charges {bytes} bytes")));
            }
        });
    }

    fn limits() -> ResolvedIsolateLimits {
        ResolvedIsolateLimits {
            isolate_total_bytes: FOUR_GIB,
            baseline_bytes: 96_000_000,
            inbound_request_bytes: 0x0004_0000,
            inbound_parse_factor: auctioneer_runtime::MIN_INBOUND_PARSE_FACTOR,
            max_outbound_request_bytes: 0x0004_0000,
            max_outbound_raw_bytes: 0x0004_0000,
            max_transport_bytes: 0x0004_0000,
            max_decoded_bytes: 0x0004_0000,
            response_parse_factor: auctioneer_runtime::MIN_RESPONSE_PARSE_FACTOR,
            response_node_peak_bytes: auctioneer_runtime::MIN_RESPONSE_NODE_PEAK_BYTES,
            final_response_bytes: 0x0100_0000,
            egress_permits: 3,
            response_write_deadline_ms: 30_000,
            guest_decoder_bound_bytes: 0,
            // One aux permit and 256 KiB config caps keep the baseline within
            // 4 GiB, including the independently funded probe work.
            aux_route_permits: 1,
            aux_route_peak_bytes: min_aux_route_peak_bytes(
                TEST_CONFIG_BYTE_CAP,
                TEST_CONFIG_BYTE_CAP,
                u64::from(DEFAULT_CONFIG_JSON_NODES),
            ),
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            gvl_cache_entries: 4,
            gvl_parsed_bytes: auctioneer_runtime::MAX_GVL_PARSED_BYTES,
            max_bidders_per_auction: 2,
            max_outbound_requests: 2,
            max_concurrent_auctions: 2,
            max_active_body_streams: 2,
            max_runtime_config_bytes: TEST_CONFIG_BYTE_CAP,
            max_runtime_config_blob_bytes: TEST_CONFIG_BYTE_CAP,
            max_parsed_nodes: 0x0001_0000,
            max_copied_header_bytes: 4_096,
            max_admitted_header_bytes: 0x4000,
            max_admitted_header_count: 128,
            max_impressions_per_auction: 100,
            max_config_json_nodes: DEFAULT_CONFIG_JSON_NODES,
            terminal_response_permits: 68,
            refusal_body_bytes: 256,
            transmit_state_bytes: 4_096,
        }
    }

    fn target() -> TargetPlatformConstants {
        TargetPlatformConstants {
            platform_concurrent_ceiling: 384,
            header_entry_overhead_bytes: 64,
            parser_header_ceiling_bytes: 21_632,
            parser_header_knob_bytes: 21_633,
            max_request_target_bytes: 768,
            h1_max_buf_size_bytes: 24_576,
            h1_max_headers: 128,
            parser_max_header_entries: 676,
            pseudoheader_allowance_bytes: 1_024,
            parser_transient_bytes: 0x4000,
            egress_handoff_factor: 2,
        }
    }

    fn isolated_boot_case(test_name: &str) -> Option<(u32, u64)> {
        const NODES_ENV: &str = "AUCTIONEER__TEST__CONFIG_NODE_BUDGET__CHILD_NODES";
        const CEILING_ENV: &str = "AUCTIONEER__TEST__CONFIG_NODE_BUDGET__CHILD_CEILING";
        if let Ok(nodes) = var(NODES_ENV) {
            return Some((
                nodes.parse().expect("declared node cap"),
                var(CEILING_ENV)
                    .expect("child ceiling")
                    .parse()
                    .expect("ceiling"),
            ));
        }
        // Sweep normalization at one funded ceiling, then characterize forced
        // and realistic overshoots. Each case needs fresh boot OnceLocks.
        for (nodes, ceiling) in [
            1,
            0,
            DEFAULT_CONFIG_JSON_NODES.saturating_div(2),
            DEFAULT_CONFIG_JSON_NODES,
            DEFAULT_CONFIG_JSON_NODES * 2,
            u32::MAX,
        ]
        .into_iter()
        .map(|nodes| (nodes, u64::MAX))
        .chain([
            (DEFAULT_CONFIG_JSON_NODES, 1),
            (DEFAULT_CONFIG_JSON_NODES, FOUR_GIB),
            (DEFAULT_CONFIG_JSON_NODES * 2, FOUR_GIB),
            (OVERSIZED_CONFIG_NODE_CAP, FOUR_GIB),
        ]) {
            let child = Command::new(current_exe().expect("unit test executable"))
                .args(["--exact", test_name, "--nocapture"])
                .env(NODES_ENV, nodes.to_string())
                .env(CEILING_ENV, ceiling.to_string())
                .output()
                .expect("isolated boot test executes");
            let output = format!(
                "{}{}",
                String::from_utf8_lossy(&child.stdout),
                String::from_utf8_lossy(&child.stderr),
            );
            assert!(
                child.status.success(),
                "nodes={nodes}, ceiling={ceiling}: {output}"
            );
            assert!(
                output.contains("1 passed"),
                "the child must execute: {output}"
            );
        }
        None
    }

    #[test]
    fn config_node_budget_matches_effective_extraction_limits() {
        let Some((nodes, ceiling)) = isolated_boot_case(
            "host_primitives::gates::tests::config_node_budget_matches_effective_extraction_limits",
        ) else {
            return;
        };
        testing_logger::setup();
        let mut input = limits();
        input.max_config_json_nodes = nodes;
        input.isolate_total_bytes = ceiling;
        let expected_nodes = if nodes == 0 {
            DEFAULT_CONFIG_JSON_NODES
        } else {
            nodes
        };
        let extraction = extraction_limits_for(Some(&input));
        assert_eq!(extraction.max_json_nodes, expected_nodes);
        input.aux_route_peak_bytes = min_aux_route_peak_bytes(
            extraction.max_backend_bytes,
            extraction.max_blob_bytes,
            u64::from(expected_nodes),
        );

        let mut expected_limits = input;
        expected_limits.max_config_json_nodes = extraction.max_json_nodes;
        let baked_bytes = serialized_json_bytes(&(
            auctioneer_config_data::host_default(),
            auctioneer_config_data::account_default(),
        ))
        .unwrap();
        // Config extraction and sync-route work share this auxiliary reservation.
        input.aux_route_peak_bytes = input.aux_route_peak_bytes.max(
            derive_aux_work_for_profile(&expected_limits, &target(), baked_bytes)
                .unwrap()
                .peak_bytes(),
        );
        expected_limits.aux_route_peak_bytes = input.aux_route_peak_bytes;
        let catalog = auctioneer_runtime::CatalogResourceCosts::UNCHANGED;
        install_boot_envelope(&input, &target(), 4_384, construction_costs(), catalog)
            .expect("the effective config caps are funded, even over ceiling");
        let gates = boot_gates().expect("boot installed gates");
        let expected_config_term = input.max_runtime_config_bytes
            + input.max_runtime_config_bytes * CONFIG_ENVELOPE_VALUE_FACTOR
            + (input.max_runtime_config_blob_bytes + gates.baked_config_bytes)
                * CONFIG_PARSE_FACTOR
            + u64::from(expected_nodes) * EXPECTED_CONFIG_NODE_RETAINED_BYTES;
        assert_eq!(gates.derived.runtime_config_term, expected_config_term);
        assert_eq!(gates.derived.auction_charges.config, expected_config_term);
        let mut declared_limits = input;
        declared_limits.max_config_json_nodes = nodes;
        let declared = derive_isolate_budget_with_baked_config(
            &declared_limits,
            &target(),
            4_384,
            construction_costs(),
            gates.baked_config_bytes,
            auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
        )
        .expect("the declared row has enough aux capacity for diagnostic derivation");
        assert_eq!(
            declared.runtime_config_term,
            expected_config_term - u64::from(expected_nodes) * EXPECTED_CONFIG_NODE_RETAINED_BYTES
                + u64::from(nodes) * EXPECTED_CONFIG_NODE_RETAINED_BYTES,
        );
        if nodes == 0 {
            assert_ne!(
                declared.runtime_config_term, gates.derived.runtime_config_term,
                "zero must fund the default extractor, not a zero-node charge",
            );
        } else {
            assert_eq!(
                declared.runtime_config_term,
                gates.derived.runtime_config_term
            );
        }
        assert_eq!(
            boot_limits(),
            Some(&expected_limits),
            "boot getters must retain the extractor's actual node cap, not its declaration",
        );
        assert_eq!(
            input.max_config_json_nodes, nodes,
            "the caller's row is unchanged"
        );
        let app = App::build::<crate::App>(edgezero_core::PlatformMetadata::default())
            .expect("per-App configuration accepts the normalized row");
        assert_eq!(app.config_extraction_limits(), extraction);
        assert_effective_config_rederivation(gates, &expected_limits);
        install_boot_envelope(&input, &target(), 4_384, construction_costs(), catalog)
            .expect("repeated installation validates the same effective caps");
        let expect_warning =
            ceiling == 1 || (ceiling == FOUR_GIB && nodes == OVERSIZED_CONFIG_NODE_CAP);
        assert_ceiling_warning(&gates.derived, ceiling, expect_warning);
    }

    fn assert_effective_config_rederivation(
        gates: &super::BootGates,
        expected_limits: &ResolvedIsolateLimits,
    ) {
        assert_eq!(
            gates.derived,
            derive_isolate_budget_with_baked_config(
                expected_limits,
                &target(),
                4_384,
                construction_costs(),
                gates.baked_config_bytes,
                auctioneer_runtime::CatalogResourceCosts::UNCHANGED
            )
            .expect("the effective row derives"),
        );
        assert_eq!(gates.rederive_budget(), Ok(gates.derived));
    }

    fn assert_ceiling_warning(derived: &Derived, ceiling: u64, expect_warning: bool) {
        assert_eq!(
            derived.total_bytes > ceiling,
            expect_warning,
            "ordinary node caps must fit; the oversized cap must exceed the ceiling: \
             total={}, ceiling={ceiling}",
            derived.total_bytes,
        );
        testing_logger::validate(|captured| {
            let headlines: Vec<_> = captured
                .iter()
                .filter(|line| line.body.contains("Booting anyway"))
                .collect();
            if expect_warning {
                assert_eq!(derived.margin_bytes, 0);
                assert_eq!(headlines.len(), 1, "over-ceiling boot warns only once");
                assert!(headlines[0].body.contains(&derived.total_bytes.to_string()));
            } else {
                assert!(headlines.is_empty(), "a funded row does not warn");
            }
        });
    }

    #[test]
    fn config_node_budget_rejects_aux_below_effective_extraction_floor() {
        let Some((nodes, ceiling)) = isolated_boot_case(
            "host_primitives::gates::tests::config_node_budget_rejects_aux_below_effective_extraction_floor",
        ) else {
            return;
        };
        let mut input = limits();
        input.max_config_json_nodes = nodes;
        input.isolate_total_bytes = ceiling;
        let expected_nodes = if nodes == 0 {
            DEFAULT_CONFIG_JSON_NODES
        } else {
            nodes
        };
        let extraction = extraction_limits_for(Some(&input));
        assert_eq!(extraction.max_json_nodes, expected_nodes);
        let required = min_aux_route_peak_bytes(
            extraction.max_backend_bytes,
            extraction.max_blob_bytes,
            u64::from(expected_nodes),
        );
        input.aux_route_peak_bytes = required - 1;
        assert_eq!(
            install_boot_envelope(
                &input,
                &target(),
                4_384,
                construction_costs(),
                auctioneer_runtime::CatalogResourceCosts::UNCHANGED
            ),
            Err(BudgetError::AuxRoutePeakBelowExtractionFloor {
                configured: required - 1,
                required,
            }),
            "an underfunded effective extractor is fatal, even over ceiling",
        );
        assert!(boot_gates().is_none(), "a floor breach installs no gates");
    }
}
