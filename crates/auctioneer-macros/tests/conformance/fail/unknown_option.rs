use auctioneer_macros::OpenRtbFieldMetadata;
#[derive(OpenRtbFieldMetadata)]
#[openrtb_metadata(guess = true)]
struct Root {
    value: String,
}
fn main() {}
