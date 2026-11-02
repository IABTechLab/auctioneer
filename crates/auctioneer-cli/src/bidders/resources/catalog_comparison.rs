//! Pure catalog-baseline arithmetic, not artifact qualification or readiness.

use super::{
    AdapterId, AdapterRow, Error, Fact, PlatformFacts, catalog_allowances, check_named_terms,
};
use auctioneer_core::host_primitives::resolve_budget_inputs;
use auctioneer_runtime::{
    HeadroomClass, IsolateTerm, PROBE_CONFIG_MIN_PERMITS, ResolvedIsolateLimits,
    derive_isolate_budget_with_baked_config, minimum_headroom_bytes,
};
use serde::Serialize;
use std::num::NonZeroU64;

#[derive(Debug, Serialize)]
pub(super) struct ComparisonRow {
    pub adapter: AdapterId,
    pub baseline_total_bytes: u64,
    pub candidate_total_bytes: u64,
    pub total_delta_bytes: u64,
    pub total_delta_allowance_bytes: u64,
    pub growth: Growth,
    pub native_arithmetic_fit: Option<bool>,
    pub absolute_required_fit: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct Growth {
    #[serde(rename = "catalog_resident_bytes")]
    pub catalog_resident: u64,
    #[serde(rename = "catalog_initialization_bytes")]
    pub catalog_initialization: u64,
    #[serde(rename = "schema_validation_per_auction_bytes")]
    pub schema_validation_per_auction: u64,
    #[serde(rename = "context_per_auction_bytes")]
    pub context_per_auction: u64,
    #[serde(rename = "aux_route_peak_bytes")]
    pub aux_route_peak: u64,
    #[serde(rename = "auction_isolate_bytes")]
    pub auction_isolate: u64,
    #[serde(rename = "auxiliary_isolate_bytes")]
    pub auxiliary_isolate: u64,
}

pub(super) fn compare(baseline: &AdapterRow, current: &AdapterRow) -> Result<ComparisonRow, Error> {
    check_matched_inputs(baseline, current)?;
    catalog_allowances::check_allowances(current.catalog_costs)?;
    check_capture(baseline)?;
    check_capture(current)?;
    let growth = growth_between(baseline, current)?;
    check_isolate_growth(baseline, current, &growth)?;
    let expected_delta = [
        growth.catalog_resident,
        growth.catalog_initialization,
        growth.auction_isolate,
        growth.auxiliary_isolate,
    ]
    .into_iter()
    .try_fold(0_u64, |total, bytes| {
        total.checked_add(bytes).ok_or(Error::ResourceInput)
    })?;
    let total_delta_bytes = current
        .derived
        .total_bytes
        .checked_sub(baseline.derived.total_bytes)
        .ok_or(Error::ResourceInput)?;
    let total_delta_allowance_bytes = total_allowance(current.adapter);
    if total_delta_bytes != expected_delta || total_delta_bytes > total_delta_allowance_bytes {
        return Err(Error::ResourceInput);
    }
    Ok(ComparisonRow {
        adapter: current.adapter,
        baseline_total_bytes: baseline.derived.total_bytes,
        candidate_total_bytes: current.derived.total_bytes,
        total_delta_bytes,
        total_delta_allowance_bytes,
        growth,
        native_arithmetic_fit: native_arithmetic_fit(current)?,
        absolute_required_fit: current.required_fit,
    })
}

fn check_matched_inputs(baseline: &AdapterRow, current: &AdapterRow) -> Result<(), Error> {
    let previous = &baseline.identity;
    let candidate = &current.identity;
    if previous.sdk_revision != candidate.sdk_revision
        || previous.lock_sha256 != candidate.lock_sha256
        || previous.features != candidate.features
        || previous.declared_toolchain != candidate.declared_toolchain
        || previous.compiler_verbose != candidate.compiler_verbose
        || previous.compiled_target != candidate.compiled_target
        || previous.reporter_abi != candidate.reporter_abi
        || previous.target_abi != candidate.target_abi
        || previous.layout_evidence != candidate.layout_evidence
        || baseline.adapter != current.adapter
        || baseline.platform_profile != current.platform_profile
        || baseline.dispatch_profile != current.dispatch_profile
        || baseline.memory_scope != current.memory_scope
        || !same_facts(&baseline.platform_facts, &current.platform_facts)?
        || !same_facts(&baseline.dispatch_facts, &current.dispatch_facts)?
        || baseline.construction_costs != current.construction_costs
        || baseline.outbound_meta_bytes != current.outbound_meta_bytes
        || baseline.baked_config_bytes != current.baked_config_bytes
        || baseline.baked_config_sha256 != current.baked_config_sha256
        || baseline.actual_gvl_cache_entries != current.actual_gvl_cache_entries
        || baseline.target_constants != current.target_constants
        || !same_workload(&baseline.raw_limits, &current.raw_limits)
        || !same_workload(&baseline.effective_limits, &current.effective_limits)
    {
        return Err(Error::ResourceInput);
    }
    Ok(())
}

fn same_facts(baseline: &PlatformFacts, current: &PlatformFacts) -> Result<bool, Error> {
    Ok(
        serde_json::to_value(baseline).map_err(|_error| Error::ResourceInput)?
            == serde_json::to_value(current).map_err(|_error| Error::ResourceInput)?,
    )
}

fn same_workload(baseline: &ResolvedIsolateLimits, current: &ResolvedIsolateLimits) -> bool {
    let mut candidate = *current;
    candidate.aux_route_peak_bytes = baseline.aux_route_peak_bytes;
    *baseline == candidate
}

fn check_capture(row: &AdapterRow) -> Result<(), Error> {
    let inputs = resolve_budget_inputs(&row.raw_limits)?;
    let derived = derive_isolate_budget_with_baked_config(
        &row.effective_limits,
        &row.target_constants,
        row.outbound_meta_bytes,
        row.construction_costs,
        row.baked_config_bytes,
        row.catalog_costs,
    )
    .map_err(|_error| Error::ResourceInput)?;
    check_named_terms(derived.isolate_terms, &row.terms)?;
    if inputs.limits != row.effective_limits
        || derived != row.derived
        || derived.auction_charges != row.auction_receipts
    {
        return Err(Error::ResourceInput);
    }
    Ok(())
}

fn growth_between(baseline: &AdapterRow, current: &AdapterRow) -> Result<Growth, Error> {
    let previous = baseline.catalog_costs;
    let candidate = current.catalog_costs;
    let catalog_resident_bytes = candidate
        .resident_bytes
        .checked_sub(previous.resident_bytes)
        .ok_or(Error::ResourceInput)?;
    let catalog_initialization_bytes = candidate
        .initialization_bytes
        .checked_sub(previous.initialization_bytes)
        .ok_or(Error::ResourceInput)?;
    let schema_validation_per_auction_bytes = candidate
        .schema_validation_per_auction_bytes
        .checked_sub(previous.schema_validation_per_auction_bytes)
        .ok_or(Error::ResourceInput)?;
    let context_per_auction_bytes = candidate
        .context_per_auction_bytes
        .checked_sub(previous.context_per_auction_bytes)
        .ok_or(Error::ResourceInput)?;
    let limits = &current.effective_limits;
    let aux_route_peak_bytes = limits
        .aux_route_peak_bytes
        .checked_sub(baseline.effective_limits.aux_route_peak_bytes)
        .ok_or(Error::ResourceInput)?;
    if aux_route_peak_bytes > 0x0080_0000 {
        return Err(Error::ResourceInput);
    }
    let auction_isolate_bytes = schema_validation_per_auction_bytes
        .checked_add(context_per_auction_bytes)
        .and_then(|bytes| bytes.checked_mul(u64::from(limits.max_concurrent_auctions)))
        .ok_or(Error::ResourceInput)?;
    let auxiliary_population = u64::from(limits.aux_route_permits)
        .checked_add(u64::from(
            limits.aux_route_permits.max(PROBE_CONFIG_MIN_PERMITS),
        ))
        .ok_or(Error::ResourceInput)?;
    let auxiliary_isolate_bytes = aux_route_peak_bytes
        .checked_mul(u64::from(current.target_constants.egress_handoff_factor))
        .and_then(|bytes| bytes.checked_mul(auxiliary_population))
        .ok_or(Error::ResourceInput)?;
    Ok(Growth {
        catalog_resident: catalog_resident_bytes,
        catalog_initialization: catalog_initialization_bytes,
        schema_validation_per_auction: schema_validation_per_auction_bytes,
        context_per_auction: context_per_auction_bytes,
        aux_route_peak: aux_route_peak_bytes,
        auction_isolate: auction_isolate_bytes,
        auxiliary_isolate: auxiliary_isolate_bytes,
    })
}

fn check_isolate_growth(
    baseline: &AdapterRow,
    current: &AdapterRow,
    growth: &Growth,
) -> Result<(), Error> {
    let previous = baseline.derived.isolate_terms.entries();
    let candidate = current.derived.isolate_terms.entries();
    for ((term, old_bytes), (current_term, new_bytes)) in previous.into_iter().zip(candidate) {
        let expected_delta = match term {
            IsolateTerm::Auctions => growth.auction_isolate,
            IsolateTerm::Auxiliary => growth.auxiliary_isolate,
            IsolateTerm::CatalogResident => growth.catalog_resident,
            IsolateTerm::CatalogInitialization => growth.catalog_initialization,
            IsolateTerm::Baseline
            | IsolateTerm::Gvl
            | IsolateTerm::AuctionEgress
            | IsolateTerm::TerminalEgress
            | IsolateTerm::ProbeOutput
            | IsolateTerm::PreAdmissionParser
            | IsolateTerm::BodyStreams => 0,
        };
        if term != current_term || new_bytes.checked_sub(old_bytes) != Some(expected_delta) {
            return Err(Error::ResourceInput);
        }
    }
    Ok(())
}

const fn total_allowance(adapter: AdapterId) -> u64 {
    match adapter {
        AdapterId::Axum => 0x0600_0000,
        AdapterId::Cloudflare => 0x0800_0000,
        AdapterId::Fastly | AdapterId::Spin => 0x0400_0000,
    }
}

fn native_arithmetic_fit(row: &AdapterRow) -> Result<Option<bool>, Error> {
    if row.adapter.class() != HeadroomClass::Native {
        return Ok(None);
    }
    let configured = row.effective_limits.isolate_total_bytes;
    let ceiling = match &row.platform_facts.ceiling {
        Fact::Known { value, .. } => {
            if value.scope != row.memory_scope {
                return Err(Error::ResourceInput);
            }
            configured.min(value.primary_bytes)
        }
        Fact::Pending { .. } => configured,
    };
    let cap = NonZeroU64::new(ceiling).ok_or(Error::ResourceInput)?;
    let required = row
        .derived
        .total_bytes
        .checked_add(minimum_headroom_bytes(cap, HeadroomClass::Native))
        .ok_or(Error::ResourceInput)?;
    if required > ceiling {
        return Err(Error::ResourceNativeFit);
    }
    Ok(Some(true))
}

#[cfg(test)]
mod tests {
    use super::{AdapterId, AdapterRow, Error, compare};
    use crate::bidders::resources::{
        Ceiling, Fact, Identity, LayoutEvidence, baked_identity, profile_digest, row,
    };
    use auctioneer_core::host_primitives::{ResourceProfile, resolve_budget_inputs};
    use auctioneer_runtime::{CatalogResourceCosts, derive_isolate_budget_with_baked_config};
    use serde_json::Value;

