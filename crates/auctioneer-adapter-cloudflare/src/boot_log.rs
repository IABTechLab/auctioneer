//! This target's `log` sink for boot diagnostics.
//!
//! # Why this module exists at all
//!
//! The memory envelope ships in warn-not-refuse mode: a row over its
//! ceiling boots with a warning rather than refusing, and this adapter's
//! row IS over its ceiling. So that warning is the only signal an
//! operator has, and it has to reach them.
//!
//! On Workers it did not. The envelope passed
//! [`StderrBootLog`](auctioneer_core::host_primitives::StderrBootLog), on
//! the reasoning that `worker-build` links the guest through
//! `wasm-bindgen`, whose shim routes guest stderr to the platform
//! console. The old comment said in as many words that this was "the
//! one link in this chain not verified by a local run", and it does not
//! hold: `wasm32-unknown-unknown` has no WASI, so there is no
//! `fd_write` import for a shim to route. The writes succeed into a
//! sink with no host behind it, and the warning is lost.
//!
//! So this sink calls the console directly. `worker::console_warn!`,
//! `console_error!`, `console_log!` and `console_debug!` expand to `web_sys` console
//! calls — real host calls through `wasm-bindgen`, which is what
//! `wrangler tail` and the dashboard read.
//!
//! # Why it is its own module
//!
//! `envelope.rs` is HOST-VISIBLE: it compiles and is unit-tested off
//! this target, and `worker` is a `cfg(target_arch = "wasm32")`
//! dependency. Naming a console sink there would have meant splitting
//! that function. Naming it here instead costs one `use` and keeps the
//! envelope's own arithmetic testable on the host, which is where every
//! row in it is proved.
//!
//! The off-target build gets stderr. Nothing deploys there — it exists
//! so the envelope's rows stay testable — and stderr is what a host
//! test can observe.

use auctioneer_core::host_primitives::install_boot_logger;
use edgezero_core::env_config::EnvConfig;
use log::{Level, LevelFilter, Log, Metadata, Record};
use std::sync::OnceLock;

static RUNTIME_LEVEL: OnceLock<LevelFilter> = OnceLock::new();

/// Resolve the Worker binding and install its backend before bootstrap.
/// The first install freezes the ordinary filter; existing logger owners
/// retain their sink and facade settings.
#[must_use]
pub fn install_logging<F>(lookup: F) -> bool
where
    F: FnOnce(&'static str) -> Option<String>,
{
    const LEVEL_ENV: &str = "EDGEZERO__LOGGING__LEVEL";
    let env = EnvConfig::from_vars(lookup(LEVEL_ENV).map(|value| (LEVEL_ENV, value)));
    let runtime_level = edgezero_core::resolve_logging_level(&env);
    let installed = install_boot_logger(|| {
        RUNTIME_LEVEL.get_or_init(|| runtime_level);
        BOOT_LOG
    });
    if installed {
        log::set_max_level(runtime_level.max(edgezero_core::BOOT_LOG_LEVEL));
    }
    installed
}

/// The line one record becomes, as both bodies emit it.
///
/// Factored out so a host test can assert the TEXT without reaching a
/// platform channel. The channel is what differs between targets; the
/// line is not, and an operator reading a `wrangler tail` sees this.
#[must_use]
pub fn render(record: &Record<'_>) -> String {
    format!(
        "[auctioneer] {level} {target}: {args}",
        level = record.level(),
        target = record.target(),
        args = record.args(),
    )
}

/// Apply the independent boot filter only to the exact boot target.
#[must_use]
pub fn carries(metadata: &Metadata<'_>, runtime_level: LevelFilter) -> bool {
    let filter = if metadata.target() == edgezero_core::BOOT_LOG_TARGET {
        edgezero_core::BOOT_LOG_LEVEL
    } else {
        runtime_level
    };
    metadata.level() <= filter
}

/// Hand `line` to this target's platform channel.
///
/// The WASM body calls Worker console macros, which are host calls
/// through `wasm-bindgen` — what `wrangler tail`
/// and the dashboard read. The severity split is so the dashboard's own
/// filter agrees with the record's level.
#[cfg(target_arch = "wasm32")]
pub fn emit(level: Level, line: &str) {
    match level {
        Level::Error => worker::console_error!("{}", line),
        Level::Warn => worker::console_warn!("{}", line),
        Level::Info => worker::console_log!("{}", line),
        Level::Debug | Level::Trace => worker::console_debug!("{}", line),
    }
}

/// The off-target body: process stderr, which is what a host test can
/// observe. Nothing deploys here — the target exists so the envelope's
/// rows stay testable off wasm.
#[cfg(not(target_arch = "wasm32"))]
pub fn emit(_level: Level, line: &str) {
    use std::io::{self, Write as _};
    // `write!` on a locked handle rather than `eprintln!`, which the
    // workspace's `clippy::restriction` group denies. The result is
    // deliberately discarded: a boot diagnostic that cannot be written
    // has nowhere else to go, and a logger that panicked trying to
    // report its own failure would be worse than the lost line.
    let mut handle = io::stderr().lock();
    let _unwritten = writeln!(handle, "{line}");
}

/// The sink [`crate::envelope::envelope`] installs.
///
/// One name, two channels, selected by target: the Worker console on
/// the deployment target, process stderr off it. A caller does not
/// choose, which is the point — the envelope names `BOOT_LOG` and the
/// target decides where it goes.
pub static BOOT_LOG: &(dyn Log + 'static) = &BootLog;

/// Writes one line per record to whatever this target's platform reads.
#[derive(Debug)]
pub struct BootLog;

impl Log for BootLog {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        carries(
            metadata,
            RUNTIME_LEVEL.get().copied().unwrap_or(LevelFilter::Warn),
        )
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        emit(record.level(), &render(record));
    }

    fn flush(&self) {
        // Nothing is buffered here: the console calls are synchronous
        // host calls and `io::stderr()` is unbuffered, so every line is
        // already through by the time `log` returns.
    }
}

#[cfg(test)]
mod tests {
    use super::carries;
    use log::{Level, LevelFilter, Metadata};

    #[test]
    fn only_the_exact_boot_target_has_an_independent_warn_filter() {
        for runtime_level in [
            LevelFilter::Off,
            LevelFilter::Error,
            LevelFilter::Warn,
            LevelFilter::Info,
            LevelFilter::Debug,
            LevelFilter::Trace,
        ] {
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
                    ("edgezero::boot::request", level <= runtime_level),
                ] {
                    let metadata = Metadata::builder().target(target).level(level).build();
                    assert_eq!(carries(&metadata, runtime_level), expected);
                }
            }
        }
    }
}
