use auctioneer_macros::OpenRtbFieldMetadata;

#[derive(OpenRtbFieldMetadata)]
struct Root {
    #[serde(flatten)]
    other: serde_json::Map<String, String>,
}

fn main() {}