    fn identity() -> Identity {
        Identity {
            code_base_commit: "baseline-commit".to_owned(),
            dirty: false,
            source_tree_matches_commit: true,
            sdk_revision: "matched-sdk".to_owned(),
            lock_sha256: "matched-lock".to_owned(),
            declared_toolchain: "matched-declared-tools".to_owned(),
            features: vec!["application".to_owned()],
            source_sha256: "baseline-source".to_owned(),
            source_file_count: 17,
            compiler_verbose: "matched-compiler".to_owned(),
            compiled_target: "matched-compiled-target".to_owned(),
            reporter_abi: "matched-reporter-abi".to_owned(),
            target_abi: "overridden-by-profile".to_owned(),
            layout_evidence: LayoutEvidence::NativeControl,
            artifact_proof: "pending_not_a_deployment_observation".to_owned(),
        }
    }

    fn profile(adapter: AdapterId, auctions: u32, auxiliary: u32) -> ResourceProfile {
        let mut result = adapter.profile();
        result.limits.max_concurrent_auctions = auctions;
        result.limits.aux_route_permits = auxiliary;
        result.catalog_costs = CatalogResourceCosts::UNCHANGED;
        result
    }

    fn capture(adapter: AdapterId, profile: &ResourceProfile) -> AdapterRow {
        let (_bytes, digest) = baked_identity().unwrap();
        row(adapter, profile, &identity(), &digest).unwrap()
    }

