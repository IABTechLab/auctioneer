use auctioneer_macros::OpenRtbFieldMetadata;
use auctioneer_openrtb::types::field::JsonField;

#[derive(OpenRtbFieldMetadata)]
struct Root {
    #[serde(flatten)]
    other: JsonField<serde_json::Map<String, serde_json::Value>>,
}

fn main() {}
