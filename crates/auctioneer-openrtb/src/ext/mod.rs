//! Ext-field types.

pub mod bid;
pub mod imp;
pub mod misc;
pub mod request_prebid;

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

/// Passthrough ext for fields auctioneer never inspects. Round-trips losslessly.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Passthrough(pub Option<Box<RawValue>>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_absent_serializes_to_null() {
        // Absent (None) serializes to null; this is OK because all call-sites
        // wrap Passthrough fields in #[serde(skip_serializing_if = "Option::is_none")]
        // on the containing struct.
        let absent = Passthrough(None);
        assert_eq!(serde_json::to_string(&absent).unwrap(), "null");
    }

    #[test]
    fn passthrough_round_trips_arbitrary_json() {
        let json = serde_json::json!({"weird":[1_i32,{"nested":true},null]});
        let parsed: Passthrough = serde_json::from_value(json.clone()).unwrap();
        // Compare as Value to avoid whitespace/key-order quibbles.
        assert_eq!(serde_json::to_value(&parsed).unwrap(), json);
    }
}
