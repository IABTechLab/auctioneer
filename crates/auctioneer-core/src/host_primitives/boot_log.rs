//! The log sink the boot sequence installs before it validates
//! anything.
//!
//! # Why boot owns a sink at all
//!
//! The boot envelope's over-ceiling diagnostic is a WARNING rather than
//! a refusal, by deliberate policy: the declared caps say what
//! production traffic is expected to need, and tuning them requires separate
//! workload evidence. That policy's whole safety margin is the
//! warning reaching an operator. If it is emitted into a process with
//! no `log` sink, it is not a warning — it is silence, and the
//! deployment looks indistinguishable from one whose envelope fits.
//!
//! That is exactly what happened before this module existed, and on
//! every target:
//!
//! - **Axum** installs `SimpleLogger` inside `run_app`, which the
//!   adapter calls AFTER the boot sequence. The over-ceiling warning
//!   fired into no sink and the server then started talking.
//! - **Cloudflare and Spin**: the pinned `EdgeZero` revision's
//!   `init_logger` for both targets is a documented no-op, so those
//!   isolates have no `log` sink at any point unless the application
//!   installs one.
//!
//! The warning also fires at most once per process, guarded by a
//! `OnceLock` so a warm isolate does not repeat it on every fetch. That
//! guard is consumed on the FIRST validation, so a cold-start warning
//! dropped for want of a sink is suppressed permanently — no later
//! request re-emits it.
//!
//! # The shape
//!
//! [`install_boot_logger`] takes the sink the TARGET knows how to write
//! to, as a factory, exactly like the clock, rng, timer and cooperator
//! the same boot sequence installs. `auctioneer-core` links no platform
//! SDK, so it cannot write to a Worker console or a Fastly endpoint
//! itself; the adapter that does hands one in.
//!
//! [`StderrBootLog`] is the sink for every target whose platform
//! surfaces process stderr — native, and the WASI guests, where the
//! runtime captures it. A target whose logs go somewhere else passes
//! its own `log::Log`.
//!
//! # Idempotence, and losing the race
//!
//! The install is best-effort and never fails a boot. `log` permits one
//! global logger per process, and several things may want to be it: a
//! serverless entrypoint re-enters the boot sequence on every request,
//! a test harness installs a capturing logger, and `EdgeZero`'s own
//! `run_app` installs one on the targets where it is not a no-op. So a
//! second install is expected and is not an error — the first sink
//! wins, and what matters is only that SOME sink exists before the
//! envelope is validated.
//!
//! The level is set to at least `Warn`. An existing sink's level is
//! left alone, because whoever installed it chose it.

use log::{Level, LevelFilter, Log, Metadata, Record};
use std::io::{self, Write as _};
use std::sync::OnceLock;

/// Tracks whether this process has already run the install, so a
/// per-request entrypoint does not rebuild a sink on every fetch.
///
/// Separate from `log`'s own one-logger rule because the two answer
/// different questions: `log` reports whether a logger is installed by
/// anyone, while this reports whether THIS function has run. A boot
/// sequence that lost the race to a test harness must still not
/// construct a second sink on the next request.
static BOOT_LOGGER_INSTALLED: OnceLock<()> = OnceLock::new();

/// A `log::Log` writing one line per record to process stderr.
///
/// Deliberately minimal: it exists so a diagnostic emitted during boot
/// has somewhere to go, not to be a logging framework. No timestamp,
/// because the native target's real logger adds one and the wasm guests
/// have no clock at this point in boot; no filtering beyond the level,
/// because the records this sink is here for are the ones nothing else
/// is listening to yet.
#[derive(Debug)]
pub struct StderrBootLog;

