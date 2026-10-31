//! Cold-parse benchmark for the Fastly fresh-sandbox lifecycle: every
//! request there starts with an empty cache, so the first
//! `PrivacyContext::resolve` pays a full `Gvl::load_from_json_bytes` on
//! the largest bundled entry. Target: p95 under 10 ms on production
//! hardware (run `cargo bench -p auctioneer-privacy --bench gvl_cold_parse`
//! and compare against your own fleet; debug-build figures are not
//! meaningful).
#![expect(
    clippy::print_stdout,
    reason = "benchmark harness reports its measurements on stdout"
)]

use auctioneer_privacy::gvl::Gvl;
use std::time::Instant;

fn main() {
    let Some(largest) = auctioneer_config::gvl_archive()
        .iter()
        .max_by_key(|entry| entry.raw.len())
    else {
        println!("gvl_cold_parse: no archive entries bundled");
        return;
    };
    let iterations: usize = 40;
    let mut samples_us: Vec<u128> = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let started = Instant::now();
        let parsed = Gvl::load_from_json_bytes(largest.raw, Some(largest.spec));
        let elapsed = started.elapsed().as_micros();
        if parsed.is_err() {
            println!("gvl_cold_parse: parse failed; archive/build gate disagree");
            return;
        }
        samples_us.push(elapsed);
    }
    samples_us.sort_unstable();
    let p50 = samples_us
        .get(samples_us.len().wrapping_div(2))
        .copied()
        .unwrap_or(0);
    let p95_index = samples_us
        .len()
        .saturating_mul(95)
        .wrapping_div(100)
        .min(samples_us.len().saturating_sub(1));
    let p95 = samples_us.get(p95_index).copied().unwrap_or(0);
    println!(
        "gvl_cold_parse: largest entry (spec {}, list {}, {} bytes): p50 {p50} us, p95 {p95} us over {iterations} iterations (target: p95 < 10ms)",
        largest.spec,
        largest.vendor_list_version,
        largest.raw.len(),
    );
}
