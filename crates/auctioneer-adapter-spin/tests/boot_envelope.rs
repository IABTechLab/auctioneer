//! Compare the installed target row with the owning-layout proof.

#[cfg(test)]
mod tests {
    use auctioneer_adapter_spin::envelope;
    use auctioneer_core::host_primitives::{boot_gates, install_boot_envelope};
    use auctioneer_runtime::budget::TYPED_CLONE_FACTOR;
    use auctioneer_runtime::{
        AuctionBudget, CatalogResourceCosts, Derived, RequestConstructionCosts,
        ResolvedIsolateLimits, derive_isolate_budget_with_baked_config, serialized_json_bytes,
    };

    fn assert_independent_budget(
        derived: &Derived,
        limits: &ResolvedIsolateLimits,
        costs: RequestConstructionCosts,
        baked_bytes: u64,
        catalog_costs: CatalogResourceCosts,
    ) {
        // Stage-2 user sync's golden already funds auxiliary work and its pinned GVLs.
        // privacy enforcement adds 2B + 8H + 189 + 23N over the old raw-body wire allowance.
        let enrichment = limits
            .inbound_request_bytes
            .checked_mul(2)
            .and_then(|bytes| {
                let headers = u64::from(limits.max_copied_header_bytes).checked_mul(8)?;
                bytes.checked_add(headers)
            })
            .and_then(|bytes| bytes.checked_add(189))
            .expect("normalization enrichment fits u64");
        let wire_delta = 23_u64
            .checked_mul(u64::from(limits.max_parsed_nodes))
            .and_then(|bytes| bytes.checked_add(enrichment))
            .expect("numeric and enrichment wire growth fits u64");
        let ingress_multiplier = limits
            .inbound_parse_factor
            .checked_add(8 + 4 + 4)
            .expect("normalized copy multiplier fits u64");
        let implicit_nodes = 30_u64
            .checked_mul(384)
            .and_then(|bytes| bytes.checked_mul(6))
            .expect("implicit node storage fits u64");
        let ingress_delta = wire_delta
            .checked_mul(ingress_multiplier)
            .and_then(|bytes| bytes.checked_add(implicit_nodes))
            .expect("normalization ingress growth fits u64");
        let projection_delta = wire_delta
            .checked_mul(TYPED_CLONE_FACTOR)
            .and_then(|bytes| bytes.checked_mul(u64::from(limits.max_bidders_per_auction)))
            .expect("normalization projection growth fits u64");
        // Two 39-byte canonical address destinations per survivor scope.
        assert_eq!(costs.retained_per_scope_bytes, 2 * 39);
        let retained_scopes = 78_u64
            .checked_mul(u64::from(limits.max_bidders_per_auction))
            .expect("retained scope destinations fit u64");
        let fixed_and_scratch = costs
            .fixed_per_auction_bytes
            .checked_add(costs.reusable_scratch_bytes)
            .expect("construction layout charge fits u64");
        let baked_allowance = baked_bytes
            .checked_mul(128)
            .expect("baked config allowance fits u64");
        let delta = ingress_delta
            .checked_add(projection_delta)
            .and_then(|bytes| bytes.checked_add(retained_scopes))
            .and_then(|bytes| bytes.checked_add(fixed_and_scratch))
            .and_then(|bytes| bytes.checked_add(baked_allowance))
            .and_then(|bytes| bytes.checked_add(9_704_608))
            // Existing sent-impression indices and correlation table, per call.
            .and_then(|bytes| bytes.checked_add(32 * (100 * 8 + 64)))
            .and_then(|bytes| bytes.checked_add(catalog_costs.schema_validation_per_auction_bytes))
            .and_then(|bytes| bytes.checked_add(catalog_costs.context_per_auction_bytes))
            .expect("independent per-auction delta fits u64");
        assert_eq!(
            derived.per_auction_peak,
            6_258_324_480_u64
                .checked_add(delta)
                .expect("independent auction peak fits u64")
        );
        assert_eq!(
            derived.total_bytes,
            10_259_037_696_u64
                .checked_add(
                    u64::from(limits.max_concurrent_auctions)
                        .checked_mul(delta)
                        .expect("concurrent auction deltas fit u64"),
                )
                .and_then(|bytes| bytes.checked_add(catalog_costs.resident_bytes))
                .and_then(|bytes| bytes.checked_add(catalog_costs.initialization_bytes))
                .expect("independent target total fits u64")
        );
    }

    #[test]
    fn construction_costs_match_the_installed_and_diagnostic_target_derivation() {
        let profile = envelope::resource_profile();
        let target = profile.target;
        let boot = profile.boot_envelope();
        install_boot_envelope(
            boot.limits,
            boot.target,
            boot.outbound_meta_bytes,
            boot.construction_costs,
            boot.catalog_costs,
        )
        .unwrap();
        let gates = boot_gates().unwrap();
        let baked = serialized_json_bytes(&(
            auctioneer_config_data::host_default(),
            auctioneer_config_data::account_default(),
        ))
        .unwrap();
        let proof = derive_isolate_budget_with_baked_config(
            &gates.limits,
            &target,
            boot.outbound_meta_bytes,
            boot.construction_costs,
            baked,
            profile.catalog_costs,
        )
        .unwrap();
        assert_eq!(gates.construction_costs, boot.construction_costs);
        assert_eq!(gates.catalog_costs, profile.catalog_costs);
        assert_eq!(
            gates.derived.catalog_context_term,
            profile.catalog_costs.context_per_auction_bytes,
        );
        assert!(gates.derived.catalog_context_term > 0);
        assert_eq!(gates.derived, proof);
        assert_independent_budget(
            &gates.derived,
            &gates.limits,
            boot.construction_costs,
            baked,
            profile.catalog_costs,
        );
        assert_eq!(gates.rederive_budget().unwrap(), proof);
        let budget = AuctionBudget::new(proof.per_auction_peak);
        let receipts = gates.derived.auction_charges.prepay(&budget).unwrap();
        assert_eq!(budget.in_use(), proof.per_auction_peak);
        drop(receipts);
        assert_eq!(budget.in_use(), 0);
    }
}
