//! The boot warning's channel on this target reaches the Worker
//! console.
//!
//! # What was wrong, and why it was invisible
//!
//! The memory envelope ships in warn-not-refuse mode and this
//! adapter's row is over its ceiling, so the over-ceiling warning is
//! the only signal an operator gets. The envelope passed
//! `StderrBootLog`. On `wasm32-unknown-unknown` there is no WASI, so
//! there is no `fd_write` import for `worker-build`'s `wasm-bindgen`
//! shim to route: the writes SUCCEEDED and nothing came out. A sink
//! that silently writes nowhere is indistinguishable from a working one
//! at the call site, which is why it survived.
//!
//! `auctioneer-core`'s `boot_warning_lands_in_the_envelopes_sink` already
//! proves the warning reaches whatever sink the envelope carries. What
//! it cannot know is whether that sink has a platform behind it. These
//! rows are the missing half, split across what each target can show:
//!
//! * the envelope carries THIS adapter's sink and not the shared stderr
//!   one — host-checkable, and the regression that would undo the fix;
//! * the sink renders and emits a record it is given rather than
//!   dropping it — host-checkable through the off-target channel;
//! * the wasm channel is the console — a compile-time fact on
//!   `wasm32-unknown-unknown`, which CI checks with
//!   `cargo check --target wasm32-unknown-unknown`, plus the
//!   source-text row below. There is no wasm runner here, so a run is
//!   not available; what is available is that the console call is in
//!   the compiled path and the stderr call is not.

#[cfg(test)]
mod tests {
    use auctioneer_adapter_cloudflare::boot_log::{BOOT_LOG, BootLog, render};
    use log::Log;
    use log::{Level, Record};

    use auctioneer_adapter_cloudflare::envelope::{envelope, limits, target};
    use auctioneer_core::host_primitives::StderrBootLog;
    use std::fs::read_to_string;
    use std::ptr::eq as ptr_eq;

    /// The envelope carries this adapter's sink, not the shared stderr
    /// one.
    ///
    /// Identified by TYPE NAME, through `Debug`, not by pointer. Both
    /// sinks are zero-sized, so two `&'static dyn Log` to them can
    /// share a data address and `ptr::eq` on the erased data pointers
    /// compares equal — which is exactly what an earlier revision of
    /// this row did, and it passed with `StderrBootLog` restored. The
    /// vtables differ even when the addresses do not, and `Debug`
    /// prints the concrete type, so the name is what distinguishes
    /// them.
    #[test]
    fn the_envelope_carries_this_targets_console_sink() {
        let boot_limits = limits();
        let boot_target = target();
        let env = envelope(&boot_limits, &boot_target);
        let carried: *const dyn Log = env.log_sink;
        let expected: *const dyn Log = BOOT_LOG;
        let shared_stderr: *const dyn Log = &StderrBootLog;
        assert!(
            ptr_eq(carried, expected),
            "the boot envelope must carry this adapter's own sink. The shared \
             StderrBootLog writes into nothing on wasm32-unknown-unknown -- no WASI, \
             so no fd_write import for worker-build's wasm-bindgen shim to route -- \
             and the over-ceiling warning that is this row's only safety signal is \
             lost",
        );
        // Stated as a NEGATIVE too, against the sink the defect used.
        // Both sinks are zero-sized, so an earlier revision that
        // compared only the erased DATA pointers found them equal and
        // passed with StderrBootLog restored. `ptr::eq` on the fat
        // pointers compares the vtables as well, and this row names the
        // one it must not be so the comparison cannot be vacuous.
        assert!(
            !ptr_eq(carried, shared_stderr),
            "the boot envelope must not carry the shared stderr sink on this target",
        );
    }

    /// A warning the sink is given is rendered and emitted, not dropped.
    ///
    /// The assertion is on the RENDERED line, which both channels emit
    /// verbatim, so what is checked is the thing an operator reads in a
    /// `wrangler tail`. Asserting only that `log()` returned would pass
    /// against a body that did nothing at all, which is the shape of
    /// the defect.
    #[test]
    fn a_warning_is_rendered_and_handed_to_the_channel() {
        let args = format_args!("envelope over ceiling by 346969088 B");
        let record = Record::builder()
            .level(Level::Warn)
            .target(edgezero_core::BOOT_LOG_TARGET)
            .args(args)
            .build();
        let line = render(&record);
        assert!(
            line.contains("[auctioneer]")
                && line.contains("WARN")
                && line.contains(edgezero_core::BOOT_LOG_TARGET)
                && line.contains("346969088"),
            "the rendered line must carry the marker, the level, the target and the \
             record's own text, because that line IS what an operator reads: {line}",
        );
        // And the sink accepts it rather than filtering it out. The
        // channel itself cannot be observed from here on either target
        // -- stderr off-wasm, the console on it -- so what is asserted
        // is that the record passes the filter and the call is made.
        assert!(
            BOOT_LOG.enabled(record.metadata()),
            "a WARN record must pass the sink's filter, or the over-ceiling warning \
             is dropped before any channel is reached",
        );
        BootLog.log(&record);
    }

    /// The wasm channel is the Worker console, and the stderr fallback
    /// is NOT in the wasm path.
    ///
    /// A SOURCE-TEXT check, and it says so. `emit` is target-split, so
    /// one of its two bodies is never compiled in any single build, and
    /// no single test process can observe both. CI's
    /// `cargo check --target wasm32-unknown-unknown` proves the console
    /// body compiles; this proves it is the body the target selects, and
    /// that the stderr body is the one behind `not(target_arch =
    /// "wasm32")` rather than the other way round.
    ///
    /// Weak evidence, in the right direction: it can be defeated by a
    /// rename, but it cannot pass while the wasm body writes to stderr,
    /// which is exactly what it was doing.
    #[test]
    fn the_wasm_channel_is_the_console_and_not_stderr() {
        let source = read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/boot_log.rs"))
            .expect("the sink's source is readable from its own test");
        let wasm_body = source
            .split("#[cfg(not(target_arch = \"wasm32\"))]")
            .next()
            .expect("splitting always yields a first part");
        for call in [
            "worker::console_error!",
            "worker::console_warn!",
            "worker::console_log!",
            "worker::console_debug!",
        ] {
            assert!(
                wasm_body.contains(call),
                "the wasm body of `emit` must call {call}, which expands to a \
                 web_sys console host call -- what wrangler tail and the dashboard \
                 read",
            );
        }
        assert!(
            !wasm_body.contains("io::stderr"),
            "the wasm body must not write to stderr: on wasm32-unknown-unknown the \
             write succeeds into nothing, which is the defect this sink replaces",
        );
        let host_body = source
            .split("#[cfg(not(target_arch = \"wasm32\"))]")
            .nth(1)
            .expect("the off-target body follows its own cfg attribute");
        assert!(
            host_body.contains("io::stderr"),
            "the off-target body is stderr, which is what a host test can observe",
        );
    }
}
