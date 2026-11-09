//! Explicit accepted resource facts and owned prepaid auction funding.

use super::audit::{AuctionDisclosureScope, CompletionStatus};
use auctioneer_bidder::{BidderRegistry, EntryPoint};
use auctioneer_config_data::types::{AccountConfig, BidderImplementationBinding, HostConfig};
use auctioneer_exchange::BidderFanout;
use auctioneer_exchange::context::RequestConstructionLimits;
use auctioneer_exchange::scrub::DisclosureAudit;
use auctioneer_privacy::{HostRestrictionSource, gcl::Gcl, gvl::GvlStore};
use auctioneer_runtime::budget::{
    NORMALIZATION_IMPLICIT_NODES, derive_isolate_budget_with_baked_config,
    normalization_wire_allowance,
};
use auctioneer_runtime::{
    AdmissionPermit, AuctionBudget, BudgetError, CatalogResourceCosts, ChargeKind, Derived,
    RequestConstructionCosts, Reservation, ResolvedIsolateLimits, ResourceError,
    TargetPlatformConstants,
};
use auctioneer_runtime::{
    Clock, MonotonicInstant, Rng, bidder_credentials::BidderCredentialSource,
};
use std::{fmt, sync::Arc};
use time::OffsetDateTime;

/// Borrowed, accepted host services. There are no global-service fallbacks.
pub struct AuctionServices<'host> {
    pub resources: &'host AuctionResourceProfile,
    pub product_host: &'host HostConfig,
    pub product_account: &'host AccountConfig,
    pub bidders: &'host BidderRegistry,
    pub bidder_bindings: &'host [BidderImplementationBinding],
    pub fanout: &'host dyn BidderFanout,
    pub clock: &'host dyn Clock,
    pub rng: &'host dyn Rng,
    pub gvl_store: &'host GvlStore,
    pub gcl: &'host Gcl,
    pub restrictions: &'host dyn HostRestrictionSource,
    pub credentials: Option<&'host dyn BidderCredentialSource>,
    pub request_start: MonotonicInstant,
    pub outer_deadline: Option<MonotonicInstant>,
    pub now: OffsetDateTime,
    pub debug_requested: bool,
    pub entry_point: EntryPoint,
}

impl AuctionServices<'_> {
    pub(super) fn deadline_spent(&self) -> bool {
        self.outer_deadline
            .is_some_and(|deadline| self.clock.now_monotonic() >= deadline)
    }

    pub(super) fn bindings_match(&self) -> bool {
        self.bidder_bindings
            .iter()
            .zip(self.bidder_bindings.iter().skip(1))
            .all(|(left, right)| left.bidder < right.bidder)
            && self
                .bidder_bindings
                .iter()
                .all(|row| !row.bidder.is_empty() && !row.implementation.is_empty())
            && self.bidders.infos().all(|info| {
                self.bidder_bindings
                    .binary_search_by_key(&info.name.as_str(), |row| row.bidder)
                    .ok()
                    .and_then(|index| self.bidder_bindings.get(index))
                    .is_some_and(|binding| {
                        self.bidders.implementation(info.name.as_str())
                            == Some(binding.implementation)
                    })
            })
    }
}

/// The existing complete derivation tuple, supplied by its accepted host owner.
#[derive(Clone, Copy, Debug)]
pub struct AuctionResourceInputs {
    pub limits: ResolvedIsolateLimits,
    pub target: TargetPlatformConstants,
    pub outbound_meta_bytes: u32,
    pub construction_costs: RequestConstructionCosts,
    pub product_default_bytes: u64,
    pub catalog_costs: CatalogResourceCosts,
}

/// Qualification is an explicit host fact, not inferred from nonzero costs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaAllowance {
    Disabled,
    Qualified(u64),
}

/// Canonically derived capacity. This does not certify the combined host's fit.
pub struct AuctionResourceProfile {
    inputs: AuctionResourceInputs,
    derived: Derived,
    schema_scratch_bytes: u64,
}

