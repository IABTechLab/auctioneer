use auctioneer_macros::OpenRtbFieldMetadata;
#[derive(OpenRtbFieldMetadata)]
struct Root {
    #[serde(rename(serialize = "out", deserialize = "in"))]
    value: String,
}
fn main() {}
