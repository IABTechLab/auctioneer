use crate::Rng;
use std::sync::{Mutex, PoisonError};

/// xorshift64* — deterministic, fast, sufficient for tests.
/// Reference: Marsaglia, "Xorshift RNGs" (2003). Identical output across
/// platforms for a given seed.
pub struct SeededRng {
    state: Mutex<u64>,
}

impl SeededRng {
    /// Seed must be non-zero (xorshift requirement).
    ///
    /// # Panics
    /// Panics if `seed == 0` — xorshift64*'s state machine fixed-points at zero.
    #[must_use]
    #[inline]
    pub fn new(seed: u64) -> Self {
        assert!(seed != 0_u64, "SeededRng seed must be non-zero");
        Self {
            state: Mutex::new(seed),
        }
    }
}

impl Rng for SeededRng {
    #[inline]
    fn next_u64(&self) -> u64 {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut value = *state;
        value ^= value << 13_u32;
        value ^= value >> 7_u32;
        value ^= value << 17_u32;
        *state = value;
        value.wrapping_mul(0x2545_F491_4F6C_DD1D_u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let lhs = SeededRng::new(42);
        let rhs = SeededRng::new(42);
        for _ in 0_u32..10_u32 {
            assert_eq!(lhs.next_u64(), rhs.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let lhs = SeededRng::new(42);
        let rhs = SeededRng::new(43);
        let mut diverged = false;
        for _ in 0_u32..10_u32 {
            if lhs.next_u64() != rhs.next_u64() {
                diverged = true;
                break;
            }
        }
        assert!(diverged);
    }

    #[test]
    fn first_three_values_are_fixed_for_seed_42() {
        // Pins the xorshift64*-with-scrambler output. If this assertion
        // changes, every downstream snapshot RNG test changes — deliberate.
        let rng = SeededRng::new(42);
        assert_eq!(rng.next_u64(), 0x089b_8aba_0d62_3742_u64);
        assert_eq!(rng.next_u64(), 0x95bc_77bf_ee2d_32a3_u64);
        assert_eq!(rng.next_u64(), 0xce46_e7f7_169b_a4d2_u64);
    }
}
