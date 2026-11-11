//! Run-A baseline probe: boot this target the way `main` boots it, make
//! the embedded GVL archive resident, then hold still so a supervising
//! process can read this process's RSS.
//!
//! This exists because the measurement contract's `baseline_bytes` is
//! *whole-isolate resident memory after boot*, and nothing inside the
//! serving binary can report that without either unsafe FFI
//! (`task_info` on this host) or a `#[global_allocator]`. Neither is
//! acceptable here: the workspace denies `unsafe_code`, and an allocator
//! counts cumulative allocation across the whole process, which would
//! miss the archive entirely — the archive is `include_bytes!` rodata in
//! the executable image and never passes through the allocator at all.
//!
//! So the split is deliberate: this probe does the booting, and
//! `scripts/measure-baseline.sh` does the reading, from outside, with
//! `ps`. The probe's only job is to reach a well-defined, quiescent
//! state, announce it, and hold that state long enough to be read.
//!
//! # Run A is a no-op boot
//!
//! No route is served and no auction is shaped. The per-auction factors
//! are a separate run whose figure is never added to this one.
//!
//! # Why the touch pass is not optional
//!
//! The archive ships inside the module, so it is in the baseline by
//! definition. But this host maps the executable's rodata lazily: a boot
//! that never dereferences those pages leaves them non-resident, and the
//! RSS reading then understates the baseline by roughly the archive
//! size. Reading one byte per 4 KiB page forces every one of them in.
//! It also removes the reading's dependence on page-cache state, which
//! is what makes repeated runs comparable rather than a lottery.

use std::hint::black_box;
use std::io::{self, Write as _};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use auctioneer_adapter_axum::{envelope, timer::TokioTimer};
use auctioneer_core::host_primitives::{
    DevRng, SystemClock, axum_production_strategy, bootstrap_adapter_with_envelope,
    install_health_counter_scope,
};
use auctioneer_runtime::NoopDecodeCooperator;
use edgezero_core::env_config::EnvConfig;

/// This host's page size. The touch stride only has to be no larger
/// than the real page size to reach every page; 4 KiB is the
/// measurement contract's stated stride and is a divisor of this host's
/// 16 KiB pages, so it is correct on both.
const TOUCH_STRIDE_BYTES: usize = 4_096;

/// Dereference one byte per [`TOUCH_STRIDE_BYTES`] of every embedded
/// archive blob and of the baked GCL, so each page of the executable's
/// rodata holding them is faulted in before the snapshot.
///
/// Returns the number of bytes walked, purely so the supervisor can
/// record what was made resident and notice if the archive changes size.
/// `black_box` is what keeps the reads from being optimized away: the
/// summed bytes are otherwise dead, and a release build would discard
/// the loop along with the page faults that are its entire point.
fn touch_embedded_archive() -> u64 {
    let mut walked: u64 = 0;
    let mut sink: u64 = 0;

    for entry in auctioneer_config_data::gvl_archive() {
        let raw: &[u8] = entry.raw;
        let mut offset = 0;
        while offset < raw.len() {
            if let Some(byte) = raw.get(offset) {
                sink = sink.wrapping_add(u64::from(*black_box(byte)));
            }
            offset = offset.saturating_add(TOUCH_STRIDE_BYTES);
        }
        // The final page of a blob whose length is not a multiple of the
        // stride is not reached by the loop above, so touch it by name.
        if let Some(last) = raw.last() {
            sink = sink.wrapping_add(u64::from(*black_box(last)));
        }
        walked = walked.saturating_add(u64::try_from(raw.len()).unwrap_or(u64::MAX));
    }

    let gcl = auctioneer_config_data::baked_gcl_json().as_bytes();
    let mut offset = 0;
    while offset < gcl.len() {
        if let Some(byte) = gcl.get(offset) {
            sink = sink.wrapping_add(u64::from(*black_box(byte)));
        }
        offset = offset.saturating_add(TOUCH_STRIDE_BYTES);
    }
    if let Some(last) = gcl.last() {
        sink = sink.wrapping_add(u64::from(*black_box(last)));
    }
    walked = walked.saturating_add(u64::try_from(gcl.len()).unwrap_or(u64::MAX));

    let _observed = black_box(sink);
    walked
}

/// How long the probe holds its post-boot state open for the supervisor
/// to read. Generous relative to a `ps` call, and bounded so an
/// abandoned probe cannot outlive the run that started it. The
/// supervisor signals the probe once it has its reading rather than
/// waiting this out.
const HOLD_DURATION: Duration = Duration::from_mins(2);

/// Boot, touch, report, hold still.
///
/// The probe prints two machine-readable lines to stdout and then sleeps.
/// Sleeping is what makes the reading well-defined: the process is
/// quiescent at snapshot time, so the supervisor's `ps` reads a settled
/// figure rather than racing boot. It allocates nothing while held, so a
/// late reading and an early one describe the same process.
fn main() -> anyhow::Result<()> {
    envelope::validate_sdk_settings(&EnvConfig::from_env())?;
    // The same sequence `main` runs, in the same order, through the same
    // production entry point: envelope validated and gates installed,
    // then host primitives, then the baked GCL static. Boot order
    // matters to the figure, so the probe does not reimplement it.
    let profile = envelope::resource_profile();
    bootstrap_adapter_with_envelope(
        profile.boot_envelope(),
        || Arc::new(SystemClock::new()),
        || Arc::new(DevRng::new()),
        || Arc::new(TokioTimer),
        || Arc::new(NoopDecodeCooperator),
        axum_production_strategy,
    )
    .map_err(|error| anyhow::anyhow!("boot envelope rejected: {error}"))?;

    install_health_counter_scope("process")?;

    let walked = touch_embedded_archive();

    let mut stdout = io::stdout();
    writeln!(stdout, "auctioneer-baseline-probe archive_bytes={walked}")?;
    writeln!(stdout, "auctioneer-baseline-probe ready")?;
    stdout.flush()?;

    // Hold the booted state still. The supervisor reads RSS and then
    // terminates the probe; this bound only covers the case where it
    // does not.
    thread::sleep(HOLD_DURATION);
    Ok(())
}