    fn pair(adapter: AdapterId) -> (AdapterRow, AdapterRow) {
        let profile = profile(adapter, 3, 1);
        (capture(adapter, &profile), capture(adapter, &profile))
    }

    fn rederive(row: &mut AdapterRow) {
        row.effective_limits = resolve_budget_inputs(&row.raw_limits).unwrap().limits;
        row.derived = derive_isolate_budget_with_baked_config(
            &row.effective_limits,
            &row.target_constants,
            row.outbound_meta_bytes,
            row.construction_costs,
            row.baked_config_bytes,
            row.catalog_costs,
        )
        .unwrap();
        row.auction_receipts = row.derived.auction_charges;
        row.terms = row
            .derived
            .isolate_terms
            .entries()
            .into_iter()
            .map(|(term, bytes)| (term.name().to_owned(), bytes))
            .collect();
        seal(row);
    }

    fn seal(row: &mut AdapterRow) {
        row.effective_profile_sha256 = profile_digest(row).unwrap();
    }

    fn refused(baseline: &AdapterRow, current: &AdapterRow) {
        assert!(matches!(
            compare(baseline, current),
            Err(Error::ResourceInput)
        ));
    }

    fn edit(row: &mut AdapterRow, path: &[&str], replacement: Value) {
        let mut value = serde_json::to_value(&*row).unwrap();
        let mut selected = &mut value;
        for field in path {
            selected = selected.get_mut(*field).unwrap();
        }
        *selected = replacement;
        *row = serde_json::from_value(value).unwrap();
    }

    #[test]
    fn unchanged_rows_preserve_absolute_failure_without_qualification() {
        for adapter in AdapterId::ALL {
            let (baseline, current) = pair(adapter);
            let comparison = compare(&baseline, &current).unwrap();
            assert_eq!(comparison.adapter, adapter);
            assert_eq!(
                comparison.baseline_total_bytes,
                baseline.derived.total_bytes
            );
            assert_eq!(
                comparison.candidate_total_bytes,
                current.derived.total_bytes
            );
            assert_eq!(comparison.total_delta_bytes, 0);
            assert!(!comparison.absolute_required_fit);
            let value = serde_json::to_value(comparison).unwrap();
            assert_eq!(value.as_object().unwrap().len(), 8);
            assert!(value.get("qualification").is_none());
            assert!(value.get("status").is_none());
        }
    }

