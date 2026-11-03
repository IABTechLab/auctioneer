//! Cloudflare Workers `Rng`. xorshift64* seeded from
//! `worker::Date::now() XOR Math::random()`.
//!
//! Not `crypto.getRandomValues` per call — the seeded xorshift path is
//! pinned for test determinism. This is not a cryptographic random source.

use auctioneer_runtime::Rng;
use js_sys::Math;
use std::sync::{Mutex, PoisonError};
use worker::Date;

pub struct CloudflareRng {
    state: Mutex<u64>,
}

impl CloudflareRng {
    #[must_use]
    pub fn new() -> Self {
        let now_ms = Date::now().as_millis();
        // `Math::random()` returns f64 in [0, 1); scale to 2^53 (the
        // JS-safe integer range) and floor. The result is losslessly
        // representable as u64.
        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::float_arithmetic,
            reason = "JS random is in [0,1); scaling by exact powers of two to 2^53 and flooring produces an integer in 0..=2^53-1 that fits u64 losslessly"
        )]
        let math_bits = (Math::random() * f64::from(1_u32 << 30_u32) * f64::from(1_u32 << 23_u32))
            .floor() as u64;
        let mut seed = now_ms ^ math_bits;
        if seed == 0 {
            seed = 1;
        }
        Self {
            state: Mutex::new(seed),
        }
    }
}

impl Default for CloudflareRng {
    fn default() -> Self {
        Self::new()
    }
}

impl Rng for CloudflareRng {
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
