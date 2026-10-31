//! User + Eid.

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use crate::ext::misc::UserExt;
use crate::types::device::Geo;

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct User {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buyeruid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yob: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keywords: Option<String>,
    /// Native 2.6 consent field. The `regs.ext.gdpr` location is rejected;
    /// `user.consent` is the only accepted location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consent: Option<String>,
    /// Native 2.6 EIDs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub eids: Vec<Eid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geo: Option<Geo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<UserExt>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata, name = "EID")]
pub struct Eid {
    /// Optional — some legacy fixtures emit `eids` entries with no `source`
    /// (e.g. sample `user-eids-source-empty.json`); the value is
    /// preserved through round-trip when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// `Option<Vec<...>>` so `"uids": []` round-trips byte-for-byte
    /// (`user-eids-uids-empty.json` deliberately ships an empty
    /// uids array as a privacy/processing signal).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uids: Option<Vec<EidUid>>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata, name = "UID")]
pub struct EidUid {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atype: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<Box<RawValue>>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::round_trip;

    #[test]
    fn user_consent_round_trips_in_native_location() {
        let json =
            serde_json::json!({"consent":"CON","eids":[{"source":"x","uids":[{"id":"u1"}]}]});
        let user: User = round_trip(&json);
        assert_eq!(user.consent.as_deref(), Some("CON"));
        assert_eq!(user.eids[0].source.as_deref(), Some("x"));
        assert_eq!(user.eids[0].uids.as_ref().unwrap().len(), 1);
    }
}
