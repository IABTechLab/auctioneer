//! Dev-only `SystemClock`.

use auctioneer_runtime::{Clock, MonotonicInstant};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use time::OffsetDateTime;

pub struct SystemClock;

impl SystemClock {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        let dur = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO);
        let nanos = i128::try_from(dur.as_nanos()).unwrap_or(0_i128);
        OffsetDateTime::from_unix_timestamp_nanos(nanos).unwrap_or(OffsetDateTime::UNIX_EPOCH)
    }

    fn now_monotonic(&self) -> MonotonicInstant {
        MonotonicInstant::now()
    }
}

#[cfg(test)]
mod tests {
    use super::{Clock as _, SystemClock};

    #[test]
    fn monotonic_does_not_go_backwards() {
        let clock = SystemClock::new();
        let a = clock.now_monotonic();
        let b = clock.now_monotonic();
        assert!(b >= a);
    }

    #[test]
    fn now_is_after_unix_epoch() {
        let clock = SystemClock::new();
        assert!(clock.now() > time::OffsetDateTime::UNIX_EPOCH);
    }
}
