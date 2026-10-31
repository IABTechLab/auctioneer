use auctioneer_macros::OpenRtbFieldMetadata;

#[derive(OpenRtbFieldMetadata)]
struct Child {
    id: String,
}

#[derive(OpenRtbFieldMetadata)]
struct Root {
    #[serde(flatten)]
    other: Child,
}

fn main() {}
