//! Bid-response ext types.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExtBid {
    /// `origbidcpm` — set when currency conversion would have applied (phase 1
    /// has none, so adapters may emit this directly).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origbidcpm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origbidcur: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prebid: Option<ExtBidPrebid>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExtBidPrebid {
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub bid_type: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub targeting: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bidid: Option<String>,
    /// `ext.prebid.dealpriority` — bidder-supplied tie-break weight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dealpriority: Option<i64>,
    /// `ext.prebid.dealtiersatisfied` — true when this bid satisfied the
    /// requested deal-tier minimum.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dealtiersatisfied: Option<bool>,
    /// `ext.prebid.targetbiddercode` — concrete bidder name to use when
    /// emitting targeting keys for an alias winner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targetbiddercode: Option<String>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::round_trip;
    use serde_json::error::Category;

    #[test]
    fn ext_bid_prebid_typed_fields_round_trip() {
        let json = serde_json::json!({"dealpriority":5_i32,"dealtiersatisfied":true,"targetbiddercode":"fake_bidder"});
        let ext: ExtBidPrebid = round_trip(&json);
        assert_eq!(ext.dealpriority, Some(5_i64));
        assert_eq!(ext.dealtiersatisfied, Some(true));
        assert_eq!(ext.targetbiddercode.as_deref(), Some("fake_bidder"));
    }

    #[test]
    fn deal_priority_round_trips_the_full_signed_integer_domain_in_the_typed_slot() {
        for priority in [
            i64::MIN,
            -0x8000_0001_i64,
            -1_i64,
            0_i64,
            0x8000_0000_i64,
            i64::MAX,
        ] {
            let input = serde_json::json!({"dealpriority":priority});
            let parsed: ExtBidPrebid = serde_json::from_value(input.clone()).unwrap();
            assert!(!parsed.other.contains_key("dealpriority"));
            assert_eq!(serde_json::to_value(parsed).unwrap(), input);
        }
    }

    #[test]
    fn deal_priority_stays_a_strict_optional_signed_integer() {
        for priority in [
            serde_json::json!(u64::MAX),
            serde_json::json!("5"),
            serde_json::json!(1.5_f64),
            serde_json::json!(true),
            serde_json::json!([]),
            serde_json::json!({}),
        ] {
            let input = serde_json::json!({"dealpriority":priority});
            assert_eq!(
                serde_json::from_value::<ExtBidPrebid>(input)
                    .unwrap_err()
                    .classify(),
                Category::Data,
            );
        }
        for input in [
            serde_json::json!({}),
            serde_json::json!({"dealpriority":null}),
        ] {
            let parsed: ExtBidPrebid = serde_json::from_value(input).unwrap();
            assert!(parsed.dealpriority.is_none());
            assert!(!parsed.other.contains_key("dealpriority"));
        }
    }
}
