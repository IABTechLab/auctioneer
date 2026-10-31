//! Cloudflare Workers `Clock`.
//!
//! `worker::Date::now()` supplies the WALL axis — ms since the Unix
//! epoch. The MONOTONIC axis is `MonotonicInstant::now()`, which
//! `web-time` backs with `performance.now()` on
//! `wasm32-unknown-unknown`, where `std::time::Instant` does not
//! exist. The two axes therefore come from separate sources, which is
//! what makes the monotonic one genuinely monotonic: `Date::now()`
//! can step backwards, so deriving interval timing from it would let a
//! JS-side clock adjustment produce a negative elapsed.

use auctioneer_runtime::{Clock, MonotonicInstant};
use time::OffsetDateTime;
use worker::Date;

pub struct CloudflareClock;

impl CloudflareClock {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for CloudflareClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for CloudflareClock {
    fn now(&self) -> OffsetDateTime {
        let now_ms = Date::now().as_millis();
        let nanos = i128::from(now_ms).saturating_mul(1_000_000_i128);
        OffsetDateTime::from_unix_timestamp_nanos(nanos).unwrap_or(OffsetDateTime::UNIX_EPOCH)
    }

    fn now_monotonic(&self) -> MonotonicInstant {
        MonotonicInstant::now()
    }
}
