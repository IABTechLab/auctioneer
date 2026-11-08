//! Preflight request-derived response metadata before copying identifiers.

use crate::AuctionError;
use crate::targeting::Winner;
use auctioneer_runtime::ResourceError;
use auctioneer_runtime::bounded_json::NODE_RETAINED_BYTES;

/// Charged separately from bidder response parsing. The byte and node
/// caps are the same materialization caps the composing handler resolves.
pub(crate) struct BookkeepingBudget {
    bytes: u64,
    nodes: u64,
    used_bytes: u64,
    used_nodes: u64,
    owned_bytes: u64,
    retained_limit: u64,
}

impl BookkeepingBudget {
    pub(crate) fn new(bytes: u64, nodes: u32) -> Result<Self, ResourceError> {
        let node_limit = u64::from(nodes);
        let retained_limit =
            Self::retained_bytes(bytes, node_limit, 0).ok_or(ResourceError::CapacityOverflow)?;
        Ok(Self {
            bytes,
            nodes: node_limit,
            used_bytes: 0,
            used_nodes: 0,
            owned_bytes: 0,
            retained_limit,
        })
    }

    pub(crate) fn response(&mut self, id: &str) -> Result<(), AuctionError> {
        // Response ID, fixed global fields and their diagnostic containers.
        let bytes = u64::try_from(id.len()).unwrap_or(u64::MAX).checked_add(128);
        self.reserve(bytes, 16)
    }

    pub(crate) fn warning(&mut self, message: &str) -> Result<(), AuctionError> {
        let bytes = u64::try_from(message.len())
            .unwrap_or(u64::MAX)
            .checked_add(32);
        self.reserve(bytes, 8)
    }

    pub(crate) fn bidder(&mut self, name: &str) -> Result<(), AuctionError> {
        // Identity keys, one pre-fanout diagnostic and their containers.
        let bytes = u64::try_from(name.len())
            .unwrap_or(u64::MAX)
            .checked_mul(2)
            .and_then(|n| n.checked_add(128));
        self.reserve(bytes, 16)
    }

    pub(crate) fn nonbid(&mut self, imp_id: &str) -> Result<(), AuctionError> {
        let bytes = u64::try_from(imp_id.len())
            .unwrap_or(u64::MAX)
            .checked_add(32);
        self.reserve(bytes, 8)
    }

    pub(crate) fn impression(&mut self, imp_id: &str) -> Result<(), AuctionError> {
        // Owned identity plus the existing reducer decision retained beside it.
        let slots = size_of::<String>()
            .checked_add(size_of::<Option<Winner>>())
            .ok_or(ResourceError::CapacityOverflow)?;
        let bytes = u64::try_from(imp_id.len())
            .unwrap_or(u64::MAX)
            .checked_add(u64::try_from(slots).unwrap_or(u64::MAX));
        self.reserve_owned(bytes)
    }

    pub(crate) fn selection_names(
        &mut self,
        bidder: &str,
        impressions: usize,
    ) -> Result<(), AuctionError> {
        // A winner owns its requested name. Charging every participating name
        // per impression bounds whichever contender the existing reducer keeps.
        let bytes = u64::try_from(bidder.len())
            .ok()
            .and_then(|length| length.checked_mul(u64::try_from(impressions).ok()?));
        self.reserve_owned(bytes)
    }

    /// Wire metadata has four coexisting representations. Typed outcome
    /// identities have one owner, within the same original funded ceiling but
    /// without consuming the independent wire byte/node quotas.
    pub(crate) fn retained_peak(&self) -> Result<u64, ResourceError> {
        Self::retained_bytes(self.used_bytes, self.used_nodes, self.owned_bytes)
            .ok_or(ResourceError::CapacityOverflow)
    }

    fn retained_bytes(bytes: u64, nodes: u64, owned: u64) -> Option<u64> {
        let node_bytes = nodes.checked_mul(NODE_RETAINED_BYTES)?;
        let metadata_bytes = node_bytes.checked_add(bytes)?.checked_mul(4)?;
        metadata_bytes.checked_add(owned)
    }

    fn reserve_owned(&mut self, charge: Option<u64>) -> Result<(), AuctionError> {
        let prospective = charge
            .and_then(|bytes| self.owned_bytes.checked_add(bytes))
            .filter(|&owned| {
                Self::retained_bytes(self.used_bytes, self.used_nodes, owned)
                    .is_some_and(|peak| peak <= self.retained_limit)
            });
        let Some(owned) = prospective else {
            return Err(AuctionError::BadInput(
                "response bookkeeping exceeds the materialization budget".into(),
            ));
        };
        self.owned_bytes = owned;
        Ok(())
    }

