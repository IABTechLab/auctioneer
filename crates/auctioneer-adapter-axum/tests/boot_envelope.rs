//! Install the actual production row, not a second handwritten envelope.
//! The shipped-row installation test owns this process's boot state. The
//! oversized-row warning control installs only in a fresh child process;
//! the library envelope's co-located tests are pure budget/config checks.

#[cfg(test)]
mod tests {
    use auctioneer_adapter_axum::envelope::resource_profile;
    use auctioneer_core::host_primitives::{
        boot_gates, boot_limits, install_boot_envelope, install_boot_logger,
    };
    use auctioneer_runtime::{
        AuctionBudget, Derived, MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR,
        RequestConstructionCosts, ResolvedIsolateLimits, TargetPlatformConstants,
        derive_isolate_budget, derive_isolate_budget_with_baked_config, serialized_json_bytes,
    };
    use std::env::{current_exe, var_os};
    use std::process::Command;

    #[test]
    fn wire_privacy_target_is_default_and_local_bidder_support_is_test_only() {
        let manifest: toml::Value = toml::from_str(include_str!("../Cargo.toml")).unwrap();
        let target = manifest["test"]
            .as_array()
            .unwrap()
            .iter()
            .find(|target| target["name"].as_str() == Some("privacy_wire"))
            .expect("the socket privacy target is registered");
        assert!(
            target.get("required-features").is_none(),
            "default all-targets must run wire privacy, not silently skip it"
        );
        let insecure = toml::Value::String("insecure-bidders".into());
        assert!(
            manifest["dev-dependencies"]["auctioneer-core"]["features"]
                .as_array()
                .unwrap()
                .contains(&insecure),
            "test-only dependency enables the local socket bidder"
        );
        assert!(
            manifest["dependencies"]["auctioneer-core"]
                .get("features")
                .is_none_or(|features| !features.as_array().unwrap().contains(&insecure)),
            "production dependency must not enable insecure bidders"
        );
        assert!(
            manifest["features"].get("default").is_none(),
            "production default features must stay unchanged"
        );
    }

