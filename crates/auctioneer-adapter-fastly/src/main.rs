//! Thin entrypoint. Everything it boots lives in the library, so the
//! adapter's timer, cooperator and outbound profile are reachable from an
//! integration test; a `[[bin]]`-only crate exports nothing.

#[cfg(target_arch = "wasm32")]
fn main() -> Result<(), fastly::Error> {
    auctioneer_adapter_fastly::run()
}

#[cfg(not(target_arch = "wasm32"))]
#[expect(
    clippy::print_stderr,
    reason = "host stub; the real binary only runs on wasm32-wasip1"
)]
fn main() {
    eprintln!("auctioneer-adapter-fastly: target wasm32-wasip1 to run on Fastly.");
}
