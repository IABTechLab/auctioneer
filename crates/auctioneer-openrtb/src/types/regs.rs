//! Regs — privacy regulation signals.

use serde::{Deserialize, Serialize};

use crate::ext::misc::RegsExt;

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct Regs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coppa: Option<i32>,
    /// Native 2.6 GDPR signal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gdpr: Option<i32>,
    /// Native 2.6 US-Privacy.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "us_privacy"
    )]
    pub us_privacy: Option<String>,
    /// Native 2.6 GPP string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpp: Option<String>,
    /// Native 2.6 GPP section IDs. Modeled as
    /// `Option<Vec<...>>` to preserve the absent-vs-empty distinction
    /// some fixtures rely on (e.g. `empty-gppsid-conflict.json`
    /// emits `"gpp_sid": []` deliberately).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpp_sid: Option<Vec<i32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<RegsExt>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::round_trip;

    #[test]
    fn regs_ext_gpc_round_trips() {
        let json = serde_json::json!({"ext":{"gpc":"1"}});
        let regs: Regs = round_trip(&json);
        assert_eq!(regs.ext.unwrap().gpc.as_deref(), Some("1"));
    }

    #[test]
    fn regs_native_fields_round_trip() {
        let json = serde_json::json!({"gdpr":1_i32,"us_privacy":"1YNN","gpp":"DBABMA","gpp_sid":[2_i32,6_i32]});
        let regs: Regs = round_trip(&json);
        assert_eq!(regs.gdpr, Some(1_i32));
        assert_eq!(regs.us_privacy.as_deref(), Some("1YNN"));
        assert_eq!(regs.gpp_sid, Some(vec![2_i32, 6_i32]));
    }
}