    fn assert_independent_budget(
        derived: &Derived,
        limits: &ResolvedIsolateLimits,
        costs: RequestConstructionCosts,
        baked_bytes: u64,
    ) -> u64 {
        // Complete equation at the independently pinned selected caps. No old
        // golden or production-derived term supplies the expected phase values.
        const WIRE: u64 = 3 * 0x8000 + 8 * 4_096 + 189 + 23 * 2_048;
        const TREE: u64 = (2_048 + 30) * 384;
        const REQUEST_COPY: u64 = 2 * WIRE + TREE;
        const SUPPLEMENT: u64 = 2 * 0x8000 + 2_048 * 384;
        const FETCHED_CONFIG: u64 = 0x0002_0000 * 129 + 0x0001_0000 * 128 + 8_192 * 623;
        const INGRESS: u64 = 0x8000
            + 0x4000
            + 128 * 64
            + 4_096 * 5
            + WIRE * 36
            + TREE
            + WIRE * 8
            + TREE
            + 2 * REQUEST_COPY
            + 2 * (REQUEST_COPY + SUPPLEMENT);
        const OUTBOUND: u64 = 512 * 1_024 + 16 * (0x8000 * 2 + 4_192 + 7_680);
        const RESPONSE: u64 = 16
            * (0x8000 + 12_288 + 0x8000 * 256 + 2_048 * 320 + 0x8000 * 128 + 0x8000 * 3)
            + 4 * (0x8000 + 2_048 * 384)
            + 9_310_368;
        const FIXED_TOTAL: u64 = 27_328_512
            + 0x0700_0000
            + 2 * (21_258_240 + 0x4000 + 0x8000 + 42_656)
            + 2 * (1_572_864 * 2 + 4_096)
            + 2 * ((128 << 20) * 2 + 4_096)
            + (4 + 2 + 1) * 512_000
            + 384 * (55_424 + 0x0010_0000 + 55_424 + 1_732 * 64)
            + 384 * (256 * 2 + 4_096)
            + 8_192
            + 4 * (8_192 * 2 + 4_096)
            + 0x0100_0000
            + 0x0080_0000;
        assert_eq!(costs.retained_per_scope_bytes, 2 * 39);
        let config = baked_bytes
            .checked_mul(128)
            .and_then(|bytes| bytes.checked_add(FETCHED_CONFIG))
            .unwrap();
        let privacy = (14_u64 << 20_u32)
            .checked_add(costs.fixed_per_auction_bytes)
            .unwrap();
        let projection = (16 * (WIRE * 128 + 78))
            .checked_add(costs.reusable_scratch_bytes)
            .and_then(|bytes| bytes.checked_add(1_576_960))
            .and_then(|bytes| bytes.checked_add(0x0008_0000))
            .unwrap();
        let charges = derived.auction_charges;
        assert_eq!(charges.config, config);
        assert_eq!(charges.ingress, INGRESS);
        assert_eq!(charges.privacy, privacy);
        assert_eq!(charges.projection, projection);
        assert_eq!(charges.outbound, OUTBOUND);
        assert_eq!(charges.response, RESPONSE);
        let auction = [config, INGRESS, privacy, projection, OUTBOUND, RESPONSE]
            .into_iter()
            .try_fold(0_u64, u64::checked_add)
            .unwrap();
        let total = auction
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(FIXED_TOTAL))
            .unwrap();
        assert_eq!(derived.per_auction_peak, auction);
        assert_eq!(derived.total_bytes, total);
        assert_eq!(
            derived.margin_bytes,
            limits.isolate_total_bytes.checked_sub(total).unwrap()
        );
        total
    }

    fn assert_baked_config_funding(
        installed: &Derived,
        pure: &Derived,
        limits: &ResolvedIsolateLimits,
        baked_bytes: u64,
    ) {
        // Only installed config charges gain the independent baked allowance.
        assert!(installed.auction_charges.config >= pure.auction_charges.config);
        assert_eq!(installed.call_side_term, pure.call_side_term);
        let baked_allowance = baked_bytes
            .checked_mul(128)
            .expect("baked config allowance fits u64");
        assert_eq!(
            installed
                .auction_charges
                .config
                .checked_sub(pure.auction_charges.config)
                .expect("installed config charge contains the pure charge"),
            baked_allowance,
        );
        assert_eq!(
            installed
                .per_auction_peak
                .checked_sub(pure.per_auction_peak)
                .expect("installed auction peak contains the pure peak"),
            baked_allowance,
        );
        assert_eq!(
            installed
                .total_bytes
                .checked_sub(pure.total_bytes)
                .expect("installed total contains the pure total"),
            u64::from(limits.max_concurrent_auctions)
                .checked_mul(baked_allowance)
                .expect("baked config population allowance fits"),
        );
    }

    fn assert_auction_prepayment(derived: &Derived) {
        let budget = AuctionBudget::new(derived.per_auction_peak);
        let receipts = derived
            .auction_charges
            .prepay(&budget)
            .expect("the installed row prepays every auction phase");
        assert_eq!(budget.in_use(), derived.per_auction_peak);
        drop(receipts);
        assert_eq!(budget.in_use(), 0);
    }

    #[test]
    fn oversized_native_row_warns_once_and_still_installs() {
        const CHILD: &str = "AUCTIONEER__TEST__OVERSIZED_NATIVE_ROW";
        let mut profile = resource_profile();
        profile.limits.max_concurrent_auctions = 64;
        profile.limits.egress_permits = 64;
        let limits = profile.limits;
        let target = profile.target;
        let boot = profile.boot_envelope();
        if var_os(CHILD).is_some() {
            install_boot_logger(|| boot.log_sink);
            assert!(boot_limits().is_none());
            for _ in 0_u32..2 {
                install_boot_envelope(
                    &limits,
                    &target,
                    boot.outbound_meta_bytes,
                    boot.construction_costs,
                    boot.catalog_costs,
                )
                .expect("over-ceiling native rows warn and boot, not fail");
            }
            let gates = boot_gates().unwrap();
            assert_eq!(gates.limits, limits);
            assert!(gates.derived.total_bytes > limits.isolate_total_bytes);
            assert_eq!(gates.egress.capacity(), 64);
            return;
        }
        let baked_bytes = serialized_json_bytes(&(
            auctioneer_config_data::host_default(),
            auctioneer_config_data::account_default(),
        ))
        .unwrap();
        let derived = derive_isolate_budget_with_baked_config(
            &limits,
            &target,
            boot.outbound_meta_bytes,
            boot.construction_costs,
            baked_bytes,
            boot.catalog_costs,
        )
        .unwrap();
        assert!(derived.total_bytes > limits.isolate_total_bytes);
        let output = Command::new(current_exe().unwrap())
            .env_clear()
            .env(CHILD, "1")
            .args([
                "--exact",
                "tests::oversized_native_row_warns_once_and_still_installs",
                "--nocapture",
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(output.status.success(), "{stderr}");
        assert_eq!(
            stderr
                .matches("boot envelope over ceiling: requires")
                .count(),
            1
        );
        for text in [
            derived.total_bytes.to_string(),
            limits.isolate_total_bytes.to_string(),
            (derived.total_bytes - limits.isolate_total_bytes).to_string(),
            "Booting anyway".into(),
        ] {
            assert!(stderr.contains(&text), "{text}: {stderr}");
        }
    }

    #[test]
    fn the_shipped_envelope_installs_and_boot_limits_answers_some() {
        assert!(boot_limits().is_none(), "nothing has booted this process");
        let profile = resource_profile();
        let limits = profile.limits;
        let target = profile.target;
        let envelope = profile.boot_envelope();
        assert_eq!(envelope.catalog_costs.context_per_auction_bytes, 1_576_960);
        assert_eq!(envelope.catalog_costs.resident_bytes, 0x0100_0000);
        assert_eq!(envelope.catalog_costs.initialization_bytes, 0x0080_0000);
        assert_eq!(
            envelope.catalog_costs.schema_validation_per_auction_bytes,
            0x0008_0000
        );
        install_boot_envelope(
            envelope.limits,
            envelope.target,
            envelope.outbound_meta_bytes,
            envelope.construction_costs,
            envelope.catalog_costs,
        )
        .expect("the shipped candidate installs within its local budget");

        let installed = boot_limits().expect("installed boot limits");
        assert_eq!(*installed, limits);
        assert_installed_profile(installed, &target);

        let pure = derive_isolate_budget(
            &limits,
            &target,
            envelope.outbound_meta_bytes,
            envelope.construction_costs,
            envelope.catalog_costs,
        )
        .expect("the pure shipped row derives all funded terms");
        assert_independent_budget(&pure, &limits, envelope.construction_costs, 0);
        assert_eq!(
            pure.total_bytes,
            limits.baseline_bytes
                + u64::from(limits.max_concurrent_auctions) * pure.per_auction_peak
                + u64::from(limits.max_active_body_streams) * pure.per_stream_peak
                + pure.egress_term
                + pure.aux_term
                + pure.gvl_terms
                + pure.pre_admission_parser_term
                + pure.terminal_term
                + pure.probe_output_term
                + pure.catalog_resident_term
                + pure.catalog_initialization_term,
        );
        assert_eq!(pure.bookkeeping_term, 3_276_800);
        assert_eq!(pure.call_side_term, 215_868_928);
        let gates = boot_gates().expect("the envelope installs charges with its gates");
        assert_eq!(gates.construction_costs, envelope.construction_costs);
        assert_eq!(gates.rederive_budget().unwrap(), gates.derived);
        let baked_bytes = serialized_json_bytes(&(
            auctioneer_config_data::host_default(),
            auctioneer_config_data::account_default(),
        ))
        .unwrap();
        assert_eq!(baked_bytes, 1_592);
        let proof = derive_isolate_budget_with_baked_config(
            installed,
            &target,
            envelope.outbound_meta_bytes,
            envelope.construction_costs,
            baked_bytes,
            envelope.catalog_costs,
        )
        .unwrap();
        assert_eq!(gates.derived, proof);
        let independent_total = assert_independent_budget(
            &gates.derived,
            installed,
            envelope.construction_costs,
            baked_bytes,
        );
        assert_eq!(gates.target, target);
        assert_eq!(gates.derived.aux_term, pure.aux_term);
        assert_eq!(gates.derived.gvl_terms, pure.gvl_terms);
        assert!(gates.aux_work_derived.peak_bytes() <= installed.aux_route_peak_bytes);
        assert_eq!(gates.aux_work_derived.peak_bytes(), 58_647_552);
        assert_eq!(installed.aux_route_peak_bytes, 0x0800_0000);
        assert_eq!(
            installed
                .aux_route_peak_bytes
                .checked_sub(gates.aux_work_derived.peak_bytes())
                .unwrap(),
            75_570_176
        );
        assert_eq!(gates.derived.total_bytes, independent_total);
        assert!(gates.derived.total_bytes + (256_u64 << 20_u32) <= installed.isolate_total_bytes);
        assert_eq!(
            gates.aux_work_derived.pinned_gvl_bytes(),
            installed.gvl_parsed_bytes
        );
        assert_eq!(gates.aux_work_budget.in_use(), 0);
        assert_baked_config_funding(&gates.derived, &pure, installed, baked_bytes);
        assert_auction_prepayment(&gates.derived);
    }

    fn assert_installed_profile(
        installed: &ResolvedIsolateLimits,
        target: &TargetPlatformConstants,
    ) {
        assert_eq!(installed.isolate_total_bytes, 4_u64 << 30_u32);
        assert_eq!(installed.max_runtime_config_bytes, 128_u64 << 10_u32);
        assert_eq!(installed.max_runtime_config_blob_bytes, 64_u64 << 10_u32);
        assert_eq!(installed.max_parsed_nodes, 2_048);
        assert_eq!(installed.response_write_deadline_ms, 30_000);
        assert_eq!(installed.response_parse_factor, MIN_RESPONSE_PARSE_FACTOR);
        assert_eq!(installed.response_parse_factor, 256);
        assert_eq!(
            installed.response_node_peak_bytes,
            MIN_RESPONSE_NODE_PEAK_BYTES
        );
        assert_eq!(installed.response_node_peak_bytes, 320);
        // The gate capacities the envelope sizes.
        assert_eq!(installed.max_concurrent_auctions, 2);
        assert_eq!(installed.max_active_body_streams, 2);
        assert_eq!(installed.egress_permits, 2);
        assert_eq!(installed.aux_route_permits, 1);
        assert_eq!(installed.aux_route_peak_bytes, 128_u64 << 20_u32);
        assert_eq!(target.max_request_target_bytes, 33 * 1024);
        assert_eq!(target.h1_max_buf_size_bytes, 1_u32 << 16_u32);
        assert_eq!(installed.terminal_response_permits, 384);
    }
}