impl AuctionResourceProfile {
    /// Derive from every existing owner input; never accept a fabricated `Derived`.
    ///
    /// # Errors
    /// Returns the existing budget error for invalid facts or an unfunded schema allowance.
    pub fn new(
        inputs: AuctionResourceInputs,
        schema: SchemaAllowance,
    ) -> Result<Self, BudgetError> {
        let derived = derive_isolate_budget_with_baked_config(
            &inputs.limits,
            &inputs.target,
            inputs.outbound_meta_bytes,
            inputs.construction_costs,
            inputs.product_default_bytes,
            inputs.catalog_costs,
        )?;
        let schema_scratch_bytes = match schema {
            SchemaAllowance::Disabled => 0,
            SchemaAllowance::Qualified(bytes)
                if bytes != 0
                    && bytes == inputs.catalog_costs.schema_validation_per_auction_bytes
                    && bytes <= derived.auction_charges.projection =>
            {
                bytes
            }
            SchemaAllowance::Qualified(_) => return Err(BudgetError::UnqualifiedSchemaArtifact),
        };
        Ok(Self {
            inputs,
            derived,
            schema_scratch_bytes,
        })
    }

    #[must_use]
    pub fn inputs(&self) -> &AuctionResourceInputs {
        &self.inputs
    }

    #[must_use]
    pub fn derived(&self) -> &Derived {
        &self.derived
    }

    #[must_use]
    pub fn schema_scratch_bytes(&self) -> u64 {
        self.schema_scratch_bytes
    }

    fn ensure_owner_costs(&self) -> Result<(), ResourceError> {
        const REQUIRED: RequestConstructionCosts = super::audit::construction_costs();
        let supplied = self.inputs.construction_costs;
        if supplied.fixed_per_auction_bytes < REQUIRED.fixed_per_auction_bytes {
            return Err(ResourceError::Exhausted {
                kind: ChargeKind::Privacy,
            });
        }
        if supplied.reusable_scratch_bytes < REQUIRED.reusable_scratch_bytes
            || supplied.retained_per_scope_bytes < REQUIRED.retained_per_scope_bytes
        {
            return Err(ResourceError::Exhausted {
                kind: ChargeKind::Projection,
            });
        }
        Ok(())
    }

    pub(crate) fn construction_limits(&self) -> Result<RequestConstructionLimits, ResourceError> {
        let limits = &self.inputs.limits;
        normalized_construction_limits(
            u64::from(limits.max_parsed_nodes),
            limits.inbound_request_bytes,
            u64::from(limits.max_copied_header_bytes),
            self.inputs.construction_costs,
        )
    }
}

fn normalized_construction_limits(
    input_nodes: u64,
    inbound_bytes: u64,
    copied_header_bytes: u64,
    costs: RequestConstructionCosts,
) -> Result<RequestConstructionLimits, ResourceError> {
    let max_nodes = input_nodes
        .checked_add(NORMALIZATION_IMPLICIT_NODES)
        .ok_or(ResourceError::CapacityOverflow)?;
    let max_serialized_bytes =
        normalization_wire_allowance(inbound_bytes, copied_header_bytes, input_nodes)
            .map_err(|_error| ResourceError::CapacityOverflow)?;
    Ok(RequestConstructionLimits {
        max_nodes,
        max_serialized_bytes,
        reusable_scratch_bytes: costs.reusable_scratch_bytes,
        retained_per_scope_bytes: costs.retained_per_scope_bytes,
    })
}

/// The existing acquired capacity slot and prepaid phases, never a second pool.
/// Audit observes before the permit and receipts are destroyed.
pub struct AuctionFunding {
    audit: AuctionDisclosureScope,
    _permit: AdmissionPermit,
    receipts: [Reservation<'static>; 6],
}

impl AuctionFunding {
    /// Prepay before configuration/input projection, moving an already-acquired slot.
    ///
    /// # Errors
    /// Fixed, allocation-free refusal; partial prepayment and the supplied permit roll back.
    pub fn prepay(
        profile: &AuctionResourceProfile,
        budget: &Arc<AuctionBudget>,
        permit: AdmissionPermit,
    ) -> Result<Self, ResourceError> {
        profile.ensure_owner_costs()?;
        let receipts = profile.derived.auction_charges.prepay_owned(budget)?;
        Ok(Self {
            audit: AuctionDisclosureScope::new(),
            _permit: permit,
            receipts,
        })
    }

