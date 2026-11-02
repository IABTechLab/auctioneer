use auctioneer_macros::OpenRtbFieldMetadata;

type Opaque = opaque!();

#[derive(OpenRtbFieldMetadata)]
struct Root {
    value: Opaque,
}

fn main() {}