    #[test]
    fn unequal_owner_deltas_are_disjoint_and_population_scaled() {
        let mut profile = profile(AdapterId::Axum, 3, 1);
        profile.catalog_costs = CatalogResourceCosts {
            resident_bytes: 101,
            initialization_bytes: 211,
            schema_validation_per_auction_bytes: 307,
            context_per_auction_bytes: 401,
        };
        let baseline = capture(AdapterId::Axum, &profile);
        profile.catalog_costs = CatalogResourceCosts {
            resident_bytes: 124,
            initialization_bytes: 240,
            schema_validation_per_auction_bytes: 338,
            context_per_auction_bytes: 438,
        };
        let current = capture(AdapterId::Axum, &profile);
        let comparison = compare(&baseline, &current).unwrap();
        assert_eq!(comparison.growth.catalog_resident, 23);
        assert_eq!(comparison.growth.catalog_initialization, 29);
        assert_eq!(comparison.growth.schema_validation_per_auction, 31);
        assert_eq!(comparison.growth.context_per_auction, 37);
        assert_eq!(comparison.growth.aux_route_peak, 0);
        assert_eq!(comparison.growth.auction_isolate, 204);
        assert_eq!(comparison.growth.auxiliary_isolate, 0);
        assert_eq!(comparison.total_delta_bytes, 256);
        assert_eq!(
            serde_json::to_value(&comparison.growth).unwrap(),
            serde_json::json!({
                "catalog_resident_bytes": 23_u64,
                "catalog_initialization_bytes": 29_u64,
                "schema_validation_per_auction_bytes": 31_u64,
                "context_per_auction_bytes": 37_u64,
                "aux_route_peak_bytes": 0_u64,
                "auction_isolate_bytes": 204_u64,
                "auxiliary_isolate_bytes": 0_u64,
            }),
        );
    }

    #[test]
    fn initialization_alone_is_added_even_under_dominant_auction_demand() {
        let (baseline, mut current) = pair(AdapterId::Axum);
        current.catalog_costs.initialization_bytes = 1;
        rederive(&mut current);
        assert_eq!(compare(&baseline, &current).unwrap().total_delta_bytes, 1);
    }

    #[test]
    fn all_owner_and_auxiliary_increments_are_added_together() {
        let (baseline, mut current) = pair(AdapterId::Axum);
        current.catalog_costs = CatalogResourceCosts {
            resident_bytes: 23,
            initialization_bytes: 29,
            schema_validation_per_auction_bytes: 31,
            context_per_auction_bytes: 37,
        };
        current.raw_limits.aux_route_peak_bytes = current
            .raw_limits
            .aux_route_peak_bytes
            .checked_add(113)
            .unwrap();
        rederive(&mut current);
        let comparison = compare(&baseline, &current).unwrap();
        assert_eq!(comparison.growth.auction_isolate, 204);
        assert_eq!(comparison.growth.auxiliary_isolate, 452);
        assert_eq!(comparison.total_delta_bytes, 708);
    }

    #[test]
    fn auxiliary_growth_includes_independent_probe_config_population() {
        let cases = [
            (AdapterId::Axum, 0, 226),
            (AdapterId::Axum, 1, 452),
            (AdapterId::Axum, 3, 1_356),
            (AdapterId::Cloudflare, 0, 339),
            (AdapterId::Cloudflare, 1, 678),
            (AdapterId::Cloudflare, 3, 2_034),
        ];
        for (adapter, permits, expected) in cases {
            // Keep native headroom funded while varying only auxiliary population.
            let profile = profile(adapter, 1, permits);
            let baseline = capture(adapter, &profile);
            let mut current = capture(adapter, &profile);
            current.raw_limits.aux_route_peak_bytes = current
                .raw_limits
                .aux_route_peak_bytes
                .checked_add(113)
                .unwrap();
            rederive(&mut current);
            let comparison = compare(&baseline, &current).unwrap();
            assert_eq!(comparison.growth.aux_route_peak, 113);
            assert_eq!(comparison.growth.auxiliary_isolate, expected);
            assert_eq!(comparison.total_delta_bytes, expected);
        }
    }

    #[test]
    fn introduced_auxiliary_peak_has_its_own_inclusive_ceiling() {
        let profile = profile(AdapterId::Axum, 3, 0);
        let baseline = capture(AdapterId::Axum, &profile);
        let mut current = capture(AdapterId::Axum, &profile);
        current.raw_limits.aux_route_peak_bytes = baseline
            .raw_limits
            .aux_route_peak_bytes
            .checked_add(0x0080_0000)
            .unwrap();
        rederive(&mut current);
        let comparison = compare(&baseline, &current).unwrap();
        assert_eq!(comparison.growth.aux_route_peak, 0x0080_0000);
        assert_eq!(comparison.total_delta_bytes, 0x0100_0000);
        current.raw_limits.aux_route_peak_bytes = current
            .raw_limits
            .aux_route_peak_bytes
            .checked_add(1)
            .unwrap();
        rederive(&mut current);
        refused(&baseline, &current);
    }

