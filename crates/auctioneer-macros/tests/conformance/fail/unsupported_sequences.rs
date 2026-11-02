use auctioneer_macros::OpenRtbFieldMetadata;
use serde_json::value::RawValue;

#[derive(OpenRtbFieldMetadata)]
struct Root {
    typed_array: [String; 2],
    raw_array: [Box<RawValue>; 2],
    typed_boxed_slice: Box<[String]>,
    raw_boxed_slice: Box<[Box<RawValue>]>,
    typed_borrowed_slice: &'static [String],
    raw_borrowed_slice: &'static [Box<RawValue>],
}

fn main() {}
