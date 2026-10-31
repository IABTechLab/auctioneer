use auctioneer_macros::OpenRtbFieldMetadata;
#[derive(OpenRtbFieldMetadata)]
struct Root {
    #[serde(flatten)]
    other: std::collections::BTreeMap<String, String>,
}
fn main() {}