    #[test]
    fn each_candidate_component_has_an_independent_inclusive_ceiling() {
        for (field, ceiling, expected_delta) in [
            ("resident_bytes", 0x0100_0000_u64, 0x0100_0000_u64),
            ("initialization_bytes", 0x0080_0000, 0x0080_0000),
            (
                "schema_validation_per_auction_bytes",
                0x0008_0000,
                0x0018_0000,
            ),
        ] {
            let (baseline, mut current) = pair(AdapterId::Axum);
            edit(&mut current, &["catalog_costs", field], ceiling.into());
            rederive(&mut current);
            let comparison = compare(&baseline, &current).unwrap();
            assert_eq!(comparison.total_delta_bytes, expected_delta);
            edit(
                &mut current,
                &["catalog_costs", field],
                ceiling.checked_add(1).unwrap().into(),
            );
            rederive(&mut current);
            refused(&baseline, &current);
        }
    }

    #[test]
    fn component_ceiling_applies_to_candidate_not_only_increment() {
        let (mut baseline, mut current) = pair(AdapterId::Axum);
        baseline.catalog_costs.resident_bytes = 0x0100_0000;
        rederive(&mut baseline);
        current.catalog_costs.resident_bytes = 0x0100_0001;
        rederive(&mut current);
        refused(&baseline, &current);
    }

    #[test]
    fn complete_context_is_not_capped_at_the_settings_only_allowance() {
        let (baseline, mut current) = pair(AdapterId::Axum);
        current.catalog_costs.context_per_auction_bytes = 0x0010_0001;
        rederive(&mut current);
        let comparison = compare(&baseline, &current).unwrap();
        assert_eq!(comparison.growth.context_per_auction, 0x0010_0001);
        assert_eq!(comparison.total_delta_bytes, 0x0030_0003);
    }

    #[test]
    fn each_target_total_allowance_is_inclusive_and_independent() {
        for (adapter, allowance) in [
            (AdapterId::Axum, 0x0600_0000_u64),
            (AdapterId::Cloudflare, 0x0800_0000),
            (AdapterId::Fastly, 0x0400_0000),
            (AdapterId::Spin, 0x0400_0000),
        ] {
            let profile = profile(adapter, 1, 0);
            let baseline = capture(adapter, &profile);
            let mut current = capture(adapter, &profile);
            current.catalog_costs.context_per_auction_bytes = allowance;
            rederive(&mut current);
            let comparison = compare(&baseline, &current).unwrap();
            assert_eq!(comparison.total_delta_allowance_bytes, allowance);
            assert_eq!(comparison.total_delta_bytes, allowance);
            current.catalog_costs.context_per_auction_bytes = allowance.checked_add(1).unwrap();
            rederive(&mut current);
            refused(&baseline, &current);
        }
    }

    #[test]
    fn owner_reductions_cannot_be_offset_by_growth_in_another_owner() {
        for field in [
            "resident_bytes",
            "initialization_bytes",
            "schema_validation_per_auction_bytes",
            "context_per_auction_bytes",
        ] {
            let (mut baseline, mut current) = pair(AdapterId::Axum);
            baseline.catalog_costs = CatalogResourceCosts {
                resident_bytes: 101,
                initialization_bytes: 211,
                schema_validation_per_auction_bytes: 307,
                context_per_auction_bytes: 401,
            };
            current.catalog_costs = baseline.catalog_costs;
            current.catalog_costs.context_per_auction_bytes = 501;
            rederive(&mut baseline);
            edit(&mut current, &["catalog_costs", field], 0_u64.into());
            rederive(&mut current);
            refused(&baseline, &current);
        }
    }

    #[test]
    fn auxiliary_peak_reduction_cannot_be_offset_by_catalog_growth() {
        let (mut baseline, mut current) = pair(AdapterId::Axum);
        baseline.raw_limits.aux_route_peak_bytes = baseline
            .raw_limits
            .aux_route_peak_bytes
            .checked_add(1)
            .unwrap();
        rederive(&mut baseline);
        current.catalog_costs.resident_bytes = 101;
        rederive(&mut current);
        refused(&baseline, &current);
    }

    #[test]
    fn source_capture_identity_changes_are_allowed_without_rewriting_baseline() {
        let (baseline, mut current) = pair(AdapterId::Axum);
        let before = serde_json::to_value(&baseline).unwrap();
        current.identity.code_base_commit = "candidate-commit".to_owned();
        current.identity.source_sha256 = "candidate-source".to_owned();
        current.identity.source_file_count = 23;
        current.identity.dirty = true;
        current.identity.source_tree_matches_commit = false;
        seal(&mut current);
        let comparison = compare(&baseline, &current).unwrap();
        assert_eq!(comparison.total_delta_bytes, 0_u64);
        assert_eq!(serde_json::to_value(&baseline).unwrap(), before);
    }