    fn reserve(&mut self, charge: Option<u64>, nodes: u64) -> Result<(), AuctionError> {
        let prospective = charge
            .and_then(|bytes| {
                Some((
                    self.bytes.checked_sub(bytes)?,
                    self.nodes.checked_sub(nodes)?,
                    self.used_bytes.checked_add(bytes)?,
                    self.used_nodes.checked_add(nodes)?,
                ))
            })
            .filter(|&(_, _, used_bytes, used_nodes)| {
                Self::retained_bytes(used_bytes, used_nodes, self.owned_bytes)
                    .is_some_and(|peak| peak <= self.retained_limit)
            });
        let Some((remaining_bytes, remaining_nodes, used_bytes, used_nodes)) = prospective else {
            return Err(AuctionError::BadInput(
                "response bookkeeping exceeds the materialization budget".into(),
            ));
        };
        self.bytes = remaining_bytes;
        self.nodes = remaining_nodes;
        self.used_bytes = used_bytes;
        self.used_nodes = used_nodes;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{BookkeepingBudget, Winner};
    use auctioneer_runtime::ResourceError;

    #[test]
    fn exact_byte_and_node_boundaries_are_accepted() {
        let exact_bytes = 2 * 10 + 128 + 4 + 32;
        let mut budget = BookkeepingBudget::new(exact_bytes, 24).unwrap();
        budget.bidder("fake_short").unwrap();
        budget.nonbid("imp1").unwrap();
        assert!(budget.nonbid("").is_err());
    }

    #[test]
    fn byte_and_node_limits_are_independent() {
        let exact_bytes = 2 * 10 + 128 + 4 + 32;
        BookkeepingBudget::new(exact_bytes - 1, 24)
            .unwrap()
            .bidder("fake_short")
            .unwrap();
        let mut bytes = BookkeepingBudget::new(exact_bytes - 1, 24).unwrap();
        bytes.bidder("fake_short").unwrap();
        assert!(bytes.nonbid("imp1").is_err());
        let mut nodes = BookkeepingBudget::new(exact_bytes, 23).unwrap();
        nodes.bidder("fake_short").unwrap();
        assert!(nodes.nonbid("imp1").is_err());
    }

    #[test]
    fn typed_identities_preserve_independent_wire_metadata_quotas() {
        let id = "i".repeat(200);
        let slots = u64::try_from(size_of::<String>() + size_of::<Option<Winner>>()).unwrap();
        for (bytes, nodes) in [(184, 32), (512, 24)] {
            let mut budget = BookkeepingBudget::new(bytes, nodes).unwrap();
            budget.impression(&id).unwrap();
            budget.selection_names("fake_long", 3).unwrap();
            assert_eq!((budget.bytes, budget.nodes), (bytes, u64::from(nodes)));
            budget.bidder("fake_short").unwrap();
            budget.nonbid("imp1").unwrap();
            assert_eq!(
                budget.retained_peak().unwrap(),
                4 * (184 + 24 * 384) + slots + 200 + 27
            );
            assert_eq!(
                (budget.bytes, budget.nodes),
                (bytes - 184, u64::from(nodes) - 24)
            );
            assert!(budget.nonbid("").is_err());
        }
    }

    #[test]
    fn typed_identities_and_later_metadata_share_the_original_funded_ceiling() {
        for identity_first in [false, true] {
            let mut budget = BookkeepingBudget::new(1, 1).unwrap();
            let id = "i".repeat(1536);
            if identity_first {
                budget.selection_names(&id, 1).unwrap();
                budget.reserve(Some(1), 0).unwrap();
            } else {
                budget.reserve(Some(1), 0).unwrap();
                budget.selection_names(&id, 1).unwrap();
            }
            assert_eq!(budget.retained_peak().unwrap(), 1540);
            assert_eq!((budget.bytes, budget.nodes), (0, 1));
            assert!(budget.selection_names("x", 1).is_err());
            assert!(budget.reserve(Some(0), 1).is_err());
            assert_eq!(budget.retained_peak().unwrap(), 1540);
            assert_eq!((budget.bytes, budget.nodes), (0, 1));
        }
    }

    #[test]
    fn retained_selection_names_scale_with_both_width_and_impression_count() {
        for (name, count, expected) in [("fake_a", 1, 6), ("fake_long", 1, 9), ("fake_a", 4, 24)] {
            let mut budget = BookkeepingBudget::new(0, 1).unwrap();
            budget.selection_names(name, count).unwrap();
            assert_eq!(budget.retained_peak().unwrap(), expected);
            assert_eq!((budget.bytes, budget.nodes), (0, 1));
            budget.reserve_owned(Some(1536 - expected)).unwrap();
            assert!(budget.selection_names("x", 1).is_err());
            assert_eq!(budget.retained_peak().unwrap(), 1536);
        }
        #[cfg(target_pointer_width = "64")]
        {
            let mut budget = BookkeepingBudget::new(0, 1).unwrap();
            assert!(budget.selection_names("fake_", usize::MAX).is_err());
            assert_eq!(budget.retained_peak().unwrap(), 0);
            assert_eq!((budget.bytes, budget.nodes), (0, 1));
        }
    }

    #[test]
    fn retained_impression_identity_and_selection_slots_preflight_the_funded_ceiling() {
        let slots = u64::try_from(size_of::<String>() + size_of::<Option<Winner>>()).unwrap();
        for charge in [1535, 1536, 1537] {
            let mut budget = BookkeepingBudget::new(0, 1).unwrap();
            let id = "i".repeat(usize::try_from(charge - slots).unwrap());
            assert_eq!(budget.impression(&id).is_ok(), charge <= 1536);
            assert_eq!(
                budget.retained_peak().unwrap(),
                if charge <= 1536 { charge } else { 0 }
            );
            assert_eq!((budget.bytes, budget.nodes), (0, 1));
        }
    }

    #[test]
    fn response_id_and_global_fields_reserve_before_copying_at_both_caps() {
        let mut exact = BookkeepingBudget::new(131, 16).unwrap();
        exact.response("abc").unwrap();
        assert_eq!(exact.retained_peak().unwrap(), 4 * (131 + 16 * 384));
        assert!(
            BookkeepingBudget::new(130, 16)
                .unwrap()
                .response("abc")
                .is_err()
        );
        assert!(
            BookkeepingBudget::new(131, 15)
                .unwrap()
                .response("abc")
                .is_err()
        );
    }

    #[test]
    fn global_warnings_are_charged_in_addition_to_response_identity() {
        let mut budget = BookkeepingBudget::new(168, 24).unwrap();
        budget.response("r").unwrap();
        budget.warning("privacy").unwrap();
        assert_eq!(budget.retained_peak().unwrap(), 4 * (168 + 24 * 384));
        assert!(budget.warning("").is_err());
    }

    #[test]
    fn failed_reservation_keeps_authoritative_used_bytes_and_nodes_unchanged() {
        let exact_bytes = 2 * 10 + 128 + 4 + 32;
        let mut budget = BookkeepingBudget::new(exact_bytes, 24).unwrap();
        budget.bidder("fake_short").unwrap();
        let before = budget.retained_peak().unwrap();
        assert!(budget.nonbid("too long").is_err());
        assert_eq!(budget.retained_peak().unwrap(), before);
        budget.nonbid("imp1").unwrap();
        assert_eq!(
            budget.retained_peak().unwrap(),
            4 * (exact_bytes + 24 * 384)
        );
    }

    #[test]
    fn retained_peak_depends_on_reserved_metadata_not_the_available_caps() {
        let mut narrow = BookkeepingBudget::new(512, 64).unwrap();
        let mut wide = BookkeepingBudget::new(32 * 1024, 512).unwrap();
        for budget in [&mut narrow, &mut wide] {
            budget.response("\0\"\\").unwrap();
            budget.bidder("fake_").unwrap();
            budget.nonbid("\n\t").unwrap();
        }
        assert_eq!(narrow.retained_peak(), wide.retained_peak());
        assert_eq!(narrow.retained_peak().unwrap(), 4 * (303 + 40 * 384));
    }

    #[test]
    fn retained_peak_checks_overflow_instead_of_wrapping_the_receipt_check() {
        assert!(matches!(
            BookkeepingBudget::new(u64::MAX, 1),
            Err(ResourceError::CapacityOverflow)
        ));
        let mut budget = BookkeepingBudget::new(0, 1).unwrap();
        budget.reserve_owned(Some(1)).unwrap();
        assert!(budget.reserve_owned(Some(u64::MAX)).is_err());
        assert_eq!(budget.retained_peak().unwrap(), 1);
        assert_eq!((budget.bytes, budget.nodes), (0, 1));
    }
}
