//! Fastly's outbound profile.
//!
//! Static documentation and expectation-test data only. This constant is not
//! boot-installed or read by the scheduler; it adds no SDK enforcement.
//! The independently installed memory budget lives in [`crate::envelope`].
//!
//! Every cell here is `BestEffort` or the degraded elapsed semantic, and
//! each one is what the pinned capability contract reports for this
//! target rather than a cautious guess:
//!
//! - `outbound-http` — `BestEffort`, because Compute reaches arbitrary
//!   origins through DYNAMIC BACKENDS, which a service may not have
//!   enabled. Fastly is the only target whose outbound HTTP itself is
//!   degraded.
//! - `outbound-deadlines` — `BestEffort`. The guest has no sleep to
//!   await between dispatch and harvest, so the wave-gate wait cannot be
//!   woken by a deadline; see [`crate::timer`].
//! - `outbound-batch-completion-order` — `BestEffort`, because the
//!   harvest is SERIAL, which is what forces the elapsed semantic below.
//! - `outbound-batch-cancellation` — `BestEffort`. Dropping the pending
//!   handles is the whole contract auctioneer has; nothing proves host-side
//!   teardown is finite.
//!
//! auctioneer guarantees only its own half of each gap: nothing late is
//! published as on-time, results stay positional, and wave leases release
//! by RAII. Platform-side behaviour is not asserted anywhere.

use auctioneer_core::outbound::{ElapsedSemantics, SupportLevel, TargetOutboundProfile};

/// What this adapter reports about its own outbound behaviour.
///
/// `ObservationTime` — and it is the ONLY profile that says so. Fastly is
/// `BestEffort` for `outbound-batch-completion-order`: the batch is
/// harvested serially in input order rather than yielding each slot as it
/// becomes terminal, so a slot's `elapsed` measures when auctioneer OBSERVED
/// it, which on a serial harvest includes waiting out its predecessors. A
/// fast bidder behind a slow one therefore looks slow.
/// Earlier-own-deadline expiry receipts instead report logical expiry, not
/// later serial observation. Pre-expiry typed failures retain their observed
/// terminal sample. Auctioneer forwards those authoritative durations unchanged.
///
/// That is a reporting fact, not a bug, and it is why the semantic is
/// named in the profile instead of being left implicit: a consumer
/// reading `responsetimemillis` has to know it is holding an observation
/// time and not the bidder's own latency. Nothing in auctioneer presents this
/// figure as a completion timestamp.
pub const OUTBOUND_PROFILE: TargetOutboundProfile = TargetOutboundProfile {
    elapsed_semantics: ElapsedSemantics::ObservationTime,
    deadline_support: SupportLevel::BestEffort,
    cancellation_support: SupportLevel::BestEffort,
};
