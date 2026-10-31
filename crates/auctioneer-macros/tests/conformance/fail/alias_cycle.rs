use auctioneer_macros::OpenRtbFieldMetadata;
type First = Option<Box<Second>>;
type Second = Vec<First>;
#[derive(OpenRtbFieldMetadata)]
struct Root {
    value: First,
}
fn main() {}
