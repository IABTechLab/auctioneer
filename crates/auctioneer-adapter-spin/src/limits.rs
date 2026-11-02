//! Spin's outbound profile.
//!
//! Static documentation and expectation-test data only. This constant is not
//! boot-installed or read by the scheduler; it adds no SDK enforcement.
//! The independently installed memory budget lives in [`crate::envelope`].
//!
//! The levels are what upstream reports at the pinned revision rather
//! than a conservative guess. Spin's `outbound-batch-completion-order`
//! and `outbound-batch-slot-isolation` rows are both `Native` — it
//! drives complete eligible exchanges concurrently, as Axum and
//! Cloudflare do — while `outbound-deadlines` and
//! `outbound-batch-cancellation` are `BestEffort`.
//!
//! What the cancellation level means concretely: dropping the pending
//! batch future IS the whole cancellation contract auctioneer has.
//! Upstream's own note is that Spin cancellation stays cooperative
//! until host teardown is observable within a documented bound, and no
//! such bound is established. So auctioneer guarantees only its own half:
//! nothing late is published as on-time, and the wave lease releases by
//! RAII. Whether the host actually tore the request down is
//! `EdgeZero`'s concern and is `BestEffort` by design.

use auctioneer_core::outbound::{ElapsedSemantics, SupportLevel, TargetOutboundProfile};

/// What this adapter reports about its own outbound behaviour.
///
/// `CompletionTime` because the batch driver yields each slot as it
/// becomes terminal rather than harvesting in input order, so a fast
/// bidder behind a slow one reports its own latency. That is the
/// `Native` slot-isolation and completion-order pair, and it is the
/// difference between this target and Fastly's serial harvest.
pub const OUTBOUND_PROFILE: TargetOutboundProfile = TargetOutboundProfile {
    elapsed_semantics: ElapsedSemantics::CompletionTime,
    deadline_support: SupportLevel::BestEffort,
    cancellation_support: SupportLevel::BestEffort,
};