    pub(crate) fn ensure(&self, profile: &AuctionResourceProfile) -> Result<(), ResourceError> {
        profile.ensure_owner_costs()?;
        let costs = profile.derived.auction_charges;
        for (kind, bytes) in [
            (ChargeKind::RuntimeConfig, costs.config),
            (ChargeKind::Ingress, costs.ingress),
            (ChargeKind::Privacy, costs.privacy),
            (ChargeKind::Projection, costs.projection),
            (ChargeKind::Outbound, costs.outbound),
            (ChargeKind::Response, costs.response),
        ] {
            let receipt = self
                .receipts
                .iter()
                .find(|receipt| receipt.kind() == kind)
                .ok_or(ResourceError::Exhausted { kind })?;
            receipt.ensure_capacity(kind, bytes)?;
        }
        Ok(())
    }

    pub(crate) fn parts(&mut self) -> (&mut DisclosureAudit, &[Reservation<'static>; 6]) {
        (self.audit.audit_mut(), &self.receipts)
    }

    pub(crate) fn finish(&mut self, status: CompletionStatus) {
        self.audit.finish(status);
    }
}

impl fmt::Debug for AuctionFunding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuctionFunding").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::{AuctionFunding, AuctionResourceInputs, AuctionResourceProfile, SchemaAllowance};
    use crate::test_support::boot_parity_fixture::profile_for_adapter;
    use auctioneer_runtime::budget::derive_isolate_budget_with_baked_config;
    use auctioneer_runtime::{
        AdmissionGate, AuctionBudget, BudgetError, ChargeKind, ResourceError,
    };
    use std::sync::Arc;

    #[test]
    fn normalized_construction_limits_include_funded_header_and_node_additions() {
        let costs = super::super::audit::construction_costs();
        let limits = super::normalized_construction_limits(512, 1024, 4096, costs)
            .expect("funded additions fit");
        assert_eq!(limits.max_nodes, 542);
        assert_eq!(
            limits.max_serialized_bytes,
            1024_u64
                .checked_mul(3)
                .unwrap()
                .checked_add(4096_u64.checked_mul(8).unwrap())
                .unwrap()
                .checked_add(189)
                .unwrap()
                .checked_add(512_u64.checked_mul(23).unwrap())
                .unwrap()
        );
        assert_eq!(limits.reusable_scratch_bytes, costs.reusable_scratch_bytes);
        assert_eq!(
            limits.retained_per_scope_bytes,
            costs.retained_per_scope_bytes
        );
    }

    #[test]
    fn normalized_construction_overflow_is_a_static_resource_refusal() {
        for (nodes, inbound, headers) in [
            (u64::MAX, 0, 0),
            (0, u64::MAX, 0),
            (0, 0, u64::MAX),
            (u64::MAX.checked_div(23).unwrap() + 1, 0, 0),
            (u64::MAX.checked_div(23).unwrap(), 0, 0),
        ] {
            let error = super::normalized_construction_limits(
                nodes,
                inbound,
                headers,
                super::super::audit::construction_costs(),
            )
            .expect_err("overflow refuses rather than widening the allowance");
            assert!(matches!(error, ResourceError::CapacityOverflow));
        }
    }

    fn inputs() -> AuctionResourceInputs {
        let row = profile_for_adapter("unequal_explicit_caps", "axum");
        AuctionResourceInputs {
            limits: row.limits,
            target: row.target,
            outbound_meta_bytes: row.outbound_meta_bytes,
            construction_costs: row.construction_costs,
            product_default_bytes: 271,
            catalog_costs: row.catalog_costs,
        }
    }

    #[test]
    fn profile_uses_the_complete_existing_derivation_tuple() {
        let facts = inputs();
        let expected = derive_isolate_budget_with_baked_config(
            &facts.limits,
            &facts.target,
            facts.outbound_meta_bytes,
            facts.construction_costs,
            facts.product_default_bytes,
            facts.catalog_costs,
        )
        .unwrap();
        let profile = AuctionResourceProfile::new(facts, SchemaAllowance::Disabled).unwrap();
        assert_eq!(*profile.derived(), expected);
        assert_eq!(profile.schema_scratch_bytes(), 0);
        assert_eq!(profile.inputs().product_default_bytes, 271);
    }

