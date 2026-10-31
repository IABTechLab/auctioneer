//! Small ext structs that don't warrant their own file.

use serde::{Deserialize, Serialize};

use crate::types::field::JsonField;

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct SiteExt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amp: Option<i32>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct PublisherExt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prebid: Option<PublisherExtPrebid>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct DeviceExt {
    /// iOS App Tracking Transparency status. Read-only — drives the iOS LMT
    /// override in `prepare()`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atts: Option<i32>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub prebid: JsonField<DeviceExtPrebid>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct DeviceExtPrebid {
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub interstitial: JsonField<DeviceInterstitial>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct DeviceInterstitial {
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub minwidthperc: JsonField<i64>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub minheightperc: JsonField<i64>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct RegsExt {
    /// `regs.ext.dsa` — typed pass-through (typed only, no DSA behavior
    /// in phase 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dsa: Option<serde_json::Value>,
    /// `regs.ext.gpc` — populated by `prepare()` from `Sec-GPC: 1`.
    /// Outbound forwarded when "1".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpc: Option<String>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct SourceExt {
    /// `source.ext.schain` — typed pass-through.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schain: Option<serde_json::Value>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct UserExt {
    /// `user.ext.data` - first-party data governed by `transmitUfpd`,
    /// independently of GDPR's identifier-only `PassID` permission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// `user.ext.eids` — legacy 2.5 location. Legacy 2.5 privacy fields
    /// are rejected, but `eids` itself is not privacy and may live here
    /// for passthrough. Scrubbed by `scrub_gdpr_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eids: Option<serde_json::Value>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct PublisherExtPrebid {
    /// `site/app/dooh.publisher.ext.prebid.parentAccount` — drives account
    /// resolution.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "parentAccount"
    )]
    pub parent_account: Option<String>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}
