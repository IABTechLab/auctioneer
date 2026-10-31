//! Axum's outbound profile.
//!
//! Static documentation and expectation-test data only. This constant is not
//! boot-installed or read by the scheduler; it adds no SDK enforcement.
//!
//! Axum is the ONE target that is Native for both per-request deadlines
//! and cancellation: it runs on tokio, where a dropped future stops
//! polling and `reqwest`'s own timeout is a real timer rather than a
//! request the platform may decline. Every other target is `BestEffort` for
//! at least one of the two, so this is the profile the others are
//! measured against.

use auctioneer_core::outbound::{ElapsedSemantics, SupportLevel, TargetOutboundProfile};

/// What this adapter reports about its own outbound behaviour.
///
/// `CompletionTime` because the tokio driver races every slot against
/// every other and reports each one's own completion — nothing is
/// harvested serially, so a fast bidder behind a slow one is not charged
/// for the wait.
pub const OUTBOUND_PROFILE: TargetOutboundProfile = TargetOutboundProfile {
    elapsed_semantics: ElapsedSemantics::CompletionTime,
    deadline_support: SupportLevel::Native,
    cancellation_support: SupportLevel::Native,
};

#[cfg(test)]
mod tests {
    use super::OUTBOUND_PROFILE;
    use auctioneer_core::outbound::{ElapsedSemantics, SupportLevel};

    /// Checks the static Axum profile against hardcoded expected values.
    ///
    /// This does not query SDK capability metadata or exercise provider
    /// behavior. It detects declaration changes relative to these literals,
    /// not SDK drift or deadline/cancellation guarantees. The profile is not
    /// installed or used for runtime enforcement.
    ///
    /// The other three `capability_table_matches_*` checks live in their
    /// adapters' `tests/fanout.rs`; Axum is binary-only, so its check sits
    /// beside the constant.
    #[test]
    fn capability_table_matches_axum() {
        assert_eq!(
            OUTBOUND_PROFILE.elapsed_semantics,
            ElapsedSemantics::CompletionTime,
            "tokio races every slot and reports each one's own completion \
             rather than its observation, which is the semantic Fastly's \
             serial harvest cannot offer",
        );
        assert_eq!(
            OUTBOUND_PROFILE.deadline_support,
            SupportLevel::Native,
            "Axum is the target that genuinely enforces the deadline",
        );
        assert_eq!(
            OUTBOUND_PROFILE.cancellation_support,
            SupportLevel::Native,
            "and the one where dropping the batch provably stops the work",
        );
        assert_ne!(
            OUTBOUND_PROFILE.deadline_support,
            SupportLevel::BestEffort,
            "Axum is the reference the other three are measured against, so a \
             deadline row degrading here invalidates their `only Axum is \
             Native` claim rather than just weakening this target",
        );
        assert_ne!(
            OUTBOUND_PROFILE.cancellation_support,
            SupportLevel::BestEffort,
            "including cancellation: no target would be Native for it, and \
             nothing else in the suite would notice",
        );
        assert_ne!(
            OUTBOUND_PROFILE.elapsed_semantics,
            ElapsedSemantics::ObservationTime,
            "ObservationTime is Fastly's serial-harvest semantic and must \
             stay the only profile that reports it",
        );
    }
}
