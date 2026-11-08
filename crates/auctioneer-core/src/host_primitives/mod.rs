//! Dev host primitives — clock, RNG, timer, decode cooperator, and
//! fanout strategy. Wired into `AuctionContext` by the per-adapter
//! `bootstrap_adapter` calls.

mod bidder_credentials;
pub mod boot_log;
pub mod bootstrap;
pub mod clock;
pub mod edgezero_proxy;
pub mod error_renderer;
pub mod fanout;
pub mod gates;
pub mod ingress_lease;
pub mod race;
pub mod registry;
pub mod request_clock;
pub mod resource_profile;
pub mod rng;
pub mod static_egress;
pub mod strategy;

pub use auctioneer_exchange::construction_diagnostics::construction_diagnostic_costs;
pub use bidder_credentials::CredentialLoader;
pub use boot_log::{
    StderrBootLog, install_boot_logger, install_stderr_boot_logger,
    install_stderr_boot_logger_with_level,
};
pub use bootstrap::{BootEnvelope, bootstrap_adapter, bootstrap_adapter_with_envelope};
pub use clock::SystemClock;
pub use edgezero_proxy::{EdgeZeroFanout, EdgeZeroProxyAdapter};
pub use error_renderer::render_error_response;
pub use fanout::{DevFanout, ProxyClientApi};
pub use gates::{
    BootGates, BudgetInputs, admission_gate, aux_gate, boot_gates, boot_limits, egress_gate,
    install_boot_envelope, probe_egress_gate, resolve_budget_inputs, static_egress_gate,
    stream_gate,
};
pub use ingress_lease::{
    AuctioneerIngressLease, RouteAdmissionMarker, RouteEgressPermit, admit, admit_pending,
    admit_pending_holding, admit_pending_holding_terminal, install_route_permit,
};
pub use race::BiasedRace3;
pub use registry::{
    BearerToken, HealthAccessPolicy, HealthAuth, HealthAuthz, HealthBootError, TestClockHandle,
    TestMonotonicClockHandle, TestRngHandle, TestStrategyHandle, TestTimerHandle, clock_handle,
    counter_scope_handle, decide_health_policy, decode_cooperator_handle, health_authorize,
    health_policy_handle, install_health_primitives, install_health_primitives_without_policy,
    read_bool_flag, rng_handle, strategy_handle, timer_handle,
};
pub use request_clock::RequestClock;
pub use resource_profile::ResourceProfile;
pub use rng::DevRng;
pub use static_egress::detached_egress_decision;
pub use strategy::{
    DelegateToDevStrategy, FanoutStrategy, RaceOutcome, RaceTimersStrategy,
    axum_production_strategy,
};
