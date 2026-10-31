//! Request-scoped decisions resolved before the auction runs.

use crate::alias::ResolvedAliases;
use crate::budgets::{BootEnvelope, ResolvedBudgets};
use auctioneer_bidder::BidderName;
use std::collections::{BTreeMap, BTreeSet};

/// Immutable auction scratch, borrowed alongside `AuctionContext`.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct AuctionPlan {
    pub aliases: ResolvedAliases,
    pub disabled_bidders: BTreeSet<BidderName>,
    /// Requested per-bidder caps, before funded-envelope clamping.
    pub budgets: ResolvedBudgets,
    /// Installed funded ceilings. Absent for callers without a boot envelope.
    pub boot_envelope: Option<BootEnvelope>,
    /// Per-request sample-rate map. The handler builds it at request
    /// entry and the sampler reads it. Empty for tests and handlers
    /// that don't sample.
    pub bidder_sample_rates: BTreeMap<BidderName, f64>,
}

impl AuctionPlan {
    #[must_use]
    pub fn new(
        aliases: ResolvedAliases,
        disabled_bidders: BTreeSet<BidderName>,
        budgets: ResolvedBudgets,
        bidder_sample_rates: BTreeMap<BidderName, f64>,
    ) -> Self {
        Self {
            aliases,
            disabled_bidders,
            budgets,
            boot_envelope: None,
            bidder_sample_rates,
        }
    }

    #[must_use]
    pub fn with_boot_envelope(mut self, envelope: BootEnvelope) -> Self {
        self.boot_envelope = Some(envelope);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::AuctionPlan;
    use crate::scrub::construction_costs;
    use crate::{AliasMap, BidderBudgets, BootEnvelope, ResolvedAliases};
    use auctioneer_bidder::BidderName;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn construction_preserves_resolved_decisions_and_funded_caps() {
        let mut map = AliasMap::default();
        map.bases
            .insert(BidderName::new("fake_bz"), BidderName::new("fake_aaa"));
        let budgets = BTreeMap::from([(
            BidderName::new("fake_aaa"),
            BidderBudgets {
                max_transport_bytes: 2_048,
                max_decoded_bytes: 0x2000,
            },
        )]);
        let disabled = BTreeSet::from([BidderName::new("fake_disabled")]);
        let rates = BTreeMap::from([(BidderName::new("fake_aaa"), 0.5_f64)]);
        let envelope = BootEnvelope {
            construction_costs: construction_costs(),
            max_transport_bytes: 4_096,
            max_decoded_bytes: 0x4000,
        };
        let plan = AuctionPlan::new(
            ResolvedAliases::wrap(map),
            disabled.clone(),
            budgets.clone(),
            rates.clone(),
        )
        .with_boot_envelope(envelope);
        assert_eq!(
            plan.aliases.map.bases.get(&BidderName::new("fake_bz")),
            Some(&BidderName::new("fake_aaa")),
            "the constructor must preserve the resolved alias map verbatim",
        );
        assert_eq!(plan.budgets, budgets);
        assert_eq!(plan.disabled_bidders, disabled);
        assert_eq!(plan.bidder_sample_rates, rates);
        assert_eq!(plan.boot_envelope, Some(envelope));
    }

    #[test]
    fn empty_aliases_carry_no_warnings() {
        let aliases = ResolvedAliases::empty();
        assert!(aliases.map.bases.is_empty());
        assert!(aliases.disabled_warnings.is_empty());
    }
}
