//! Cloudflare's outbound profile.
//!
//! Static documentation and expectation-test data only. This constant is not
//! boot-installed or read by the scheduler; it adds no SDK enforcement.
//! The independently installed memory budget lives in [`crate::envelope`].
//!
//! Both support levels are `BestEffort`, and that is what upstream
//! reports at the pinned revision rather than a conservative guess:
//! Cloudflare's `outbound-deadlines` and `outbound-batch-cancellation`
//! rows are both `BestEffort` in the generated capability table, where
//! only Axum is `Native` for either.
//!
//! What the cancellation level means concretely: dropping the pending
//! batch future IS the whole cancellation contract auctioneer has, and the
//! adapter below it issues a guest-side `AbortController::abort()` per
//! Fetch. The abort is REQUESTED, not proven finite — there is no
//! deployed evidence of bounded host-side teardown — so auctioneer
//! guarantees only its own half: nothing late is published as on-time,
//! and the wave lease releases by RAII. Origin-side termination is not
//! something auctioneer asserts.

use auctioneer_core::outbound::{ElapsedSemantics, SupportLevel, TargetOutboundProfile};

/// What this adapter reports about its own outbound behaviour.
///
/// `CompletionTime` because the batch driver yields each slot as it
/// becomes terminal rather than harvesting in input order — Cloudflare
/// is `Native` for both `outbound-batch-completion-order` and
/// `outbound-batch-slot-isolation` — so a fast bidder behind a slow one
/// reports its own latency.
pub const OUTBOUND_PROFILE: TargetOutboundProfile = TargetOutboundProfile {
    elapsed_semantics: ElapsedSemantics::CompletionTime,
    deadline_support: SupportLevel::BestEffort,
    cancellation_support: SupportLevel::BestEffort,
};
