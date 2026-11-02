use auctioneer_macros::OpenRtbFieldMetadata;
use serde_json::value::RawValue;

pub type Raw<T = RawValue> = Option<Box<T>>;

#[derive(OpenRtbFieldMetadata)]
#[openrtb_metadata(name = "Geo")]
pub struct Location {
    pub ext: Raw,
}
