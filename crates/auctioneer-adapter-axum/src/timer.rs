//! Tokio-backed `Timer` — the real sleep primitive on the native host.

use async_trait::async_trait;
use auctioneer_runtime::Timer;
use std::time::Duration;
use tokio::time::sleep;

/// Native host timer shared by serving, baseline and live-wire entrypoints.
pub struct TokioTimer;

#[async_trait(?Send)]
impl Timer for TokioTimer {
    async fn sleep(&self, duration: Duration) {
        sleep(duration).await;
    }
}
