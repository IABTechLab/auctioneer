//! Global-level assertions need isolation from unit-test capturing loggers.

#[cfg(test)]
mod tests {
    use auctioneer_core::host_primitives::StderrBootLog;
    use log::{Level, LevelFilter, Log as _, MetadataBuilder};
    use std::sync::{Mutex, PoisonError};

    static LEVEL_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn the_boot_sink_passes_warnings_and_errors_only() {
        let _guard = LEVEL_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let enabled = |level| {
            StderrBootLog.enabled(
                &MetadataBuilder::new()
                    .level(level)
                    .target("auctioneer::budget")
                    .build(),
            )
        };
        assert!(enabled(Level::Error), "an error reaches the boot sink");
        assert!(
            enabled(Level::Warn),
            "the over-ceiling diagnostic is a warning, so this is the load-bearing level",
        );
        assert!(
            !enabled(Level::Info),
            "boot-time info is not what this sink exists for",
        );
        assert!(!enabled(Level::Debug));
        assert!(!enabled(Level::Trace));
    }

    #[test]
    fn the_sink_widens_when_the_process_asks_for_it() {
        let _guard = LEVEL_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let enabled = |level| {
            StderrBootLog.enabled(
                &MetadataBuilder::new()
                    .level(level)
                    .target("auctioneer::budget")
                    .build(),
            )
        };
        let restore = log::max_level();
        log::set_max_level(LevelFilter::Debug);
        let info = enabled(Level::Info);
        let debug = enabled(Level::Debug);
        let trace = enabled(Level::Trace);
        let warn = enabled(Level::Warn);
        log::set_max_level(restore);

        assert!(info, "a process configured for DEBUG must receive INFO");
        assert!(debug, "a process configured for DEBUG must receive DEBUG");
        assert!(
            !trace,
            "TRACE stays filtered at DEBUG rather than being enabled unconditionally",
        );
        assert!(warn, "the warning floor holds at every configured level");
    }
}
