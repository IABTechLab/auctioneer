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
}

impl BookkeepingBudget {
    pub(crate) fn new(bytes: u64, nodes: u32) -> Self {
        Self {
            bytes,
            nodes: u64::from(nodes),
            used_bytes: 0,
            used_nodes: 0,
        }
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
        self.reserve(bytes, 8)
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
        self.reserve(bytes, 0)
    }

    /// Four coexisting metadata representations, bounded independently of calls
    /// and bidder-body parsing. Unrelated request/FPD bytes are not metadata.
    pub(crate) fn retained_peak(&self) -> Result<u64, ResourceError> {
        self.used_nodes
            .checked_mul(NODE_RETAINED_BYTES)
            .and_then(|nodes| nodes.checked_add(self.used_bytes))
            .and_then(|bytes| bytes.checked_mul(4))
            .ok_or(ResourceError::CapacityOverflow)
    }

    fn reserve(&mut self, charge: Option<u64>, nodes: u64) -> Result<(), AuctionError> {
        let prospective = charge.and_then(|bytes| {
            Some((
                self.bytes.checked_sub(bytes)?,
                self.nodes.checked_sub(nodes)?,
                self.used_bytes.checked_add(bytes)?,
                self.used_nodes.checked_add(nodes)?,
            ))
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
        let mut budget = BookkeepingBudget::new(exact_bytes, 24);
        budget.bidder("fake_short").unwrap();
        budget.nonbid("imp1").unwrap();
        assert!(budget.nonbid("").is_err());
    }

    #[test]
    fn byte_and_node_limits_are_independent() {
        let exact_bytes = 2 * 10 + 128 + 4 + 32;
        BookkeepingBudget::new(exact_bytes - 1, 24)
            .bidder("fake_short")
            .unwrap();
        let mut bytes = BookkeepingBudget::new(exact_bytes - 1, 24);
        bytes.bidder("fake_short").unwrap();
        assert!(bytes.nonbid("imp1").is_err());
        let mut nodes = BookkeepingBudget::new(exact_bytes, 23);
        nodes.bidder("fake_short").unwrap();
        assert!(nodes.nonbid("imp1").is_err());
    }

    #[test]
    fn retained_selection_names_scale_with_both_width_and_impression_count() {
        for (name, count, expected) in [("fake_a", 1, 6), ("fake_long", 1, 9), ("fake_a", 4, 24)] {
            let mut budget = BookkeepingBudget::new(expected, 1);
            budget.selection_names(name, count).unwrap();
            assert_eq!(budget.retained_peak().unwrap(), expected * 4);
            assert!(budget.selection_names("x", 1).is_err());
            assert!(
                BookkeepingBudget::new(expected - 1, 1)
                    .selection_names(name, count)
                    .is_err()
            );
        }
        #[cfg(target_pointer_width = "64")]
        assert!(
            BookkeepingBudget::new(u64::MAX, 1)
                .selection_names("fake_", usize::MAX)
                .is_err()
        );
    }

    #[test]
    fn retained_impression_identity_and_selection_slots_preflight_both_caps() {
        let slots = u64::try_from(size_of::<String>() + size_of::<Option<Winner>>()).unwrap();
        let mut exact = BookkeepingBudget::new(slots + 3, 8);
        exact.impression("abc").unwrap();
        assert_eq!(exact.retained_peak().unwrap(), 4 * (slots + 3 + 8 * 384));
        assert!(
            BookkeepingBudget::new(slots + 2, 8)
                .impression("abc")
                .is_err()
        );
        assert!(
            BookkeepingBudget::new(slots + 3, 7)
                .impression("abc")
                .is_err()
        );
    }

    #[test]
    fn response_id_and_global_fields_reserve_before_copying_at_both_caps() {
        let mut exact = BookkeepingBudget::new(131, 16);
        exact.response("abc").unwrap();
        assert_eq!(exact.retained_peak().unwrap(), 4 * (131 + 16 * 384));
        assert!(BookkeepingBudget::new(130, 16).response("abc").is_err());
        assert!(BookkeepingBudget::new(131, 15).response("abc").is_err());
    }

    #[test]
    fn global_warnings_are_charged_in_addition_to_response_identity() {
        let mut budget = BookkeepingBudget::new(168, 24);
        budget.response("r").unwrap();
        budget.warning("privacy").unwrap();
        assert_eq!(budget.retained_peak().unwrap(), 4 * (168 + 24 * 384));
        assert!(budget.warning("").is_err());
    }

    #[test]
    fn failed_reservation_keeps_authoritative_used_bytes_and_nodes_unchanged() {
        let exact_bytes = 2 * 10 + 128 + 4 + 32;
        let mut budget = BookkeepingBudget::new(exact_bytes, 24);
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
        let mut narrow = BookkeepingBudget::new(512, 64);
        let mut wide = BookkeepingBudget::new(32 * 1024, 512);
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
        let mut budget = BookkeepingBudget::new(u64::MAX, 1);
        budget.reserve(Some(u64::MAX), 1).unwrap();
        assert_eq!(budget.retained_peak(), Err(ResourceError::CapacityOverflow));
    }
}
