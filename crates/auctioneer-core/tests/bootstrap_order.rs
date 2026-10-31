//! `bootstrap_adapter` is the one boot sequence every adapter
//! entrypoint runs, and the GCL lowering is part of it.
//!
//! This lives ALONE in its own test binary on purpose. Each integration
//! test file is a separate process, so the process-global `OnceLock`
//! behind the baked GCL starts empty here. A same-binary neighbour that
//! happened to touch the GCL first would leave the "not baked yet"
//! precondition already false and turn the assertion vacuous.

#[cfg(test)]
mod tests {
    use auctioneer_core::host_primitives::{
        DelegateToDevStrategy, DevRng, SystemClock, bootstrap_adapter,
    };
    use auctioneer_privacy::gcl::Gcl;
    use auctioneer_runtime::{ImmediateResolveTimer, NoopDecodeCooperator};
    use std::sync::Arc;

    /// Boot lowers the baked GCL. Asserting the "before" state first is
    /// what makes the "after" state evidence: without it the test would
    /// pass against a GCL some other code path had already initialised.
    #[test]
    fn bootstrap_adapter_initializes_gcl() {
        assert!(
            !Gcl::is_baked(),
            "nothing in this binary may lower the GCL before the boot sequence runs",
        );

        bootstrap_adapter(
            || Arc::new(SystemClock::new()),
            || Arc::new(DevRng::new()),
            || Arc::new(ImmediateResolveTimer),
            || Arc::new(NoopDecodeCooperator),
            || Arc::new(DelegateToDevStrategy),
        );

        assert!(
            Gcl::is_baked(),
            "the boot sequence must lower the baked GCL, not leave it to the first TCF request",
        );
    }
}
