//! Inventory — `Site`, `App`, `Dooh`, `Publisher`.

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use crate::ext::misc::{PublisherExt, SiteExt};

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct Site {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<Publisher>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<SiteExt>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct App {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<Publisher>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<Box<RawValue>>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct Dooh {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<Publisher>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<Box<RawValue>>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct Publisher {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<PublisherExt>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::round_trip;

    #[test]
    fn site_amp_round_trips() {
        let json = serde_json::json!({"id":"s1","ext":{"amp":1_i32}});
        let site: Site = round_trip(&json);
        assert_eq!(site.ext.unwrap().amp, Some(1_i32));
    }

    #[test]
    fn site_with_publisher_parent_account_round_trips() {
        let json = serde_json::json!({
            "id": "s1",
            "publisher": {"id": "p1", "ext": {"prebid": {"parentAccount": "acct-9"}}}
        });
        let site: Site = round_trip(&json);
        let parent = site
            .publisher
            .unwrap()
            .ext
            .unwrap()
            .prebid
            .unwrap()
            .parent_account;
        assert_eq!(parent.as_deref(), Some("acct-9"));
    }
}
