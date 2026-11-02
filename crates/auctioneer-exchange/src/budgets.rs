//! Independent response caps resolved for each observable bidder.

use crate::alias::ResolvedAliases;
use crate::error::AuctionError;
use crate::response::ErrorEntry;
use auctioneer_bidder::BidderName;
use auctioneer_openrtb::WarningCode;
use std::collections::BTreeMap;
use thiserror::Error;

/// Resolved caps keyed by observable name. A missing alias row may
/// inherit its base's fully resolved row at lookup.
pub type ResolvedBudgets = BTreeMap<BidderName, BidderBudgets>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BidderBudgets {
    pub max_transport_bytes: u64,
    pub max_decoded_bytes: u64,
}

/// Funded ceilings, not runtime host defaults. Higher scoped overrides
/// may raise a host default but cannot authorize unfunded allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootEnvelope {
    pub max_transport_bytes: u64,
    pub max_decoded_bytes: u64,
    /// The construction descriptor funded by the installed auction ledger.
    pub construction_costs: auctioneer_runtime::RequestConstructionCosts,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum InternalInvariantError {
    #[error("missing resolved bidder response budget")]
    BudgetMissing { bidder_name: BidderName },
}

impl From<InternalInvariantError> for AuctionError {
    fn from(_error: InternalInvariantError) -> Self {
        Self::Internal("Internal auction error".to_owned())
    }
}

/// Look up a fully resolved row and independently clamp its caps to the
/// installed envelope, attributing a single warning to this bidder.
/// `None` is for callers without an installed boot envelope.
///
/// # Errors
/// Returns [`InternalInvariantError::BudgetMissing`] when neither the
/// observable name nor its resolved alias base has a budget row.
pub fn resolve_budget(
    budgets: &ResolvedBudgets,
    aliases: &ResolvedAliases,
    bidder_name: &BidderName,
    boot_envelope: Option<BootEnvelope>,
    warnings: &mut Vec<ErrorEntry>,
) -> Result<BidderBudgets, InternalInvariantError> {
    let requested = budgets
        .get(bidder_name)
        .or_else(|| {
            let base = aliases.map.bases.get(bidder_name)?;
            budgets.get(base)
        })
        .copied()
        .ok_or_else(|| InternalInvariantError::BudgetMissing {
            bidder_name: bidder_name.clone(),
        })?;
    let Some(envelope) = boot_envelope else {
        return Ok(requested);
    };
    let resolved = BidderBudgets {
        max_transport_bytes: requested
            .max_transport_bytes
            .min(envelope.max_transport_bytes),
        max_decoded_bytes: requested.max_decoded_bytes.min(envelope.max_decoded_bytes),
    };
    if resolved != requested {
        warnings.push(ErrorEntry {
            code: WarningCode::OutboundBudgetClamp.as_u32(),
            message: "per-bidder response budget exceeded the adapter's boot envelope and was clamped down".to_owned(),
        });
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::{
        BidderBudgets, BootEnvelope, InternalInvariantError, ResolvedBudgets, resolve_budget,
    };
    use crate::scrub::construction_costs;
    use crate::{AliasMap, ResolvedAliases};
    use auctioneer_bidder::BidderName;
    use auctioneer_openrtb::WarningCode;

    #[test]
    fn budget_lookup_prefers_the_observable_row_then_the_alias_base() {
        let base = BidderName::new("fake_base");
        let alias = BidderName::new("fake_alias");
        let mut map = AliasMap::default();
        map.bases.insert(alias.clone(), base.clone());
        let aliases = ResolvedAliases::wrap(map);
        let inherited = BidderBudgets {
            max_transport_bytes: 2_048,
            max_decoded_bytes: 0x2000,
        };
        let own = BidderBudgets {
            max_transport_bytes: 4_096,
            max_decoded_bytes: 0x4000,
        };
        let mut budgets = ResolvedBudgets::from([(base, inherited)]);
        let mut warnings = Vec::new();
        assert_eq!(
            resolve_budget(&budgets, &aliases, &alias, None, &mut warnings).unwrap(),
            inherited
        );
        budgets.insert(alias.clone(), own);
        assert_eq!(
            resolve_budget(&budgets, &aliases, &alias, None, &mut warnings).unwrap(),
            own
        );
        assert!(warnings.is_empty());
    }

    #[test]
    fn budget_lookup_missing_entry_returns_an_internal_invariant_error() {
        let alias = BidderName::new("fake_alias");
        let mut map = AliasMap::default();
        map.bases
            .insert(alias.clone(), BidderName::new("fake_base"));
        for aliases in [ResolvedAliases::empty(), ResolvedAliases::wrap(map)] {
            let mut warnings = Vec::new();
            assert_eq!(
                resolve_budget(
                    &ResolvedBudgets::new(),
                    &aliases,
                    &alias,
                    None,
                    &mut warnings
                ),
                Err(InternalInvariantError::BudgetMissing {
                    bidder_name: alias.clone()
                }),
            );
            assert!(warnings.is_empty());
        }
    }

    #[test]
    fn budget_caps_clamp_independently_to_the_boot_envelope_with_one_warning() {
        let name = BidderName::new("fake_bidder");
        let envelope = BootEnvelope {
            max_transport_bytes: 0x2000,
            max_decoded_bytes: 0x4000,
            construction_costs: construction_costs(),
        };
        for (requested, expected, warning_count) in [
            ((2_048, 4_096), (2_048, 4_096), 0),
            ((0x2000, 0x4000), (0x2000, 0x4000), 0),
            ((0x8000, 4_096), (0x2000, 4_096), 1),
            ((2_048, 0x8000), (2_048, 0x4000), 1),
            ((0x8000, 0x8000), (0x2000, 0x4000), 1),
        ] {
            let budgets = ResolvedBudgets::from([(
                name.clone(),
                BidderBudgets {
                    max_transport_bytes: requested.0,
                    max_decoded_bytes: requested.1,
                },
            )]);
            let mut warnings = Vec::new();
            let actual = resolve_budget(
                &budgets,
                &ResolvedAliases::empty(),
                &name,
                Some(envelope),
                &mut warnings,
            )
            .unwrap();
            assert_eq!(
                (actual.max_transport_bytes, actual.max_decoded_bytes),
                expected
            );
            assert_eq!(warnings.len(), warning_count);
            for warning in &warnings {
                assert_ne!(warning.code, WarningCode::GclStale.as_u32());
                assert_eq!(warning.code, WarningCode::OutboundBudgetClamp.as_u32());
            }
        }
    }

    #[test]
    fn an_alias_inheriting_an_oversized_base_gets_its_own_clamp_warning() {
        let alias = BidderName::new("fake_alias");
        let base = BidderName::new("fake_base");
        let mut map = AliasMap::default();
        map.bases.insert(alias.clone(), base.clone());
        let budgets = ResolvedBudgets::from([(
            base,
            BidderBudgets {
                max_transport_bytes: 0x8000,
                max_decoded_bytes: 0x0001_0000,
            },
        )]);
        let mut warnings = Vec::new();
        assert_eq!(
            resolve_budget(
                &budgets,
                &ResolvedAliases::wrap(map),
                &alias,
                Some(BootEnvelope {
                    max_transport_bytes: 0x2000,
                    max_decoded_bytes: 0x4000,
                    construction_costs: construction_costs(),
                }),
                &mut warnings
            )
            .unwrap(),
            BidderBudgets {
                max_transport_bytes: 0x2000,
                max_decoded_bytes: 0x4000
            },
        );
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, WarningCode::OutboundBudgetClamp.as_u32());
    }
}
