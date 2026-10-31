use time::OffsetDateTime;

/// Portable monotonic instant.
///
/// This is `web_time::Instant`, the SAME type `EdgeZero` exposes as
/// `edgezero_core::time::MonotonicInstant`. Cargo unifies both crates
/// on one `web-time`, so an instant minted here can be handed straight
/// to `Deadline::at_instant` with no projection, epoch or conversion
/// in between. On `wasm32-unknown-unknown`, where
/// `std::time::Instant` does not exist, `web-time` backs it with
/// `performance.now()`.
pub type MonotonicInstant = web_time::Instant;

/// Wall clock + monotonic source.
pub trait Clock: Send + Sync {
    /// Wall clock — used for RFC3339 timestamps (cookie expiry, auctiontimestamp).
    fn now(&self) -> OffsetDateTime;

    /// Monotonic source — used for interval timing (responsetimemillis, tmax).
    /// Only differences between instants are meaningful.
    fn now_monotonic(&self) -> MonotonicInstant;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    struct FrozenClock {
        at: MonotonicInstant,
    }

    impl Clock for FrozenClock {
        #[inline]
        fn now(&self) -> OffsetDateTime {
            OffsetDateTime::UNIX_EPOCH
        }
        #[inline]
        fn now_monotonic(&self) -> MonotonicInstant {
            self.at
        }
    }

    #[test]
    fn clock_is_dyn_compatible() {
        let at = MonotonicInstant::now();
        let clock: &dyn Clock = &FrozenClock { at };
        assert_eq!(clock.now(), OffsetDateTime::UNIX_EPOCH);
        assert_eq!(clock.now_monotonic(), at);
    }

    #[test]
    fn monotonic_instants_subtract_to_durations() {
        let at = MonotonicInstant::now();
        let later = at
            .checked_add(Duration::from_millis(250))
            .expect("no overflow");
        assert_eq!(
            later.saturating_duration_since(at),
            Duration::from_millis(250)
        );
        assert_eq!(
            at.saturating_duration_since(later),
            Duration::ZERO,
            "a backwards difference saturates to zero rather than panicking"
        );
    }
}