    #[test]
    fn schema_allowance_requires_explicit_qualification_and_exact_funding() {
        for allowance in [0, 3010, 3012] {
            assert!(matches!(
                AuctionResourceProfile::new(inputs(), SchemaAllowance::Qualified(allowance)),
                Err(BudgetError::UnqualifiedSchemaArtifact)
            ));
        }
        let profile =
            AuctionResourceProfile::new(inputs(), SchemaAllowance::Qualified(3011)).unwrap();
        assert_eq!(profile.schema_scratch_bytes(), 3011);
    }

    #[test]
    fn funding_keeps_the_original_ledger_and_acquired_population_slot_alive() {
        let profile = AuctionResourceProfile::new(inputs(), SchemaAllowance::Disabled).unwrap();
        let total = profile.derived().auction_charges.total().unwrap();
        let budget = Arc::new(AuctionBudget::new(total));
        let weak = Arc::downgrade(&budget);
        let gate = AdmissionGate::new(1);
        let funding =
            AuctionFunding::prepay(&profile, &budget, gate.try_acquire().unwrap()).unwrap();
        assert_eq!(budget.in_use(), total);
        assert_eq!(gate.in_use(), 1);
        assert!(gate.try_acquire().is_none());
        drop(budget);
        assert_eq!(weak.upgrade().unwrap().in_use(), total);
        drop(funding);
        assert!(weak.upgrade().is_none());
        assert_eq!(gate.in_use(), 0);
        assert!(gate.try_acquire().is_some());
    }

    #[test]
    fn prepayment_rejects_each_underdeclared_construction_owner_before_work() {
        let required = super::super::audit::construction_costs();
        for field in 0_u8..3 {
            for offset in [-1_i8, 0, 1] {
                let mut facts = inputs();
                let declared = match field {
                    0 => &mut facts.construction_costs.fixed_per_auction_bytes,
                    1 => &mut facts.construction_costs.reusable_scratch_bytes,
                    _ => &mut facts.construction_costs.retained_per_scope_bytes,
                };
                let minimum = match field {
                    0 => required.fixed_per_auction_bytes,
                    1 => required.reusable_scratch_bytes,
                    _ => required.retained_per_scope_bytes,
                };
                assert!(minimum > 0);
                *declared = minimum.checked_add_signed(i64::from(offset)).unwrap();
                let profile =
                    AuctionResourceProfile::new(facts, SchemaAllowance::Disabled).unwrap();
                let budget = Arc::new(AuctionBudget::new(profile.derived().per_auction_peak));
                let gate = AdmissionGate::new(1);
                let result = AuctionFunding::prepay(&profile, &budget, gate.try_acquire().unwrap());
                if offset < 0 {
                    let kind = if field == 0 {
                        ChargeKind::Privacy
                    } else {
                        ChargeKind::Projection
                    };
                    assert!(
                        matches!(result, Err(ResourceError::Exhausted { kind: actual }) if actual == kind)
                    );
                    assert_eq!(budget.in_use(), 0);
                    assert_eq!(gate.in_use(), 0);
                } else {
                    let funded = result.unwrap();
                    funded.ensure(&profile).unwrap();
                    assert_eq!(budget.in_use(), profile.derived().per_auction_peak);
                    assert_eq!(gate.in_use(), 1);
                    drop(funded);
                    assert_eq!(budget.in_use(), 0);
                    assert_eq!(gate.in_use(), 0);
                }
            }
        }
    }

    #[test]
    fn insufficient_prepayment_releases_every_phase_and_the_supplied_permit() {
        let profile = AuctionResourceProfile::new(inputs(), SchemaAllowance::Disabled).unwrap();
        let total = profile.derived().auction_charges.total().unwrap();
        let budget = Arc::new(AuctionBudget::new(total - 1));
        let gate = AdmissionGate::new(1);
        assert_eq!(
            AuctionFunding::prepay(&profile, &budget, gate.try_acquire().unwrap()).unwrap_err(),
            ResourceError::Exhausted {
                kind: ChargeKind::Response
            }
        );
        assert_eq!(budget.in_use(), 0);
        assert_eq!(gate.in_use(), 0);
    }
}