impl Log for StderrBootLog {
    /// Warnings and errors always; anything quieter only when the
    /// process asked for it.
    ///
    /// `Level::Warn` is a FLOOR, not a ceiling. Reading only the floor
    /// made INFO and DEBUG unreachable for the process's whole life:
    /// `log` admits one global logger, boot installs this one before
    /// `run_app`, so the host's own `init()` then fails and discards
    /// its error — leaving every record this sink rejects with nowhere
    /// else to go. Startup and successful-request lines were lost even
    /// with the global level set to DEBUG.
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= Level::Warn || metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        // `write!` on a locked handle rather than `eprintln!`, which
        // the workspace's `clippy::restriction` group denies. The result
        // is deliberately discarded: a boot diagnostic that cannot be
        // written has nowhere else to go, and a logger that panicked or
        // recursed trying to report its own failure would be worse than
        // the lost line.
        let mut handle = io::stderr().lock();
        let _unwritten = writeln!(
            handle,
            "[auctioneer] {level} {target}: {args}",
            level = record.level(),
            target = record.target(),
            args = record.args(),
        );
    }

    fn flush(&self) {
        // `io::stderr()` is unbuffered, so every line is already
        // through by the time `log` returns and there is nothing held
        // here to flush.
    }
}

/// Install `sink` as this process's `log` sink, if nothing has claimed
/// that role yet.
///
/// Best-effort and infallible by design: see the module docs for why a
/// lost race is the expected case rather than a failure. The sink is
/// built from a factory so a target that never needs one — because
/// something else already installed a logger — pays nothing to
/// construct it.
///
/// Returns `true` when THIS call installed the sink, which is what the
/// boot-order regression asserts.
pub fn install_boot_logger<F>(make_sink: F) -> bool
where
    F: FnOnce() -> &'static (dyn Log + 'static),
{
    let mut installed = false;
    BOOT_LOGGER_INSTALLED.get_or_init(|| {
        if log::set_logger(make_sink()).is_ok() {
            // Only raise the ceiling. A logger installed by someone else
            // carries their chosen level, and this call is not the place
            // to overrule it — but a sink installed HERE must pass the
            // warnings it was installed for.
            if log::max_level() < LevelFilter::Warn {
                log::set_max_level(LevelFilter::Warn);
            }
            installed = true;
        }
    });
    installed
}

/// Install [`StderrBootLog`], for the targets whose platform surfaces
/// process stderr.
#[must_use]
pub fn install_stderr_boot_logger() -> bool {
    install_boot_logger(|| &StderrBootLog)
}

/// Install the stderr backend with independent boot and ordinary filters.
/// Existing logger owners retain their backend and facade settings.
#[must_use]
pub fn install_stderr_boot_logger_with_level(runtime_level: LevelFilter) -> bool {
    static SINK: OnceLock<FilteredStderrBootLog> = OnceLock::new();
    let installed =
        install_boot_logger(|| SINK.get_or_init(|| FilteredStderrBootLog { runtime_level }));
    if installed {
        log::set_max_level(runtime_level.max(edgezero_core::BOOT_LOG_LEVEL));
    }
    installed
}

struct FilteredStderrBootLog {
    runtime_level: LevelFilter,
}

impl Log for FilteredStderrBootLog {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        let level = if metadata.target() == edgezero_core::BOOT_LOG_TARGET {
            edgezero_core::BOOT_LOG_LEVEL
        } else {
            self.runtime_level
        };
        metadata.level() <= level
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            StderrBootLog.log(record);
        }
    }

    fn flush(&self) {
        StderrBootLog.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::{FilteredStderrBootLog, Log as _};
    use log::{Level, LevelFilter, MetadataBuilder};

    #[test]
    fn configured_stderr_backend_separates_boot_and_runtime_filters() {
        for runtime_level in [
            LevelFilter::Off,
            LevelFilter::Error,
            LevelFilter::Warn,
            LevelFilter::Info,
            LevelFilter::Debug,
            LevelFilter::Trace,
        ] {
            let sink = FilteredStderrBootLog { runtime_level };
            for level in [
                Level::Error,
                Level::Warn,
                Level::Info,
                Level::Debug,
                Level::Trace,
            ] {
                for (target, expected) in [
                    (
                        edgezero_core::BOOT_LOG_TARGET,
                        level <= edgezero_core::BOOT_LOG_LEVEL,
                    ),
                    ("auctioneer::request", level <= runtime_level),
                ] {
                    assert_eq!(
                        sink.enabled(&MetadataBuilder::new().target(target).level(level).build()),
                        expected
                    );
                }
            }
        }
    }
}
