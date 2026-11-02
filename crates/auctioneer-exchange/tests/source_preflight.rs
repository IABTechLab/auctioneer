//! Borrowed source admission precedes recursive serialization and copying.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

#[cfg(test)]
use crate::fake_bidder::FakeBidder;

#[cfg(test)]
use auctioneer_bidder::BidderRegistry;

#[cfg(test)]
fn registry() -> BidderRegistry {
    let mut registry = BidderRegistry::new();
    registry
        .register(fake_bidder::info(), Box::new(FakeBidder::default()))
        .expect("unique test bidder");
    registry
}

#[cfg(test)]
mod tests {
    use super::registry;
    use auctioneer_exchange::context::RequestConstructionLimits;
    use auctioneer_exchange::{AliasMap, split::group_by_bidder};
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};
    use std::collections::BTreeSet;

    #[test]
    fn grouping_rejects_overdeep_borrowed_source_before_serialization() {
        let registry = registry();
        for (depth, admitted) in [(127_i32, true), (128_i32, true), (129_i32, false)] {
            let mut request: BidRequest = serde_json::from_value(json!({
                "id": "request", "site": {"id": "publisher"},
                "imp": [{"id": "imp", "banner": {}, "ext": {"bidder": {"fake_bb": {}}}}]
            }))
            .unwrap();
            let mut value = Value::Null;
            for _ in 1_i32..depth {
                value = Value::Array(vec![value]);
            }
            request.other.insert("opaque".into(), value);
            let result = group_by_bidder(
                &request,
                &AliasMap::default(),
                &registry,
                &BTreeSet::new(),
                &BTreeSet::new(),
            );
            assert_eq!(result.is_ok(), admitted, "container depth {depth}");
            // The rejected tree belongs to the caller, including its cleanup.
            let mut remaining_tree = request.other.remove("opaque").unwrap();
            while let Value::Array(mut children) = remaining_tree {
                remaining_tree = children.pop().unwrap();
            }
        }
    }

    #[test]
    fn borrowed_source_refuses_nodes_and_wire_bytes_independently_without_mutation() {
        let request: BidRequest = serde_json::from_value(json!({
            "id": "request", "site": {"id": "publisher"},
            "imp": [{"id": "imp", "banner": {}, "ext": {"bidder": {"fake_bb": {}}}}],
            "opaque": {"key": "\u{0}\u{0}"}
        }))
        .unwrap();
        let before = serde_json::to_vec(&request).unwrap();
        let nodes = u64::from(auctioneer_runtime::count_json_nodes(&before, u32::MAX).unwrap());
        let bytes = u64::try_from(before.len()).unwrap();
        for (max_nodes, max_serialized_bytes, fits) in [
            (nodes, bytes, true),
            (nodes.checked_sub(1).unwrap(), bytes, false),
            (nodes, bytes.checked_sub(1).unwrap(), false),
        ] {
            let limits = RequestConstructionLimits {
                max_nodes,
                max_serialized_bytes,
                reusable_scratch_bytes: 0,
                retained_per_scope_bytes: 0,
            };
            assert_eq!(limits.ensure(&request).is_ok(), fits);
            assert_eq!(serde_json::to_vec(&request).unwrap(), before);
        }
    }
}