    #[test]
    fn every_non_code_build_identity_mismatch_refuses() {
        for field in [
            "sdk_revision",
            "lock_sha256",
            "declared_toolchain",
            "compiler_verbose",
            "compiled_target",
            "reporter_abi",
            "target_abi",
        ] {
            let (baseline, mut current) = pair(AdapterId::Axum);
            edit(&mut current, &["identity", field], "different".into());
            seal(&mut current);
            refused(&baseline, &current);
        }
        let (baseline, mut current) = pair(AdapterId::Axum);
        current.identity.features.push("different".to_owned());
        seal(&mut current);
        refused(&baseline, &current);
        let (layout_baseline, mut layout_current) = pair(AdapterId::Axum);
        layout_current.identity.layout_evidence = LayoutEvidence::HostNativeSubstitute;
        seal(&mut layout_current);
        refused(&layout_baseline, &layout_current);
    }

    #[test]
    fn adapter_platform_dispatch_and_scope_must_match_exactly() {
        for field in ["platform_profile", "dispatch_profile", "memory_scope"] {
            let (baseline, mut current) = pair(AdapterId::Axum);
            edit(&mut current, &[field], "different".into());
            seal(&mut current);
            refused(&baseline, &current);
        }
        let (baseline, mut current) = pair(AdapterId::Axum);
        current.adapter = AdapterId::Fastly;
        seal(&mut current);
        refused(&baseline, &current);
    }

    #[test]
    fn platform_and_dispatch_facts_include_provenance_and_pending_reasons() {
        for facts in ["platform_facts", "dispatch_facts"] {
            for field in ["ceiling", "inbound_population", "host_ingress_accounting"] {
                let (baseline, mut current) = pair(AdapterId::Axum);
                let mut replacement = serde_json::Map::new();
                replacement.insert("status".to_owned(), "pending".into());
                replacement.insert("reason".to_owned(), "different".into());
                edit(&mut current, &[facts, field], Value::Object(replacement));
                seal(&mut current);
                refused(&baseline, &current);
            }
        }
    }

    #[test]
    fn unrelated_workload_changes_refuse_even_when_they_reduce_demand() {
        for (field, replacement) in [
            ("baseline_bytes", 1_u64),
            ("max_concurrent_auctions", 1),
            ("aux_route_permits", 0),
            ("max_active_body_streams", 1),
            ("response_write_deadline_ms", 119_999),
        ] {
            let (baseline, mut current) = pair(AdapterId::Axum);
            edit(&mut current, &["raw_limits", field], replacement.into());
            rederive(&mut current);
            refused(&baseline, &current);
        }
    }

    #[test]
    fn unrelated_reduction_cannot_hide_a_component_overrun() {
        let (baseline, mut current) = pair(AdapterId::Axum);
        current.raw_limits.baseline_bytes = 1;
        current.catalog_costs.resident_bytes = 0x0100_0001;
        rederive(&mut current);
        assert!(current.derived.total_bytes < baseline.derived.total_bytes);
        refused(&baseline, &current);
    }

    #[test]
    fn matching_effective_limits_do_not_hide_different_raw_inputs() {
        let (baseline, mut current) = pair(AdapterId::Axum);
        current.raw_limits.max_config_json_nodes = 0;
        rederive(&mut current);
        assert_eq!(baseline.effective_limits, current.effective_limits);
        refused(&baseline, &current);
    }

    #[test]
    fn each_row_must_resolve_its_own_raw_inputs() {
        for forge_baseline in [false, true] {
            let (mut baseline, mut current) = pair(AdapterId::Axum);
            let forged = if forge_baseline {
                &mut baseline
            } else {
                &mut current
            };
            // Matching permits auxiliary-peak growth. Leave effective inputs
            // and derivation intact so only this row's normalization can refuse.
            forged.raw_limits.aux_route_peak_bytes = forged
                .raw_limits
                .aux_route_peak_bytes
                .checked_add(1)
                .unwrap();
            seal(forged);
            refused(&baseline, &current);
        }
    }

    #[test]
    fn construction_metadata_baked_and_cache_changes_refuse() {
        let cases = [
            (
                vec!["construction_costs", "fixed_per_auction_bytes"],
                1_u64.into(),
            ),
            (
                vec!["construction_costs", "reusable_scratch_bytes"],
                1_u64.into(),
            ),
            (
                vec!["construction_costs", "retained_per_scope_bytes"],
                1_u64.into(),
            ),
            (vec!["outbound_meta_bytes"], 1_u32.into()),
            (vec!["baked_config_bytes"], 1_u64.into()),
            (vec!["baked_config_sha256"], "different".into()),
            (vec!["actual_gvl_cache_entries"], 0_usize.into()),
            (
                vec!["target_constants", "egress_handoff_factor"],
                3_u32.into(),
            ),
            (
                vec!["target_constants", "platform_concurrent_ceiling"],
                1_u32.into(),
            ),
        ];
        for (path, replacement) in cases {
            let (baseline, mut current) = pair(AdapterId::Axum);
            edit(&mut current, &path, replacement);
            seal(&mut current);
            refused(&baseline, &current);
        }
    }

