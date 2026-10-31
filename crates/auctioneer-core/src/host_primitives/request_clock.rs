//! Request-scoped composite clock.
//!
//! ONE clock source per request, not merely one instant type. The
//! monotonic half is `EdgeZero`'s own `MonotonicClock`, taken off the
//! `RequestContext`, so every instant the exchange and the fanout read
//! comes from the source `EdgeZero` itself is timing the request
//! against. The wall half stays the boot-installed host clock, which
//! is the only thing that knows the calendar.
//!
//! Splitting the two matters because they answer different questions.
//! `MonotonicInstant::now()` is never called on the auction path once
//! this is wired: it survives only inside `SystemClock` /
//! `CloudflareClock`, whose monotonic half the request path no longer
//! consults.

use auctioneer_runtime::{Clock, MonotonicInstant};
use edgezero_core::time::MonotonicClock;
use std::sync::Arc;
use time::OffsetDateTime;

/// Pairs `EdgeZero`'s injected monotonic clock with the host wall clock.
///
/// Built ONCE per request and handed to both the `AuctionContext` and
/// the `EdgeZeroFanout`, so the auction's deadline composition, the
/// fanout's wave race and the inclusive deadline recheck all read the
/// same source. A test that installs a `MonotonicClock` reading off a
/// `FakeClock` drives all three together.
pub struct RequestClock {
    monotonic: MonotonicClock,
    wall: Arc<dyn Clock + Send + Sync>,
}

impl RequestClock {
    #[must_use]
    pub fn new(monotonic: MonotonicClock, wall: Arc<dyn Clock + Send + Sync>) -> Self {
        Self { monotonic, wall }
    }
}

impl Clock for RequestClock {
    fn now(&self) -> OffsetDateTime {
        self.wall.now()
    }

    fn now_monotonic(&self) -> MonotonicInstant {
        self.monotonic.now()
    }
}

#[cfg(test)]
mod tests {
    use super::RequestClock;
    use auctioneer_runtime::{Clock, FakeClock, MonotonicInstant};
    use edgezero_core::time::MonotonicClock;
    use std::sync::Arc;
    use std::time::Duration;
    use time::OffsetDateTime;

    #[test]
    fn monotonic_half_reads_the_injected_clock_not_the_wall_one() {
        let wall = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let driver = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let driver_for_closure = Arc::clone(&driver);
        let wall_half: Arc<dyn Clock + Send + Sync> = Arc::<FakeClock>::clone(&wall);
        let clock = RequestClock::new(
            MonotonicClock::new(move || driver_for_closure.now_monotonic()),
            wall_half,
        );

        let before = clock.now_monotonic();
        driver.advance(Duration::from_millis(250));
        assert_eq!(
            clock.now_monotonic(),
            before
                .checked_add(Duration::from_millis(250))
                .expect("no overflow"),
            "advancing the injected monotonic source moves the composite's monotonic half"
        );

        // Advancing the WALL clock must not move the monotonic half,
        // which is what proves the two halves are separate sources.
        let monotonic_before = clock.now_monotonic();
        wall.advance(Duration::from_mins(1));
        assert_eq!(
            clock.now_monotonic(),
            monotonic_before,
            "the wall clock is not the monotonic source"
        );
        assert_eq!(
            clock.now(),
            OffsetDateTime::UNIX_EPOCH + Duration::from_mins(1),
            "the wall half reads the host clock"
        );
    }

    #[test]
    fn wall_half_is_the_host_clock_verbatim() {
        let start = OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("valid unix ts");
        let wall = Arc::new(FakeClock::new(start));
        let wall_half: Arc<dyn Clock + Send + Sync> = wall;
        let clock = RequestClock::new(MonotonicClock::new(MonotonicInstant::now), wall_half);
        assert_eq!(clock.now(), start);
    }
}
