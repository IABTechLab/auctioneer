//! Explicit accepted resource facts and owned prepaid auction funding.

use crate::handlers::request_disclosure_audit::{AuctionDisclosureScope, CompletionStatus};
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

    pub(crate) fn construction_limits(&self) -> Result<RequestConstructionLimits, ResourceError> {
        let limits = &self.inputs.limits;
        let max_nodes = u64::from(limits.max_parsed_nodes)
            .checked_add(NORMALIZATION_IMPLICIT_NODES)
            .ok_or(ResourceError::CapacityOverflow)?;
        let max_serialized_bytes = normalization_wire_allowance(
            limits.inbound_request_bytes,
            u64::from(limits.max_copied_header_bytes),
            u64::from(limits.max_parsed_nodes),
        )
        .map_err(|_error| ResourceError::CapacityOverflow)?;
        Ok(RequestConstructionLimits {
            max_nodes,
            max_serialized_bytes,
            reusable_scratch_bytes: self.inputs.construction_costs.reusable_scratch_bytes,
            retained_per_scope_bytes: self.inputs.construction_costs.retained_per_scope_bytes,
        })
    }
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
        let receipts = profile.derived.auction_charges.prepay_owned(budget)?;
        Ok(Self {
            audit: AuctionDisclosureScope::new(),
            _permit: permit,
            receipts,
        })
    }

    pub(crate) fn ensure(&self, profile: &AuctionResourceProfile) -> Result<(), ResourceError> {
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