    #[test]
    fn stale_or_forged_derived_fields_refuse_on_either_side() {
        for forge_baseline in [false, true] {
            for field in [
                "runtime_config_term",
                "request_side_term",
                "call_side_term",
                "bookkeeping_term",
                "debug_term",
                "assembly_scratch_bytes",
                "per_auction_peak",
                "per_stream_peak",
                "decoder_chain_bytes",
                "gvl_terms",
                "egress_term",
                "aux_term",
                "terminal_term",
                "probe_output_term",
                "h2_slot_peak",
                "h1_slot_peak",
                "pre_admission_parser_term",
                "catalog_resident_term",
                "catalog_initialization_term",
                "schema_validation_term",
                "catalog_context_term",
                "total_bytes",
                "margin_bytes",
                "max_outbound_raw_bytes_usize",
            ] {
                let (mut baseline, mut current) = pair(AdapterId::Axum);
                let forged = if forge_baseline {
                    &mut baseline
                } else {
                    &mut current
                };
                edit(forged, &["derived", field], 1_u64.into());
                seal(forged);
                refused(&baseline, &current);
            }
        }
    }

    #[test]
    fn named_terms_and_receipts_are_validated_even_when_totals_are_unchanged() {
        for forge_baseline in [false, true] {
            for path in [
                vec!["terms", "baseline"],
                vec!["derived", "isolate_terms", "baseline"],
                vec!["auction_receipts", "config"],
                vec!["derived", "auction_charges", "config"],
                vec!["derived", "construction_costs", "fixed_per_auction_bytes"],
            ] {
                let (mut baseline, mut current) = pair(AdapterId::Axum);
                let forged = if forge_baseline {
                    &mut baseline
                } else {
                    &mut current
                };
                edit(forged, &path, 1_u64.into());
                seal(forged);
                refused(&baseline, &current);
            }
        }
    }

    #[test]
    fn matching_stale_coefficients_on_both_sides_still_refuse() {
        let (mut baseline, mut current) = pair(AdapterId::Axum);
        for captured in [&mut baseline, &mut current] {
            captured.derived.request_side_term = 1;
            seal(captured);
        }
        refused(&baseline, &current);
    }

    #[test]
    fn catalog_overflow_is_a_fixed_input_error() {
        for field in [
            "resident_bytes",
            "initialization_bytes",
            "schema_validation_per_auction_bytes",
            "context_per_auction_bytes",
        ] {
            let (baseline, mut current) = pair(AdapterId::Axum);
            edit(&mut current, &["catalog_costs", field], u64::MAX.into());
            seal(&mut current);
            refused(&baseline, &current);
        }
        let (baseline, mut current) = pair(AdapterId::Axum);
        current.raw_limits.aux_route_peak_bytes = u64::MAX;
        current.effective_limits.aux_route_peak_bytes = u64::MAX;
        seal(&mut current);
        refused(&baseline, &current);
    }

    fn native_pair(configured: u64, known: Option<u64>, demand: u64) -> (AdapterRow, AdapterRow) {
        let (mut baseline, mut current) = pair(AdapterId::Axum);
        let initial = baseline.derived.total_bytes;
        for captured in [&mut baseline, &mut current] {
            captured.raw_limits.isolate_total_bytes = configured;
            captured.raw_limits.baseline_bytes = captured
                .raw_limits
                .baseline_bytes
                .checked_add(demand.checked_sub(initial).unwrap())
                .unwrap();
            if let Some(cap) = known {
                captured.platform_facts.ceiling = Fact::Known {
                    value: Ceiling {
                        primary_bytes: cap,
                        scope: captured.memory_scope.clone(),
                        separate_stack_bytes: None,
                    },
                    source: "synthetic-platform-ceiling".to_owned(),
                };
                captured.dispatch_facts.ceiling = Fact::Known {
                    value: Ceiling {
                        primary_bytes: cap,
                        scope: captured.memory_scope.clone(),
                        separate_stack_bytes: None,
                    },
                    source: "synthetic-platform-ceiling".to_owned(),
                };
            }
            rederive(captured);
            assert_eq!(captured.derived.total_bytes, demand);
        }
        (baseline, current)
    }

    #[test]
    fn native_uses_smaller_known_ceiling_and_its_own_headroom() {
        for (configured, known) in [
            (0x1_0000_0000, 0x2_0000_0000),
            (0x2_0000_0000, 0x1_0000_0000),
        ] {
            // 4 GiB minus the native 256 MiB floor, not 8 GiB's 512 MiB.
            let (baseline, current) = native_pair(configured, Some(known), 0xF000_0000);
            let comparison = compare(&baseline, &current).unwrap();
            assert_eq!(comparison.native_arithmetic_fit, Some(true));
            assert!(!comparison.absolute_required_fit);
        }
    }

