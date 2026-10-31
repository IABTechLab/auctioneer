//! Dev-only `Rng` seeded from system time.

use auctioneer_runtime::Rng;
use std::sync::{Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct DevRng {
    state: Mutex<u64>,
}

impl DevRng {
    #[must_use]
    pub fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0x1234_5678, |d| {
                u64::try_from(d.as_nanos() & u128::from(u64::MAX)).unwrap_or(0x1234_5678)
            });
        Self {
            state: Mutex::new(nanos | 1),
        }
    }
}

impl Default for DevRng {
    fn default() -> Self {
        Self::new()
    }
}

impl Rng for DevRng {
    fn next_u64(&self) -> u64 {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut x = *state;
        x ^= x << 13_u32;
        x ^= x >> 7_u32;
        x ^= x << 17_u32;
        *state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

#[cfg(test)]
mod tests {
    use super::{DevRng, Rng as _};

    #[test]
    fn dev_rng_produces_distinct_values() {
        let rng = DevRng::new();
        assert_ne!(rng.next_u64(), rng.next_u64());
    }
}
