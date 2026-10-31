//! `bidrequest.ext.prebid` — typed Prebid extensions.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::field_metadata::{FieldMetadata, Storage};

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct ExtRequestPrebid {
    /// Request alias map: alias -> base bidder. Resolved in `run_auction`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub aliases: BTreeMap<String, String>,
    /// GVL vendor id overrides for aliases.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub aliasgvlids: BTreeMap<String, u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targeting: Option<ExtRequestTargeting>,
    /// `test=1` or `ext.prebid.debug=1` contributes to `debug_requested`.
    /// The wire shape accepts both `1` (int per `OpenRTB` schema) and
    /// `true` (bool via a custom unmarshaller); `DebugSignal`
    /// round-trips whichever form the caller sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug: Option<DebugSignal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<Channel>,
    /// No-sale list for CCPA — `["*"]` = all bidders.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nosale: Vec<String>,
    /// Request-level bidder params; merged into `imp.ext.<bidder>`
    /// (supplement-only, never creates participation).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bidderparams: BTreeMap<String, serde_json::Value>,
    /// Controls whether `ext.prebid.seatnonbid` is emitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returnallbidstatus: Option<bool>,
    /// `multibid` — typed pass-through; bid-limiting deferred.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multibid: Option<serde_json::Value>,
    /// `ext.prebid.data` — typed pass-through; FPD resolution deferred.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// `ext.prebid.bidderconfig` — typed pass-through.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bidderconfig: Option<serde_json::Value>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct ExtRequestTargeting {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricegranularity: Option<serde_json::Value>,
    /// When `ext.prebid.targeting` is present, defaults to true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub includewinners: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub includebidderkeys: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub includeformat: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferdeals: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alwaysincludedeals: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

/// `ext.prebid.debug` wire shape — accepts both int and bool via a
/// custom unmarshaller. The untagged enum lets serde pick the right
/// variant on parse and round-trip the same form on serialize.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DebugSignal {
    Int(i32),
    Bool(bool),
}

impl FieldMetadata for DebugSignal {
    const STORAGE: Storage = Storage::Typed;
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct Channel {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::round_trip;

    #[test]
    fn debug_signal_metadata_and_serde_agree_that_both_variants_are_scalar() {
        assert_eq!(DebugSignal::STORAGE, Storage::Typed);
        assert!(DebugSignal::MODEL.is_none());
        assert_eq!(
            serde_json::to_value(DebugSignal::Int(1)).unwrap(),
            serde_json::json!(1_i32)
        );
        assert_eq!(
            serde_json::to_value(DebugSignal::Bool(true)).unwrap(),
            serde_json::json!(true)
        );
    }

    #[test]
    fn aliases_and_aliasgvlids_round_trip() {
        let json = serde_json::json!({"aliases":{"fake_alias":"fake_bidder"},"aliasgvlids":{"fake_alias":17_i32}});
        let ext: ExtRequestPrebid = round_trip(&json);
        assert_eq!(
            ext.aliases.get("fake_alias").map(String::as_str),
            Some("fake_bidder"),
        );
        assert_eq!(ext.aliasgvlids.get("fake_alias"), Some(&17_u32));
    }

    #[test]
    fn bidderparams_round_trip() {
        let json = serde_json::json!({"bidderparams":{"fake_bidder":{"testParam":"123"}}});
        let ext: ExtRequestPrebid = round_trip(&json);
        assert!(ext.bidderparams.contains_key("fake_bidder"));
    }

    #[test]
    fn debug_accepts_int_one() {
        let json = serde_json::json!({"debug":1_i32});
        let ext: ExtRequestPrebid = round_trip(&json);
        assert_eq!(ext.debug, Some(DebugSignal::Int(1_i32)));
    }

    #[test]
    fn debug_accepts_bool_true() {
        // Some fixtures (e.g. all-ext.json) emit `"debug": true` via a
        // custom unmarshaller. The untagged enum keeps the wire shape
        // on round-trip.
        let json = serde_json::json!({"debug":true});
        let ext: ExtRequestPrebid = round_trip(&json);
        assert_eq!(ext.debug, Some(DebugSignal::Bool(true)));
    }

    #[test]
    fn channel_round_trips() {
        let json = serde_json::json!({"channel":{"name":"app","version":"1.0"}});
        let ext: ExtRequestPrebid = round_trip(&json);
        assert_eq!(ext.channel.unwrap().name.as_deref(), Some("app"));
    }

    #[test]
    fn passthrough_fields_round_trip() {
        let json = serde_json::json!({"multibid":[{"maxBids":2_i32}],"data":{"x":1_i32},"bidderconfig":[]});
        let _ext: ExtRequestPrebid = round_trip(&json);
    }
}
