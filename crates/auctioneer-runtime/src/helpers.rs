use crate::Rng;
use std::num::NonZeroU64;

/// Lemire's nearly-divisionless unbiased bounded sampling.
/// Returns a value in `[0, upper)`.
///
/// # Panics
/// Never; this function does not panic.
#[must_use]
#[inline]
pub fn gen_range(rng: &dyn Rng, upper: NonZeroU64) -> u64 {
    let bound = upper.get();
    // Fast path for power-of-two: avoid rejection entirely.
    if bound.is_power_of_two() {
        return rng.next_u64() & bound.wrapping_sub(1_u64);
    }
    // Lemire's method: multiply, check whether low bits land in a rejection
    // zone, retry if so.
    loop {
        let candidate = rng.next_u64();
        let product = u128::from(candidate).wrapping_mul(u128::from(bound));
        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            reason = "Lemire's algorithm intentionally takes the low 64 bits"
        )]
        let lower = product as u64;
        #[expect(
            clippy::arithmetic_side_effects,
            clippy::integer_division_remainder_used,
            reason = "modulo is the core of the rejection-zone check; \
                      operands are both > 0 so the % cannot panic"
        )]
        let threshold = bound.wrapping_neg() % bound;
        if lower >= threshold {
            #[expect(
                clippy::as_conversions,
                reason = "high 64 bits of the 128-bit product; explicit by design"
            )]
            return (product >> 64_u32) as u64;
        }
    }
}

/// Fisher-Yates in-place shuffle.
///
/// # Panics
/// Never; every step below is total.
#[inline]
pub fn shuffle<T>(slice: &mut [T], rng: &dyn Rng) {
    for idx in (1_usize..slice.len()).rev() {
        // `idx + 1` as a `NonZeroU64` with no step that can wrap or be
        // zero: widening a `usize` index is lossless on every supported
        // target, and starting from `MIN` (= 1) makes the "+ 1" the
        // non-zero base rather than something to re-check afterwards.
        let Ok(idx_u64) = u64::try_from(idx) else {
            // Unreachable on 32-/64-bit targets; skipping the swap is
            // the total, allocation-free response to a hypothetical
            // wider `usize`.
            continue;
        };
        let bound = NonZeroU64::MIN.saturating_add(idx_u64);
        let drawn = gen_range(rng, bound);
        // `drawn < bound <= idx + 1 <= slice.len()`, so the narrowing
        // back to `usize` is lossless; `unwrap_or(idx)` keeps it total
        // and degenerates to a self-swap rather than a panic.
        let swap_with = usize::try_from(drawn).unwrap_or(idx);
        slice.swap(idx, swap_with);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SeededRng;
    use std::collections::BTreeSet;

    #[test]
    fn gen_range_power_of_two_fast_path() {
        let rng = SeededRng::new(1);
        let upper = NonZeroU64::new(8).expect("8 is non-zero");
        for _ in 0_u32..1_000_u32 {
            let value = gen_range(&rng, upper);
            assert!(value < 8);
        }
    }

    #[test]
    fn gen_range_within_bounds() {
        let rng = SeededRng::new(1);
        let upper = NonZeroU64::new(10).expect("10 is non-zero");
        for _ in 0_u32..10_000_u32 {
            let value = gen_range(&rng, upper);
            assert!(value < 10);
        }
    }

    #[test]
    fn shuffle_actually_moves_elements() {
        // With seed 42 and 20 elements, the result must not equal the input.
        let rng = SeededRng::new(42);
        let mut data = (0_i32..20_i32).collect::<Vec<i32>>();
        let original = data.clone();
        shuffle(&mut data, &rng);
        assert_ne!(data, original);
    }

    #[test]
    fn shuffle_is_a_permutation() {
        let rng = SeededRng::new(42);
        let mut data = (0_i32..20_i32).collect::<Vec<i32>>();
        let original: BTreeSet<_> = data.iter().copied().collect();
        shuffle(&mut data, &rng);
        let after: BTreeSet<_> = data.iter().copied().collect();
        assert_eq!(original, after, "shuffle preserves set of elements");
    }

    #[test]
    fn shuffle_is_deterministic_for_seed() {
        let rng_a = SeededRng::new(42);
        let rng_b = SeededRng::new(42);
        let mut left = (0_i32..10_i32).collect::<Vec<i32>>();
        let mut right = (0_i32..10_i32).collect::<Vec<i32>>();
        shuffle(&mut left, &rng_a);
        shuffle(&mut right, &rng_b);
        assert_eq!(left, right);
    }
}
