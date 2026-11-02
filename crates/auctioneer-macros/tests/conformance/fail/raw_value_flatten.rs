use auctioneer_macros::OpenRtbFieldMetadata;

#[derive(OpenRtbFieldMetadata)]
struct Root {
    #[serde(flatten)]
    other: Box<serde_json::value::RawValue>,
}

fn main() {}