    #[test]
    fn native_nonfit_has_a_distinct_fixed_diagnostic_after_independent_gates() {
        for (configured, known) in [
            (0x1_0000_0000, Some(0x2_0000_0000)),
            (0x2_0000_0000, Some(0x1_0000_0000)),
            (0x1_0000_0000, None),
        ] {
            let (baseline, mut current) = native_pair(configured, known, 0xF000_0000);
            current.catalog_costs = CatalogResourceCosts {
                resident_bytes: 101,
                initialization_bytes: 211,
                schema_validation_per_auction_bytes: 307,
                context_per_auction_bytes: 401,
            };
            rederive(&mut current);
            super::check_matched_inputs(&baseline, &current).unwrap();
            super::catalog_allowances::check_allowances(current.catalog_costs).unwrap();
            super::check_capture(&baseline).unwrap();
            super::check_capture(&current).unwrap();
            let growth = super::growth_between(&baseline, &current).unwrap();
            super::check_isolate_growth(&baseline, &current, &growth).unwrap();
            assert_eq!(
                current
                    .derived
                    .total_bytes
                    .checked_sub(baseline.derived.total_bytes),
                Some(2_436_u64)
            );
            assert!(2_436_u64 <= super::total_allowance(AdapterId::Axum));
            let ceiling = configured.min(known.unwrap_or(configured));
            let cap = super::NonZeroU64::new(ceiling).unwrap();
            let required = current
                .derived
                .total_bytes
                .checked_add(super::minimum_headroom_bytes(
                    cap,
                    super::HeadroomClass::Native,
                ))
                .unwrap();
            assert!(required > ceiling);
            let error = compare(&baseline, &current).expect_err("native nonfit remains refused");
            assert_eq!(error, Error::ResourceNativeFit);
            assert_eq!(error.code(), "bidder_resource_native_fit");
        }
    }

    #[test]
    fn malformed_native_capture_remains_an_input_error_before_nonfit() {
        for forge_baseline in [false, true] {
            let (mut baseline, mut current) =
                native_pair(0x2_0000_0000, Some(0x1_0000_0000), 0xF000_0000);
            current.catalog_costs.resident_bytes = 1;
            rederive(&mut current);
            let forged = if forge_baseline {
                &mut baseline
            } else {
                &mut current
            };
            edit(forged, &["terms", "baseline"], 1_u64.into());
            seal(forged);
            let error =
                compare(&baseline, &current).expect_err("malformed capture remains refused");
            assert_eq!(error, Error::ResourceInput);
            assert_eq!(error.code(), "bidder_resource_input");
        }
    }

    #[test]
    fn native_headroom_cannot_be_spent_on_catalog_growth() {
        let (baseline, mut current) = native_pair(0x2_0000_0000, Some(0x1_0000_0000), 0xF000_0000);
        current.catalog_costs.resident_bytes = 1;
        rederive(&mut current);
        assert!(matches!(
            compare(&baseline, &current),
            Err(Error::ResourceNativeFit)
        ));
    }

    #[test]
    fn native_fractional_headroom_rounds_up_above_its_floor() {
        let (baseline, mut current) =
            native_pair(0x2_0000_0001, Some(0x3_0000_0000), 0x1_E000_0000);
        assert_eq!(
            compare(&baseline, &current).unwrap().native_arithmetic_fit,
            Some(true)
        );
        current.catalog_costs.initialization_bytes = 1;
        rederive(&mut current);
        assert!(matches!(
            compare(&baseline, &current),
            Err(Error::ResourceNativeFit)
        ));
    }

    #[test]
    fn unknown_native_platform_ceiling_uses_local_cap_without_promoting_readiness() {
        let (baseline, mut current) = native_pair(0x1_0000_0000, None, 0xF000_0000);
        let comparison = compare(&baseline, &current).unwrap();
        assert_eq!(comparison.native_arithmetic_fit, Some(true));
        assert!(!comparison.absolute_required_fit);
        current.catalog_costs.initialization_bytes = 1;
        rederive(&mut current);
        assert!(matches!(
            compare(&baseline, &current),
            Err(Error::ResourceNativeFit)
        ));
    }

    #[test]
    fn absolute_required_fit_is_only_copied_from_candidate_flag() {
        let (baseline, mut current) = pair(AdapterId::Axum);
        current.required_fit = true;
        seal(&mut current);
        assert!(compare(&baseline, &current).unwrap().absolute_required_fit);
        current.required_fit = false;
        seal(&mut current);
        assert!(!compare(&baseline, &current).unwrap().absolute_required_fit);
    }

    #[test]
    fn overcommitted_wasm_rows_can_pass_only_numeric_relative_comparison() {
        for adapter in [AdapterId::Cloudflare, AdapterId::Fastly, AdapterId::Spin] {
            let (baseline, mut current) = pair(adapter);
            current.catalog_costs.resident_bytes = 23;
            current.catalog_costs.initialization_bytes = 29;
            rederive(&mut current);
            assert!(current.derived.total_bytes > current.effective_limits.isolate_total_bytes);
            let comparison = compare(&baseline, &current).unwrap();
            assert_eq!(comparison.total_delta_bytes, 52);
            assert_eq!(comparison.native_arithmetic_fit, None);
            assert!(!comparison.absolute_required_fit);
        }
    }
}
