/// Entropy source.
pub trait Rng: Send + Sync {
    fn next_u64(&self) -> u64;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, PoisonError};

    struct CountingRng(Mutex<u64>);
    impl Rng for CountingRng {
        #[inline]
        fn next_u64(&self) -> u64 {
            let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
            let value = *state;
            *state = value.wrapping_add(1);
            value
        }
    }

    #[test]
    fn rng_is_dyn_compatible_and_stateful() {
        let rng: &dyn Rng = &CountingRng(Mutex::new(7));
        assert_eq!(rng.next_u64(), 7);
        assert_eq!(rng.next_u64(), 8);
    }
}
