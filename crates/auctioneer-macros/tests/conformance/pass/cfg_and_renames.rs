use auctioneer_macros::OpenRtbFieldMetadata;
use auctioneer_openrtb::field_metadata::FieldMetadata;

#[derive(OpenRtbFieldMetadata)]
#[serde(default)]
struct Root {
    #[cfg(any())]
    unavailable: TypeWhichDoesNotExist,
    #[cfg_attr(all(), serde(rename = "wire"))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    r#type: Option<String>,
    #[cfg(all())]
    enabled: bool,
}

fn main() {
    let fields = Root::MODEL.unwrap().fields;
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "wire");
    assert_eq!(fields[1].name, "enabled");
}
