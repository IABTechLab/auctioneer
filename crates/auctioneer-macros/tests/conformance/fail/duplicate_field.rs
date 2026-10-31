use auctioneer_macros::OpenRtbFieldMetadata;
#[derive(OpenRtbFieldMetadata)]
struct Root {
    id: String,
    #[serde(rename = "id")]
    other: String,
}
fn main() {}
