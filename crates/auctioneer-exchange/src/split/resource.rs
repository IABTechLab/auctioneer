//! Charge copy amplification before any survivor is materialized.

use super::GroupedBidders;
use crate::AuctionError;
use auctioneer_bidder::BidderName;
use auctioneer_runtime::json_preflight::{MAX_CONSTRUCTION_DEPTH, preflight};
use auctioneer_runtime::{
    ChargeKind, ResourceError, budget::TYPED_CLONE_FACTOR, serialized_json_bytes,
};
use serde_json::Value;

impl GroupedBidders {
    /// Bound each survivor's repeated request-level supplement before copying.
    /// Input params and FPD are already bounded input subtrees; supplements
    /// repeat once per impression whose selected params are an object.
    ///
    /// # Errors
    /// Returns code 2 when supplement content bytes or nodes exceed the
    /// materialization limits, or `Internal` for inconsistent grouping.
    pub fn ensure_supplement_capacity(
        &self,
        survivors: &[BidderName],
        max_nodes: u32,
        max_bytes: usize,
    ) -> Result<(), AuctionError> {
        for bidder in survivors {
            let indices = self
                .per_bidder_imps
                .get(bidder)
                .ok_or_else(|| AuctionError::Internal("ungrouped bidder selected".into()))?;
            let name = bidder.as_str();
            let Some(supplement) = self
                .request_value
                .pointer("/ext/prebid/bidderparams")
                .and_then(|params| params.get(name))
                .filter(|value| value.is_object())
            else {
                continue;
            };
            let mut nodes_left = u64::from(max_nodes);
            let mut bytes_left = max_bytes;
            for &index in indices {
                let imp = self
                    .request_value
                    .get("imp")
                    .and_then(|imps| imps.get(index))
                    .ok_or_else(|| {
                        AuctionError::Internal("grouped impression index out of bounds".into())
                    })?;
                let ext = imp.get("ext").unwrap_or(&Value::Null);
                let params = ext
                    .get("bidder")
                    .and_then(|params| params.get(name))
                    .or_else(|| ext.get("prebid")?.get("bidder")?.get(name))
                    .or_else(|| ext.get(name));
                if params.is_some_and(Value::is_object)
                    && !charge_supplement(supplement, &mut nodes_left, &mut bytes_left)
                {
                    return Err(AuctionError::BadInput(
                        "bidder-scoped supplement exceeds the materialization budget".into(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Peak allowance for selected projections, including parameter supplements
    /// copied into every participating impression, selected parameter scratch,
    /// and two transient FPD copies.
    /// The original request-level data is retained separately by each survivor.
    ///
    /// # Errors
    /// Returns a terminal capacity error on overflow or inconsistent grouping.
    pub fn projection_charge(&self, survivors: &[BidderName]) -> Result<u64, ResourceError> {
        let overflow = ResourceError::CapacityOverflow;
        let base = serialized_json_bytes(&self.request_value).map_err(|_error| overflow)?;
        let count = u64::try_from(survivors.len()).map_err(|_error| overflow)?;
        let mut expanded = base.checked_mul(count).ok_or(overflow)?;
        for bidder in survivors {
            let indices = self.per_bidder_imps.get(bidder).ok_or(overflow)?;
            let name = bidder.as_str();
            if let Some(supplement) = self
                .request_value
                .get("ext")
                .and_then(|ext| ext.get("prebid"))
                .and_then(|prebid| prebid.get("bidderparams"))
                .and_then(|params| params.get(name))
            {
                let bytes = serialized_json_bytes(supplement).map_err(|_error| overflow)?;
                let copies = u64::try_from(indices.len()).map_err(|_error| overflow)?;
                expanded = expanded
                    .checked_add(bytes.checked_mul(copies).ok_or(overflow)?)
                    .ok_or(overflow)?;
            }
            for &index in indices {
                let imp = self
                    .request_value
                    .get("imp")
                    .and_then(|imps| imps.get(index))
                    .ok_or(overflow)?;
                let imp_ext = imp.get("ext");
                let selected_params = imp_ext
                    .and_then(|ext| ext.get("bidder"))
                    .and_then(|params| params.get(name))
                    .or_else(|| imp_ext?.get("prebid")?.get("bidder")?.get(name))
                    .or_else(|| imp_ext?.get(name));
                // Materialization clones the selected parameters before
                // cloning the impression that still contains their source.
                if let Some(params) = selected_params {
                    let bytes = serialized_json_bytes(params).map_err(|_error| overflow)?;
                    expanded = expanded.checked_add(bytes).ok_or(overflow)?;
                }
                if let Some(patch) = imp
                    .get("ext")
                    .and_then(|ext| ext.get("prebid"))
                    .and_then(|prebid| prebid.get("imp"))
                    .and_then(|patches| patches.get(name))
                {
                    let bytes = serialized_json_bytes(patch).map_err(|_error| overflow)?;
                    expanded = expanded
                        .checked_add(bytes.checked_mul(2).ok_or(overflow)?)
                        .ok_or(overflow)?;
                }
            }
        }
        expanded.checked_mul(TYPED_CLONE_FACTOR).ok_or(overflow)
    }

    /// Check the prepaid projection allowance without allocating.
    ///
    /// # Errors
    /// Returns a terminal resource refusal before projection can exceed its
    /// allowance, or a checked arithmetic failure.
    pub fn ensure_projection_capacity(
        &self,
        survivors: &[BidderName],
        capacity: u64,
    ) -> Result<(), ResourceError> {
        if self.projection_charge(survivors)? > capacity {
            return Err(ResourceError::Exhausted {
                kind: ChargeKind::Projection,
            });
        }
        Ok(())
    }
}

/// Occupied keys can reduce the eventual merge, never increase this bound.
/// Params occupy request/imp-array/imp/ext before their root container. Source
/// supplements have the same depth; FPD moves from depth seven to depth three.
/// Merging corresponding keys never concatenates either tree's depth.
fn charge_supplement(value: &Value, nodes_left: &mut u64, bytes_left: &mut usize) -> bool {
    let Ok(stats) = preflight(value, *nodes_left, MAX_CONSTRUCTION_DEPTH - 4) else {
        return false;
    };
    let Ok(wire_bytes) = serialized_json_bytes(value) else {
        return false;
    };
    let Ok(bytes) = usize::try_from(wire_bytes) else {
        return false;
    };
    let Some(remaining_bytes) = bytes_left.checked_sub(bytes) else {
        return false;
    };
    let Some(remaining_nodes) = nodes_left.checked_sub(stats.nodes) else {
        return false;
    };
    *nodes_left = remaining_nodes;
    *bytes_left = remaining_bytes;
    true
}

#[cfg(test)]
mod tests {
    use super::charge_supplement;

    #[test]
    fn supplement_preflight_charges_keys_containers_and_utf8_bytes() {
        let supplement = serde_json::json!({"long": ["\u{00e9}", {}, null, true, 3_i32]});
        let wire_bytes = serde_json::to_vec(&supplement).unwrap().len();
        for (nodes, bytes, fits) in [
            (8, wire_bytes, true),
            (7, wire_bytes, false),
            (8, wire_bytes - 1, false),
            (1, 100, false),
        ] {
            let mut nodes_left = nodes;
            let mut bytes_left = bytes;
            assert_eq!(
                charge_supplement(&supplement, &mut nodes_left, &mut bytes_left),
                fits
            );
        }
    }

    #[test]
    fn supplement_preflight_charges_escaped_wire_bytes_not_only_contents() {
        let supplement = serde_json::json!({"key": "\u{0}\u{0}\u{0}"});
        let wire_bytes = serde_json::to_vec(&supplement).unwrap().len();
        let mut nodes_left = 3;
        let mut bytes_left = wire_bytes - 1;
        assert!(!charge_supplement(
            &supplement,
            &mut nodes_left,
            &mut bytes_left
        ));
    }
}
