//! `BidRequest` and its impression-level children.
//!
//! Most field names are single-letter `OpenRTB` protocol slugs (`w`, `h`, `id`).
//! These are wire-level identifiers, not Rust naming choices.

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use crate::ext::imp::ImpExt;
use crate::ext::request_prebid::ExtRequestPrebid;
use crate::types::device::Device;
use crate::types::field::JsonField;
use crate::types::inventory::{App, Dooh, Site};
use crate::types::regs::Regs;
use crate::types::source::Source;
use crate::types::user::User;

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct BidRequest {
    pub id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imp: Vec<Imp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<Site>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<App>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dooh: Option<Dooh>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<Device>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regs: Option<Regs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmax: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cur: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<BidRequestExt>,
    /// Top-level `OpenRTB` fields auctioneer doesn't yet type (`bcat`,
    /// `wseat`, `badv`, `bapp`, `wlang`, etc.). Preserved verbatim
    /// through round-trip; adapters that need them can read this map
    /// directly until the field gets promoted to a typed slot.
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct BidRequestExt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prebid: Option<ExtRequestPrebid>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct Imp {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner: Option<Banner>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video: Option<Video>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native: Option<Native>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<Audio>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bidfloor: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bidfloorcur: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secure: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<ImpExt>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct Banner {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub format: Vec<Format>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<Box<RawValue>>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct Format {
    pub w: i32,
    pub h: i32,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct Video {
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub mimes: JsonField<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minduration: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxduration: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<Box<RawValue>>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct Native {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ver: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<Box<RawValue>>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct Audio {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mimes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minduration: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxduration: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<Box<RawValue>>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::round_trip;
    use serde_json::error::Category;

    #[test]
    fn minimal_bidrequest_round_trips() {
        let json =
            serde_json::json!({"id":"r1","imp":[{"id":"i1","banner":{"w":300_i32,"h":250_i32}}]});
        let req: BidRequest = round_trip(&json);
        assert_eq!(req.id, "r1");
        assert_eq!(req.imp.len(), 1);
        assert_eq!(req.imp[0].banner.as_ref().unwrap().w, Some(300_i32));
    }

    #[test]
    fn unknown_top_level_ext_keys_round_trip_via_other_map() {
        // imp omitted from the JSON because `imp: Vec<Imp>` skips serializing
        // when empty — round_trip would see drift in the Value comparison
        // otherwise. Round-trip discipline is the test's purpose, not imp presence.
        let json = serde_json::json!({"id":"r1","ext":{"x":1_i32,"y":"z"}});
        let req: BidRequest = round_trip(&json);
        let ext = req.ext.as_ref().unwrap();
        assert!(ext.other.contains_key("x"));
        assert!(ext.other.contains_key("y"));
    }

    #[test]
    fn video_mimes_presence_survives_typed_round_trip() {
        for input in [
            serde_json::json!({}),
            serde_json::json!({"mimes": null}),
            serde_json::json!({"mimes": []}),
            serde_json::json!({"mimes": ["video/mp4"]}),
        ] {
            let video: Video = serde_json::from_value(input.clone()).unwrap();
            assert_eq!(serde_json::to_value(&video).unwrap(), input);
        }
        for malformed in [serde_json::json!(false), serde_json::json!("video/mp4")] {
            let error = serde_json::from_value::<Video>(serde_json::json!({"mimes": malformed}))
                .unwrap_err();
            assert_eq!(error.classify(), Category::Data);
        }
    }
}
