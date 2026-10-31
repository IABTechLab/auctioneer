//! Pre-bootstrap output admission needs an isolated process-global gate.

#[cfg(test)]
mod tests {
    use auctioneer_core::host_primitives::{boot_gates, probe_egress_gate};
    use auctioneer_runtime::PROBE_RESPONSE_PERMITS;
    use edgezero_core::app::App;
    use edgezero_core::body::Body;
    use edgezero_core::http::request_builder;
    use edgezero_core::ingress::{IngressDispatchOutcome, IngressFraming, IngressHeadAccounting};
    use edgezero_core::time::MonotonicInstant;
    use futures::executor::block_on;
    use std::iter::repeat_with;

    #[test]
    fn preboot_probe_outputs_are_bounded_and_recover_on_abandonment() {
        assert!(boot_gates().is_none());
        let app =
            App::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default()).unwrap();
        let dispatch = || {
            let request = request_builder()
                .uri("/status")
                .body(Body::empty())
                .unwrap();
            block_on(app.dispatch_ingress(
                request,
                MonotonicInstant::now(),
                IngressHeadAccounting::HostManaged,
                IngressFraming::HostManaged,
            ))
            .unwrap()
        };
        let gate = probe_egress_gate();
        let mut outputs: Vec<_> = repeat_with(|| {
            let IngressDispatchOutcome::Response(envelope) = dispatch() else {
                panic!("preboot probe has reserved output capacity");
            };
            envelope
        })
        .take(usize::try_from(PROBE_RESPONSE_PERMITS).unwrap())
        .collect();
        assert_eq!(gate.in_use(), PROBE_RESPONSE_PERMITS);
        assert!(matches!(dispatch(), IngressDispatchOutcome::Aborted));
        drop(outputs.pop());
        assert_eq!(gate.in_use(), PROBE_RESPONSE_PERMITS - 1);
        assert!(matches!(dispatch(), IngressDispatchOutcome::Response(_)));
        drop(outputs);
        assert_eq!(gate.in_use(), 0);
        assert!(
            boot_gates().is_none(),
            "probe admission does not bootstrap other gates"
        );
    }
}
