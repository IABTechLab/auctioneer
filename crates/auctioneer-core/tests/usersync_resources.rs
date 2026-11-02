mod fanout_limits;
use fanout_limits as envelope;

#[cfg(test)]
mod tests {
    use super::envelope;
    use auctioneer_core::{
        handlers::request_disclosure_audit::construction_costs as core_construction_costs,
        host_primitives::{boot_gates, install_boot_envelope},
        usersync::{
            cookie::MAX_SET_COOKIE_FIELD_BYTES,
            limits::CookieLimits,
            wire::{MAX_COOKIE_SYNC_BODY_BYTES, MAX_QUERY_BYTES, MAX_QUERY_PARAMS},
        },
    };
    use auctioneer_runtime::TargetPlatformConstants;
    use edgezero_core::app::App;
    use edgezero_core::body::Body;
    use edgezero_core::http::{Request, StatusCode, request_builder};
    use edgezero_core::ingress::{
        IngressDispatchOutcome, IngressFraming, IngressHeadAccounting,
        validate_normalized_ingress_parts,
    };
    use edgezero_core::time::MonotonicInstant;
    use futures::executor::block_on;

    #[test]
    fn app_installs_boot_target_limit_for_encoded_sync_query() {
        let limits = envelope::limits();
        let target = TargetPlatformConstants {
            platform_concurrent_ceiling: 384,
            header_entry_overhead_bytes: 64,
            parser_header_ceiling_bytes: 55_424,
            parser_header_knob_bytes: 55_425,
            max_request_target_bytes: 33 * 1_024,
            h1_max_buf_size_bytes: 0x0001_0000,
            h1_max_headers: 128,
            parser_max_header_entries: 1_732,
            pseudoheader_allowance_bytes: 34_816,
            parser_transient_bytes: 0x4000,
            egress_handoff_factor: 2,
        };
        let metadata = auctioneer_bidder::outbound_meta_bytes(limits.max_impressions_per_auction);
        install_boot_envelope(
            &limits,
            &target,
            metadata,
            core_construction_costs(),
            auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
        )
        .expect("boot envelope");
        let app = App::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default())
            .expect("App build");
        assert_eq!(
            app.ingress_head_limits().max_request_target_bytes(),
            u64::from(target.max_request_target_bytes)
        );
        let gates = boot_gates().expect("boot gates");
        let work = gates.aux_work_derived;
        let funded = work.limits();
        assert_eq!(
            funded.config_envelope_bytes,
            app.config_extraction_limits().max_backend_bytes
        );
        assert_eq!(
            funded.config_blob_bytes,
            app.config_extraction_limits().max_blob_bytes
        );
        assert_eq!(
            work.caps().config_nodes,
            app.config_extraction_limits().max_json_nodes
        );
        assert_eq!(
            funded.config_json_values,
            app.config_extraction_limits().max_json_nodes
        );
        assert_eq!(
            work.charges().nodes,
            u64::from(app.config_extraction_limits().max_json_nodes) * 623
                + funded.body_node_bytes
                + funded.cookie_node_bytes
        );
        assert_eq!(
            funded.body_bytes,
            u64::try_from(MAX_COOKIE_SYNC_BODY_BYTES).unwrap()
        );
        assert_eq!(funded.query_bytes, u64::try_from(MAX_QUERY_BYTES).unwrap());
        assert_eq!(
            funded.query_parameters,
            u32::try_from(MAX_QUERY_PARAMS).unwrap()
        );
        assert!(
            u64::from(work.caps().cookie_nodes)
                >= u64::try_from(CookieLimits::default().max_json_nodes).unwrap()
        );
        assert_eq!(
            funded.set_cookie_bytes,
            u64::try_from(MAX_SET_COOKIE_FIELD_BYTES).unwrap()
        );
    }

    #[test]
    fn configured_head_policy_admits_encoded_query_and_rejects_target_over_cap() {
        let limits = envelope::limits();
        let target = TargetPlatformConstants {
            platform_concurrent_ceiling: 384,
            header_entry_overhead_bytes: 64,
            parser_header_ceiling_bytes: 55_424,
            parser_header_knob_bytes: 55_425,
            max_request_target_bytes: 33 * 1_024,
            h1_max_buf_size_bytes: 0x0001_0000,
            h1_max_headers: 128,
            parser_max_header_entries: 1_732,
            pseudoheader_allowance_bytes: 34_816,
            parser_transient_bytes: 0x4000,
            egress_handoff_factor: 2,
        };
        let metadata = auctioneer_bidder::outbound_meta_bytes(limits.max_impressions_per_auction);
        install_boot_envelope(
            &limits,
            &target,
            metadata,
            core_construction_costs(),
            auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
        )
        .expect("boot envelope");
        let app = App::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default())
            .expect("App build");
        let dispatch = |path: String| {
            let request = request_builder()
                .uri(path)
                .body(Body::empty())
                .expect("request target");
            let (parts, body) = request.into_parts();
            validate_normalized_ingress_parts(&parts, app.ingress_head_limits())
                .expect("admitted normalized head");
            let admitted_request = Request::from_parts(parts, body);
            let outcome = block_on(app.dispatch_ingress(
                admitted_request,
                MonotonicInstant::now(),
                IngressHeadAccounting::HostManaged,
                IngressFraming::HostManaged,
            ))
            .expect("ingress result");
            match outcome {
                IngressDispatchOutcome::Response(mut response) => response.response_mut().status(),
                IngressDispatchOutcome::Aborted => panic!("request aborted"),
                _ => panic!("unexpected ingress outcome"),
            }
        };
        assert_ne!(
            dispatch(format!("/status?{}", "x".repeat(32 * 1_024))),
            StatusCode::URI_TOO_LONG
        );
        let over = request_builder()
            .uri(format!("/status?{}", "x".repeat(33 * 1_024)))
            .body(Body::empty())
            .expect("over-cap target");
        let (parts, _) = over.into_parts();
        let error = validate_normalized_ingress_parts(&parts, app.ingress_head_limits())
            .expect_err("over-cap target must be rejected before App dispatch");
        assert_eq!(error.status(), StatusCode::URI_TOO_LONG);
    }
}
