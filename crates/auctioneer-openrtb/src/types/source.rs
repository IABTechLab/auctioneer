//! Source — supply-chain provenance + transaction id.

use serde::{Deserialize, Serialize};

use crate::ext::misc::SourceExt;

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct Source {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fd: Option<i32>,
    /// Native 2.6 transaction id; scrubbed by `transmitTid-denied`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pchain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<SourceExt>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::round_trip;

    #[test]
    fn source_round_trips_with_tid_and_schain() {
        let json = serde_json::json!({"tid":"t1","ext":{"schain":{"ver":"1.0","complete":1_i32}}});
        let source: Source = round_trip(&json);
        assert_eq!(source.tid.as_deref(), Some("t1"));
        assert!(source.ext.unwrap().schain.is_some());
    }
}
