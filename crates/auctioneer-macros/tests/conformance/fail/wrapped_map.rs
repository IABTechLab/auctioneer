use auctioneer_macros::OpenRtbFieldMetadata;
#[derive(OpenRtbFieldMetadata)]
struct Root {
    #[serde(flatten)]
    other: Option<serde_json::Map<String, serde_json::Value>>,
}
fn main() {}
