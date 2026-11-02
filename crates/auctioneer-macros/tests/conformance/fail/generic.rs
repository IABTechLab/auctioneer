use auctioneer_macros::OpenRtbFieldMetadata;
#[derive(OpenRtbFieldMetadata)]
struct Root<T> {
    value: T,
}
fn main() {}
